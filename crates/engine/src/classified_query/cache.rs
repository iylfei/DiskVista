use super::{classifier, needs_classification, scope, Classifier};
use crate::{analysis_filter::AnalysisFilter, store::Store};
use anyhow::{ensure, Result};
use cleaner_domain::{EntryPage, EntryQuery};
use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Condvar, Mutex, OnceLock},
};

#[derive(Debug)]
pub struct ClassificationChanged;

impl fmt::Display for ClassificationChanged {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("分类依据已变化，请重新加载")
    }
}

impl std::error::Error for ClassificationChanged {}

#[derive(Debug)]
struct View {
    entries: Box<[i64]>,
    started: i64,
    valid_until: Option<i64>,
}

impl View {
    fn valid(&self, now: i64) -> bool {
        now >= self.started && self.valid_until.is_none_or(|until| now < until)
    }
}

struct Slot {
    key: String,
    value: OnceLock<Result<Arc<View>, String>>,
}

#[derive(Default)]
struct CacheState {
    revision: u64,
    active: usize,
    views: VecDeque<Arc<Slot>>,
}

#[derive(Default)]
pub struct QueryCache {
    state: Mutex<CacheState>,
    ready: Condvar,
    counts: crate::store::CountCache,
    #[cfg(test)]
    builds: std::sync::atomic::AtomicUsize,
}

impl QueryCache {
    pub fn revision(&self) -> u64 {
        self.state.lock().unwrap().revision
    }

    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.revision = state.revision.wrapping_add(1);
        state.views.clear();
        self.ready.notify_all();
        drop(state);
        self.counts.clear();
    }

    pub fn query_unclassified(&self, store: &Store, query: &EntryQuery) -> Result<EntryPage> {
        store.query_cached(query, &self.counts)
    }

    fn check_revision(&self, revision: u64) -> Result<()> {
        if self.revision() != revision {
            return Err(ClassificationChanged.into());
        }
        Ok(())
    }

    fn get(
        &self,
        key: String,
        started: i64,
        revision: u64,
        build: impl FnOnce() -> Result<View>,
    ) -> Result<Arc<View>> {
        let slot = {
            let mut state = self.state.lock().unwrap();
            let slot = loop {
                if state.revision != revision {
                    return Err(ClassificationChanged.into());
                }
                state.views.retain(|slot| {
                    slot.value
                        .get()
                        .is_none_or(|value| value.as_ref().is_ok_and(|view| view.valid(started)))
                });
                if let Some(slot) = state
                    .views
                    .iter()
                    .position(|slot| slot.key == key)
                    .and_then(|position| state.views.remove(position))
                {
                    break slot;
                }
                if state.active < 2 {
                    state.active += 1;
                    break Arc::new(Slot {
                        key: key.clone(),
                        value: OnceLock::new(),
                    });
                }
                state = self.ready.wait(state).unwrap();
            };
            state.views.push_front(Arc::clone(&slot));
            slot
        };
        let mut built = false;
        let value = slot.value.get_or_init(|| {
            built = true;
            #[cfg(test)]
            self.builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(build))
                .map_err(|_| anyhow::anyhow!("查询索引构建异常，请重试"))
                .and_then(|result| result)
                .map(Arc::new)
                .map_err(|error| error.to_string())
        });
        {
            let mut state = self.state.lock().unwrap();
            if built {
                state.active -= 1;
                if let Some(position) = state
                    .views
                    .iter()
                    .position(|entry| Arc::ptr_eq(entry, &slot))
                {
                    let entry = state.views.remove(position).unwrap();
                    state.views.push_front(entry);
                }
                self.ready.notify_all();
            }
            let mut completed = 0;
            state.views.retain(|slot| {
                if slot.value.get().is_none() {
                    return true;
                }
                completed += 1;
                completed <= 4
            });
        }
        self.check_revision(revision)?;
        value
            .as_ref()
            .map(Arc::clone)
            .map_err(|message| anyhow::anyhow!(message.clone()))
    }

    pub fn query(
        &self,
        store: &Store,
        query: &EntryQuery,
        classifier: &Classifier<'_>,
        origin_key: &str,
    ) -> Result<EntryPage> {
        self.query_with_analysis(store, query, classifier, origin_key, None)
    }

    pub fn query_with_analysis(
        &self,
        store: &Store,
        query: &EntryQuery,
        classifier: &Classifier<'_>,
        origin_key: &str,
        analysis: Option<&AnalysisFilter>,
    ) -> Result<EntryPage> {
        ensure!(query.scan_id == classifier.scan.id, "扫描记录不匹配");
        AnalysisFilter::validate(&query.analysis_status, analysis)?;
        let revision = self.revision();
        if !needs_classification(query) {
            let mut page = self.query_unclassified(store, query)?;
            for file in &mut page.items {
                classifier.apply(file);
            }
            self.check_revision(revision)?;
            return Ok(page);
        }
        let started = chrono::Utc::now().timestamp();
        let normalized = scope::normalized(query);
        let key = serde_json::to_string(&(
            "entries",
            classifier.signature(origin_key)?,
            &normalized,
            analysis,
        ))?;
        let build = || {
            let mut ids = Vec::new();
            let mut valid_until = None;
            let mut visited = 0usize;
            scope::visit(store, &normalized, false, true, analysis, |mut file| {
                if visited.is_multiple_of(512) {
                    self.check_revision(revision)?;
                }
                visited += 1;
                update_expiry(&mut valid_until, classifier.next_change(&file, started));
                classifier.apply(&mut file);
                if classifier::matches(&file, &normalized) {
                    ids.push(file.id);
                }
                Ok(true)
            })?;
            Ok(View {
                entries: ids.into_boxed_slice(),
                started,
                valid_until,
            })
        };
        let view = if classifier.cacheable() {
            self.get(key, started, revision, build)?
        } else {
            Arc::new(build()?)
        };
        let ids = &view.entries;
        let start = (query.offset as usize).min(ids.len());
        let end = start
            .saturating_add(query.limit.clamp(1, 200) as usize)
            .min(ids.len());
        let mut items = scope::load(store, &query.scan_id, &ids[start..end])?;
        for file in &mut items {
            classifier.apply(file);
        }
        self.check_revision(revision)?;
        if !view.valid(chrono::Utc::now().timestamp()) {
            return Err(ClassificationChanged.into());
        }
        Ok(EntryPage {
            items,
            total: ids.len() as u64,
        })
    }

    #[cfg(test)]
    pub(super) fn build_count(&self) -> usize {
        self.builds.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(super) fn next_expiry(&self) -> Option<i64> {
        self.state
            .lock()
            .unwrap()
            .views
            .front()?
            .value
            .get()?
            .as_ref()
            .ok()?
            .valid_until
    }
}

fn update_expiry(current: &mut Option<i64>, next: Option<i64>) {
    if let Some(next) = next {
        *current = Some(current.map_or(next, |old| old.min(next)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_survives_invalidation_and_builder_panics() {
        let cache = QueryCache::default();
        let (entered, waiting) = std::sync::mpsc::channel();
        let gate = (Mutex::new(false), Condvar::new());
        std::thread::scope(|scope| {
            for key in ["a", "b"] {
                let (cache, gate, entered) = (&cache, &gate, entered.clone());
                scope.spawn(move || {
                    cache.get(key.into(), 10, 0, || {
                        entered.send(()).unwrap();
                        let ready = gate.0.lock().unwrap();
                        let (ready, _) = gate
                            .1
                            .wait_timeout_while(ready, std::time::Duration::from_secs(5), |ready| {
                                !*ready
                            })
                            .unwrap();
                        assert!(*ready);
                        Ok(view(10, None))
                    })
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
                cache.get("c".into(), 10, 1, || {
                    entered.send(()).unwrap();
                    Ok(view(10, None))
                })
            });
            assert!(matches!(
                waiting.recv_timeout(std::time::Duration::from_millis(50)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            *gate.0.lock().unwrap() = true;
            gate.1.notify_all();
            assert!(third.join().unwrap().is_ok());
        });
        assert!(cache
            .get("panic".into(), 10, 1, || panic!("builder failed"))
            .is_err());
        assert_eq!(cache.state.lock().unwrap().active, 0);
        assert!(cache
            .get("retry".into(), 10, 1, || Ok(view(10, None)))
            .is_ok());
    }

    fn view(started: i64, valid_until: Option<i64>) -> View {
        View {
            entries: Vec::new().into_boxed_slice(),
            started,
            valid_until,
        }
    }

    #[test]
    fn lru_reuses_views_and_expires_at_age_threshold_or_clock_rollback() {
        let cache = QueryCache::default();
        let first = cache
            .get("a".into(), 10, 0, || Ok(view(10, Some(20))))
            .unwrap();
        let hit = cache
            .get("a".into(), 19, 0, || panic!("cache miss"))
            .unwrap();
        assert!(Arc::ptr_eq(&first, &hit));
        let expired = cache.get("a".into(), 20, 0, || Ok(view(20, None))).unwrap();
        assert!(!Arc::ptr_eq(&first, &expired));
        let rolled_back = cache.get("a".into(), 15, 0, || Ok(view(15, None))).unwrap();
        assert!(!Arc::ptr_eq(&expired, &rolled_back));
        for key in ["b", "c", "d", "e"] {
            cache.get(key.into(), 15, 0, || Ok(view(15, None))).unwrap();
        }
        assert_eq!(cache.state.lock().unwrap().views.len(), 4);
        let before = cache.build_count();
        cache.get("a".into(), 15, 0, || Ok(view(15, None))).unwrap();
        assert_eq!(cache.build_count(), before + 1);
    }

    #[test]
    fn concurrent_build_does_not_hold_cache_lock_and_cannot_publish_after_clear() {
        let cache = QueryCache::default();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let cache = &cache;
            let task = scope.spawn(move || {
                cache.get("slow".into(), 10, 0, || {
                    entered_tx.send(()).unwrap();
                    release_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    Ok(view(10, None))
                })
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            assert!(cache.state.try_lock().is_ok());
            cache
                .get("other".into(), 10, 0, || Ok(view(10, None)))
                .unwrap();
            cache.clear();
            release_tx.send(()).unwrap();
            assert!(task
                .join()
                .unwrap()
                .unwrap_err()
                .is::<ClassificationChanged>());
        });
        assert!(cache.state.lock().unwrap().views.is_empty());
    }

    #[test]
    fn concurrent_requests_for_the_same_view_share_one_build() {
        let cache = QueryCache::default();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let cache = &cache;
            let first = scope.spawn(move || {
                cache
                    .get("same".into(), 10, 0, || {
                        entered_tx.send(()).unwrap();
                        release_rx
                            .recv_timeout(std::time::Duration::from_secs(5))
                            .unwrap();
                        Ok(view(10, None))
                    })
                    .unwrap()
            });
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            let second = scope.spawn(|| {
                cache
                    .get("same".into(), 10, 0, || panic!("duplicate build"))
                    .unwrap()
            });
            release_tx.send(()).unwrap();
            assert!(Arc::ptr_eq(&first.join().unwrap(), &second.join().unwrap()));
        });
        assert_eq!(cache.build_count(), 1);
    }
    #[test]
    fn pending_build_survives_completed_lru_churn() {
        let cache = QueryCache::default();
        let (entered, waiting) = std::sync::mpsc::channel();
        let (release, resume) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let cache = &cache;
            let first = scope.spawn(move || {
                cache
                    .get("slow".into(), 10, 0, || {
                        entered.send(()).unwrap();
                        resume
                            .recv_timeout(std::time::Duration::from_secs(5))
                            .unwrap();
                        Ok(view(10, None))
                    })
                    .unwrap()
            });
            waiting
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            for key in ["b", "c", "d", "e", "f", "g"] {
                cache.get(key.into(), 10, 0, || Ok(view(10, None))).unwrap();
            }
            let second = scope.spawn(|| {
                cache
                    .get("slow".into(), 10, 0, || panic!("duplicate build"))
                    .unwrap()
            });
            release.send(()).unwrap();
            assert!(Arc::ptr_eq(&first.join().unwrap(), &second.join().unwrap()));
        });
        assert_eq!(cache.build_count(), 7);
    }
}
