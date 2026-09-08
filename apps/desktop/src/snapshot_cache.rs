use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock},
};

struct Slot<T> {
    key: String,
    value: OnceLock<Result<Arc<T>, String>>,
}

struct Entries<T> {
    generation: u64,
    slots: VecDeque<Arc<Slot<T>>>,
}

/// Concurrent callers for one key share its build; unrelated keys never hold each other up.
pub struct SnapshotCache<T> {
    entries: Mutex<Entries<T>>,
    #[cfg(test)]
    builds: std::sync::atomic::AtomicUsize,
}

impl<T> Default for SnapshotCache<T> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(Entries {
                generation: 0,
                slots: VecDeque::new(),
            }),
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
    }

    pub fn get(
        &self,
        key: String,
        valid: impl Fn(&T) -> bool,
        build: impl FnOnce() -> Result<T, String>,
    ) -> Result<Arc<T>, String> {
        let (generation, slot) = {
            let mut entries = self.entries.lock().unwrap();
            entries.slots.retain(|slot| {
                slot.value
                    .get()
                    .is_none_or(|value| value.as_ref().is_ok_and(|value| valid(value)))
            });
            let slot = entries
                .slots
                .iter()
                .position(|slot| slot.key == key)
                .and_then(|position| entries.slots.remove(position))
                .unwrap_or_else(|| {
                    Arc::new(Slot {
                        key,
                        value: OnceLock::new(),
                    })
                });
            entries.slots.push_front(Arc::clone(&slot));
            // Two completed scans cover back/forward navigation without retaining many large indexes.
            entries.slots.truncate(2);
            (entries.generation, slot)
        };
        let value = slot.value.get_or_init(|| {
            #[cfg(test)]
            self.builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            build().map(Arc::new)
        });
        if self.entries.lock().unwrap().generation != generation {
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
}
