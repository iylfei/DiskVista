use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, OnceLock},
};

struct Slot<T> {
    key: String,
    value: OnceLock<Result<Arc<T>, String>>,
}

struct Entries<T> {
    generation: u64,
    active: usize,
    slots: VecDeque<Arc<Slot<T>>>,
}

/// Keep in-flight builds separate from the completed LRU's eviction budget.
pub struct SnapshotCache<T> {
    entries: Mutex<Entries<T>>,
    ready: Condvar,
    #[cfg(test)]
    builds: std::sync::atomic::AtomicUsize,
}

impl<T> Default for SnapshotCache<T> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(Entries {
                generation: 0,
                active: 0,
                slots: VecDeque::new(),
            }),
            ready: Condvar::new(),
            #[cfg(test)]
            builds: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl<T> SnapshotCache<T> {
    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.generation = entries.generation.wrapping_add(1);
        entries.slots.clear();
        self.ready.notify_all();
    }

    pub fn get(
        &self,
        key: String,
        valid: impl Fn(&T) -> bool,
        build: impl FnOnce() -> Result<T, String>,
    ) -> Result<Arc<T>, String> {
        let (generation, slot) = {
            let mut entries = self.entries.lock().unwrap();
            let generation = entries.generation;
            let slot = loop {
                if entries.generation != generation {
                    return Err("分类依据正在变化，请稍后重试".into());
                }
                entries.slots.retain(|slot| {
                    slot.value
                        .get()
                        .is_none_or(|value| value.as_ref().is_ok_and(|value| valid(value)))
                });
                if let Some(slot) = entries
                    .slots
                    .iter()
                    .position(|slot| slot.key == key)
                    .and_then(|position| entries.slots.remove(position))
                {
                    break slot;
                }
                if entries.active < 2 {
                    entries.active += 1;
                    break Arc::new(Slot {
                        key: key.clone(),
                        value: OnceLock::new(),
                    });
                }
                entries = self.ready.wait(entries).unwrap();
            };
            entries.slots.push_front(Arc::clone(&slot));
            (entries.generation, slot)
        };
        let mut built = false;
        let value = slot.value.get_or_init(|| {
            built = true;
            #[cfg(test)]
            self.builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // Always publish a result and release admission, including a panicking builder.
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(build))
                .unwrap_or_else(|_| Err("索引构建异常，请重试".into()))
                .map(Arc::new)
        });
        let mut entries = self.entries.lock().unwrap();
        if built {
            entries.active -= 1;
            if let Some(position) = entries
                .slots
                .iter()
                .position(|entry| Arc::ptr_eq(entry, &slot))
            {
                let entry = entries.slots.remove(position).unwrap();
                entries.slots.push_front(entry);
            }
            self.ready.notify_all();
        }
        let mut completed = 0;
        entries.slots.retain(|slot| {
            if slot.value.get().is_none() {
                return true;
            }
            completed += 1;
            completed <= 2
        });
        if entries.generation != generation {
            return Err("分类依据正在变化，请稍后重试".into());
        }
        value.as_ref().map(Arc::clone).map_err(Clone::clone)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().slots.is_empty()
    }
    #[cfg(test)]
    pub fn build_count(&self) -> usize {
        self.builds.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_survives_invalidation_and_builder_panics() {
        let cache = SnapshotCache::<usize>::default();
        let (entered, waiting) = std::sync::mpsc::channel();
        let gate = (Mutex::new(false), Condvar::new());
        std::thread::scope(|scope| {
            for key in ["a", "b"] {
                let (cache, gate, entered) = (&cache, &gate, entered.clone());
                scope.spawn(move || {
                    cache.get(
                        key.into(),
                        |_| true,
                        || {
                            entered.send(()).unwrap();
                            let ready = gate.0.lock().unwrap();
                            let (ready, _) = gate
                                .1
                                .wait_timeout_while(
                                    ready,
                                    std::time::Duration::from_secs(5),
                                    |ready| !*ready,
                                )
                                .unwrap();
                            assert!(*ready);
                            Ok(1)
                        },
                    )
                });
            }
            for _ in 0..2 {
                waiting
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
            }
            cache.clear();
            let (cache, entered) = (&cache, entered.clone());
            let third = scope.spawn(move || {
                cache.get(
                    "c".into(),
                    |_| true,
                    || {
                        entered.send(()).unwrap();
                        Ok(3)
                    },
                )
            });
            assert!(matches!(
                waiting.recv_timeout(std::time::Duration::from_millis(50)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            *gate.0.lock().unwrap() = true;
            gate.1.notify_all();
            assert_eq!(*third.join().unwrap().unwrap(), 3);
        });
        assert!(cache
            .get("panic".into(), |_| true, || panic!("builder failed"))
            .is_err());
        assert_eq!(cache.entries.lock().unwrap().active, 0);
        assert_eq!(*cache.get("retry".into(), |_| true, || Ok(4)).unwrap(), 4);
    }

    #[test]
    fn different_key_and_invalidation_do_not_wait_for_slow_build() {
        let cache = SnapshotCache::<usize>::default();
        let (entered, waiting) = std::sync::mpsc::channel();
        let (release, resume) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let cache = &cache;
            let task = scope.spawn(move || {
                cache.get(
                    "slow".into(),
                    |_| true,
                    || {
                        entered.send(()).unwrap();
                        resume
                            .recv_timeout(std::time::Duration::from_secs(5))
                            .unwrap();
                        Ok(1)
                    },
                )
            });
            waiting
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            assert_eq!(*cache.get("fast".into(), |_| true, || Ok(2)).unwrap(), 2);
            cache.clear();
            release.send(()).unwrap();
            assert!(task.join().unwrap().is_err());
        });
        assert!(cache.is_empty());
    }

    #[test]
    fn reuses_values_and_rebuilds_expired_or_failed_values() {
        let cache = SnapshotCache::default();
        let first = cache.get("key".into(), |_| true, || Ok(1)).unwrap();
        let second = cache
            .get("key".into(), |_| true, || panic!("duplicate build"))
            .unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(*cache.get("key".into(), |_| false, || Ok(2)).unwrap(), 2);
        assert!(cache
            .get("failure".into(), |_| true, || Err("retry".into()))
            .is_err());
        assert_eq!(*cache.get("failure".into(), |_| true, || Ok(3)).unwrap(), 3);
    }
    #[test]
    fn pending_build_survives_completed_lru_churn() {
        let cache = SnapshotCache::<usize>::default();
        let (entered, waiting) = std::sync::mpsc::channel();
        let (release, resume) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let cache = &cache;
            let first = scope.spawn(move || {
                cache
                    .get(
                        "slow".into(),
                        |_| true,
                        || {
                            entered.send(()).unwrap();
                            resume
                                .recv_timeout(std::time::Duration::from_secs(5))
                                .unwrap();
                            Ok(1)
                        },
                    )
                    .unwrap()
            });
            waiting
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            for key in ["b", "c", "d", "e", "f", "g"] {
                cache.get(key.into(), |_| true, || Ok(2)).unwrap();
            }
            let second = scope.spawn(|| {
                cache
                    .get("slow".into(), |_| true, || panic!("duplicate build"))
                    .unwrap()
            });
            release.send(()).unwrap();
            assert!(Arc::ptr_eq(&first.join().unwrap(), &second.join().unwrap()));
        });
        assert_eq!(cache.build_count(), 7);
    }
}
