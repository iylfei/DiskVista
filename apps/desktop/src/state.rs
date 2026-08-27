use cleaner_domain::*;
use cleaner_engine::{context::Sample, store::Store};
use cleaner_llm::client::Budget;
use cleaner_platform::process::WorkerJob;
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
    pub suggestions_snapshot: Mutex<Option<(String, cleaner_engine::suggestions::SuggestionIndex)>>,
}
impl AppState {
    pub fn new(store: Store) -> Shared {
        Arc::new(Self {
            store,
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
            suggestions_snapshot: Mutex::new(None),
        })
    }
}
pub fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
