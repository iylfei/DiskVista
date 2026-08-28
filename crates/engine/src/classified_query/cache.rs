use super::{classifier, needs_classification, scope, Classifier};
use crate::{analysis_filter::AnalysisFilter, store::Store};
use anyhow::{ensure, Result};
use cleaner_domain::{EntryPage, EntryQuery, Group};
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{Arc, Mutex, OnceLock},
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
enum Contents {
    Entries(Box<[i64]>),
    Groups(Vec<Group>),
}

#[derive(Debug)]
struct View {
    contents: Contents,
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
    views: VecDeque<Arc<Slot>>,
}

#[derive(Default)]
pub struct QueryCache {
    state: Mutex<CacheState>,
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
            if state.revision != revision {
                return Err(ClassificationChanged.into());
            }
            state.views.retain(|slot| {
                slot.value
                    .get()
                    .is_none_or(|value| value.as_ref().is_ok_and(|view| view.valid(started)))
            });
            let slot = state
                .views
                .iter()
                .position(|slot| slot.key == key)
                .and_then(|position| state.views.remove(position))
                .unwrap_or_else(|| {
                    Arc::new(Slot {
                        key,
                        value: OnceLock::new(),
                    })
                });
            state.views.push_front(Arc::clone(&slot));
            state.views.truncate(4);
            slot
        };
        // Only callers for the same view wait here; no global cache or mutation lock is held.
        let value = slot.value.get_or_init(|| {
            #[cfg(test)]
            self.builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            build().map(Arc::new).map_err(|error| error.to_string())
        });
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
            let mut page = store.query(query)?;
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
                contents: Contents::Entries(ids.into_boxed_slice()),
                started,
                valid_until,
            })
        };
        let view = if classifier.cacheable() {
            self.get(key, started, revision, build)?
        } else {
            Arc::new(build()?)
        };
        let Contents::Entries(ids) = &view.contents else {
            unreachable!()
        };
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

    pub fn groups(
        &self,
        store: &Store,
        classifier: &Classifier<'_>,
        origin_key: &str,
        kind: &str,
    ) -> Result<Vec<Group>> {
        let kind = match kind {
            "risk" => "risk",
            "category" => "category",
            _ => "owner",
        };
        let started = chrono::Utc::now().timestamp();
        let revision = self.revision();
        let key = serde_json::to_string(&("groups", classifier.signature(origin_key)?, kind))?;
        let build = || {
            let mut groups = HashMap::<String, Group>::new();
            let mut valid_until = None;
            let mut visited = 0usize;
            let query = EntryQuery {
                scan_id: classifier.scan.id.clone(),
                ..Default::default()
            };
            scope::visit(store, &query, true, false, None, |mut file| {
                if visited.is_multiple_of(512) {
                    self.check_revision(revision)?;
                }
                visited += 1;
                update_expiry(&mut valid_until, classifier.next_change(&file, started));
                classifier.apply(&mut file);
                let name = match kind {
                    "risk" => &file.assessment.risk,
                    "category" => &file.assessment.category,
                    _ => file.assessment.owner.as_deref().unwrap_or("未知"),
                };
                let group = groups.entry(name.into()).or_insert_with(|| Group {
                    name: name.into(),
                    bytes: 0,
                    count: 0,
                });
                group.bytes = group
                    .bytes
                    .saturating_add(file.allocated_bytes.unwrap_or(file.logical_bytes));
                group.count = group.count.saturating_add(1);
                Ok(true)
            })?;
            let mut groups: Vec<_> = groups.into_values().collect();
            groups.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.name.cmp(&b.name)));
            groups.truncate(100);
            Ok(View {
                contents: Contents::Groups(groups),
                started,
                valid_until,
            })
        };
        let view = if classifier.cacheable() {
            self.get(key, started, revision, build)?
        } else {
            Arc::new(build()?)
        };
        self.check_revision(revision)?;
        if !view.valid(chrono::Utc::now().timestamp()) {
            return Err(ClassificationChanged.into());
        }
        let Contents::Groups(groups) = &view.contents else {
            unreachable!()
        };
        Ok(groups.clone())
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

    fn view(started: i64, valid_until: Option<i64>) -> View {
        View {
            contents: Contents::Entries(Vec::new().into_boxed_slice()),
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
}
