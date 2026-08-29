use cleaner_domain::*;
use cleaner_engine::{
    application_index::ApplicationIndex,
    classified_query::{ClassificationChanged, Classifier, QueryCache},
    context::Sample,
    rules::RuleSet,
    safety::SafetyPolicy,
    store::Store,
};
use cleaner_llm::client::Budget;
use cleaner_platform::process::WorkerJob;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    process::Child,
    sync::{atomic::AtomicBool, Arc, Mutex},
};
pub type Shared = Arc<AppState>;
pub struct WorkerHandle {
    pub child: Child,
    pub _job: WorkerJob,
}
pub struct ContextPreview {
    pub context: AnalysisContext,
    pub created: i64,
}
pub struct SamplePreview {
    pub context_id: String,
    pub samples: Vec<Sample>,
    pub created: i64,
}
pub struct AppState {
    pub store: Store,
    pub mutations: Mutex<()>,
    pub initialized: Mutex<bool>,
    pub worker: Mutex<Option<WorkerHandle>>,
    pub cleanup_previews: Mutex<HashMap<String, CleanupPreview>>,
    pub context_previews: Mutex<HashMap<String, ContextPreview>>,
    pub sample_previews: Mutex<HashMap<String, SamplePreview>>,
    pub cleanup_cancel: Arc<AtomicBool>,
    pub cleaning: AtomicBool,
    pub analysis_busy: AtomicBool,
    pub budgets: Mutex<HashMap<String, Budget>>,
    pub progress: Mutex<AnalysisProgress>,
    pub units_snapshot: Mutex<Option<(String, Vec<ApplicationUnit>)>>,
    pub origin_snapshot: Mutex<Option<(String, Arc<ApplicationIndex>)>>,
    pub suggestions_snapshot: Mutex<Option<(String, cleaner_engine::suggestions::SuggestionIndex)>>,
    pub classified_queries: QueryCache,
}
impl AppState {
    pub fn new(store: Store) -> Shared {
        Arc::new(Self {
            store,
            mutations: Mutex::new(()),
            initialized: Mutex::new(false),
            worker: Mutex::new(None),
            cleanup_previews: Mutex::new(HashMap::new()),
            context_previews: Mutex::new(HashMap::new()),
            sample_previews: Mutex::new(HashMap::new()),
            cleanup_cancel: Arc::new(AtomicBool::new(false)),
            cleaning: AtomicBool::new(false),
            analysis_busy: AtomicBool::new(false),
            budgets: Mutex::new(HashMap::new()),
            progress: Mutex::new(AnalysisProgress::default()),
            units_snapshot: Mutex::new(None),
            origin_snapshot: Mutex::new(None),
            suggestions_snapshot: Mutex::new(None),
            classified_queries: QueryCache::default(),
        })
    }
}
pub fn classification_key(settings: &Settings) -> Result<String, String> {
    serde_json::to_string(&(
        settings.community_enabled,
        &settings.protected_paths,
        &settings.unprotected_paths,
        &settings.ignored_paths,
        &settings.labels,
    ))
    .map_err(error)
}
impl AppState {
    pub fn application_index(
        &self,
        scan_id: &str,
        policy: &SafetyPolicy,
    ) -> Result<(String, Arc<ApplicationIndex>), String> {
        let scan = self.store.scan(scan_id).map_err(error)?;
        let apps = self.store.apps(scan_id).map_err(error)?;
        let key = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    "application-origins-v1",
                    &scan,
                    &apps,
                    classification_key(&policy.settings)?,
                ))
                .map_err(error)?
            )
        );
        let mut cached = self.origin_snapshot.lock().unwrap();
        if cached.as_ref().is_none_or(|(previous, _)| previous != &key) {
            let index = if scan.status == "complete" {
                ApplicationIndex::with_snapshot(&self.store, scan_id, &apps, policy)
                    .map_err(error)?
            } else {
                ApplicationIndex::new(&apps, policy)
            };
            *cached = Some((key.clone(), Arc::new(index)));
        }
        Ok((key, Arc::clone(&cached.as_ref().unwrap().1)))
    }

    pub fn invalidate_classification(&self) {
        self.classified_queries.clear();
        *self.units_snapshot.lock().unwrap() = None;
        *self.suggestions_snapshot.lock().unwrap() = None;
        *self.origin_snapshot.lock().unwrap() = None;
    }

    pub(crate) fn with_classification<T>(
        &self,
        scan_id: &str,
        mut read: impl FnMut(&Classifier<'_>, &str) -> anyhow::Result<T>,
    ) -> Result<T, String> {
        for _ in 0..3 {
            let revision = self.classified_queries.revision();
            let settings = self.store.settings().map_err(error)?;
            let scan = self.store.scan(scan_id).map_err(error)?;
            let rules = RuleSet::load(settings.community_enabled).map_err(error)?;
            let policy = SafetyPolicy::new(settings);
            let (key, apps) = self.application_index(scan_id, &policy)?;
            let classifier = Classifier::new(&scan, &rules, &policy, &apps);
            let result = read(&classifier, &key);
            if self.classified_queries.revision() != revision
                || result
                    .as_ref()
                    .err()
                    .is_some_and(|error| error.is::<ClassificationChanged>())
            {
                continue;
            }
            return result.map_err(error);
        }
        Err("分类依据正在变化，请稍后重试".into())
    }

    pub fn query_entries(&self, query: &EntryQuery) -> Result<EntryPage, String> {
        self.with_classification(&query.scan_id, |classifier, key| {
            let analysis =
                crate::analysis_results::filter(self, &query.scan_id, &query.analysis_status)
                    .map_err(anyhow::Error::msg)?;
            self.classified_queries.query_with_analysis(
                &self.store,
                query,
                classifier,
                key,
                analysis.as_ref(),
            )
        })
    }

    pub fn classified_entry(&self, scan_id: &str, entry_id: i64) -> Result<FileRecord, String> {
        self.with_classification(scan_id, |classifier, _| {
            let mut file = self.store.entry(scan_id, entry_id)?;
            classifier.apply(&mut file);
            Ok(file)
        })
    }
}
pub fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_queries_follow_labels_and_do_not_acquire_the_mutation_lock() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("queries.sqlite")).unwrap();
        store
            .save_scan(&Scan {
                id: "s".into(),
                root: "D:\\StateFixture".into(),
                status: "complete".into(),
                ..Default::default()
            })
            .unwrap();
        Store::insert_batch(
            &mut store.connection().unwrap(),
            "s",
            &[FileRecord {
                path: "D:\\StateFixture\\file.bin".into(),
                parent: "D:\\StateFixture".into(),
                name: "file.bin".into(),
                complete: true,
                logical_bytes: 200,
                assessment: Assessment {
                    risk: "review".into(),
                    category: "unknown".into(),
                    confidence: "low".into(),
                    ..Default::default()
                },
                ..Default::default()
            }],
        )
        .unwrap();
        let state = AppState::new(store);
        let mut query = EntryQuery {
            scan_id: "s".into(),
            parent: Some("D:\\StateFixture".into()),
            risk: Some("known".into()),
            limit: 100,
            ..Default::default()
        };
        let guard = state.mutations.lock().unwrap();
        let (sent, received) = std::sync::mpsc::channel();
        let task_state = state.clone();
        let task_query = query.clone();
        let task =
            std::thread::spawn(move || sent.send(task_state.query_entries(&task_query)).unwrap());
        let result = received.recv_timeout(std::time::Duration::from_secs(5));
        drop(guard);
        task.join().unwrap();
        assert_eq!(result.unwrap().unwrap().total, 0);

        let mut settings = Settings::default();
        settings
            .labels
            .insert("D:\\StateFixture\\file.bin".into(), "Current label".into());
        state.store.put("settings", &settings).unwrap();
        state.invalidate_classification();
        let labelled = state.query_entries(&query).unwrap();
        assert_eq!(labelled.total, 1);
        assert_eq!(
            labelled.items[0].assessment.owner.as_deref(),
            Some("Current label")
        );

        settings
            .protected_paths
            .push("D:\\StateFixture\\file.bin".into());
        state.store.put("settings", &settings).unwrap();
        state.invalidate_classification();
        query.risk = Some("protected".into());
        let protected = state.query_entries(&query).unwrap();
        assert_eq!(protected.total, 1);
        let detail = state.classified_entry("s", protected.items[0].id).unwrap();
        assert_eq!(
            serde_json::to_value(&detail.assessment).unwrap(),
            serde_json::to_value(&protected.items[0].assessment).unwrap()
        );
        assert!(state
            .store
            .entry("s", detail.id)
            .unwrap()
            .assessment
            .owner
            .is_none());
    }

    #[test]
    fn origin_cache_tracks_scan_completion_inventory_and_classification_settings() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("cache.sqlite")).unwrap();
        let mut scan = Scan {
            id: "s".into(),
            root: "D:\\Fixture".into(),
            status: "scanning".into(),
            ..Default::default()
        };
        store.save_scan(&scan).unwrap();
        let state = AppState::new(store);
        let policy = SafetyPolicy::new(Default::default());
        let (initial_key, initial) = state.application_index("s", &policy).unwrap();
        let (_, repeated) = state.application_index("s", &policy).unwrap();
        assert!(Arc::ptr_eq(&initial, &repeated));
        scan.status = "complete".into();
        scan.finished = Some(1);
        state.store.save_scan(&scan).unwrap();
        let (complete_key, _) = state.application_index("s", &policy).unwrap();
        assert_ne!(initial_key, complete_key);
        state
            .store
            .save_apps(
                "s",
                &[InstalledApp {
                    id: "app".into(),
                    name: "FixtureApp".into(),
                    publisher: String::new(),
                    install_location: "D:\\Fixture\\FixtureApp".into(),
                    source: "test".into(),
                    last_used: None,
                }],
            )
            .unwrap();
        let (inventory_key, _) = state.application_index("s", &policy).unwrap();
        assert_ne!(complete_key, inventory_key);
        let mut settings = Settings::default();
        settings
            .labels
            .insert("D:\\Fixture\\data".into(), "My files".into());
        let (classification_key, cached) = state
            .application_index("s", &SafetyPolicy::new(settings))
            .unwrap();
        assert_ne!(inventory_key, classification_key);
        state.invalidate_classification();
        assert!(state.origin_snapshot.lock().unwrap().is_none());
        assert!(!Arc::ptr_eq(&initial, &cached));
    }
}
