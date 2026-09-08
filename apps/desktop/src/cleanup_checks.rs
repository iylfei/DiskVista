use cleaner_domain::{CleanupCheckProgress, CleanupCheckStage, CleanupPreview};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

struct CheckState {
    started: bool,
    finished: Option<Instant>,
    preview_id: Option<String>,
    progress: CleanupCheckProgress,
}

pub struct Check {
    pub cancel: Arc<AtomicBool>,
    state: Mutex<CheckState>,
}

impl Check {
    fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            state: Mutex::new(CheckState {
                started: false,
                finished: None,
                preview_id: None,
                progress: CleanupCheckProgress {
                    stage: CleanupCheckStage::Queued,
                    targets_done: 0,
                    targets_total: 0,
                    current_path: None,
                    checked_entries: 0,
                },
            }),
        }
    }

    pub fn report(&self, progress: CleanupCheckProgress) {
        let mut state = self.state.lock().unwrap();
        if !self.cancel.load(Ordering::Relaxed) && state.finished.is_none() {
            state.progress = progress;
        }
    }

    pub fn finish(
        &self,
        result: Result<CleanupPreview, String>,
        previews: &Mutex<HashMap<String, CleanupPreview>>,
    ) -> Result<CleanupPreview, String> {
        let mut state = self.state.lock().unwrap();
        state.finished = Some(Instant::now());
        if self.cancel.load(Ordering::Relaxed) {
            state.progress.stage = CleanupCheckStage::Cancelled;
            return Err("安全检查已取消".into());
        }
        match result {
            Ok(preview) => {
                let mut previews = previews.lock().unwrap();
                previews.retain(|_, p| chrono::Utc::now().timestamp() - p.created < 600);
                if previews.len() > 20 {
                    previews.clear();
                }
                state.preview_id = Some(preview.id.clone());
                state.progress.stage = CleanupCheckStage::Complete;
                previews.insert(preview.id.clone(), preview.clone());
                Ok(preview)
            }
            Err(error) => {
                state.progress.stage = CleanupCheckStage::Failed;
                Err(error)
            }
        }
    }

    fn cancel(&self, previews: &Mutex<HashMap<String, CleanupPreview>>) {
        self.cancel.store(true, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap();
        state.progress.stage = CleanupCheckStage::Cancelled;
        state.finished = Some(Instant::now());
        if let Some(id) = state.preview_id.take() {
            previews.lock().unwrap().remove(&id);
        }
    }
}

#[derive(Default)]
pub struct PreviewChecks {
    checks: Mutex<HashMap<String, Arc<Check>>>,
}

impl PreviewChecks {
    fn get_or_insert(&self, id: &str) -> Result<Arc<Check>, String> {
        uuid::Uuid::parse_str(id).map_err(|_| "安全检查标识无效")?;
        let mut checks = self.checks.lock().unwrap();
        checks.retain(|_, check| {
            check
                .state
                .lock()
                .unwrap()
                .finished
                .is_none_or(|finished| finished.elapsed() < Duration::from_secs(600))
        });
        if let Some(check) = checks.get(id) {
            return Ok(check.clone());
        }
        if checks.len() >= 64 {
            let oldest = checks
                .iter()
                .filter_map(|(id, check)| {
                    check
                        .state
                        .lock()
                        .unwrap()
                        .finished
                        .map(|when| (id.clone(), when))
                })
                .min_by_key(|(_, when)| *when)
                .map(|(id, _)| id);
            if let Some(id) = oldest {
                checks.remove(&id);
            } else {
                return Err("正在处理的安全检查过多，请稍后重试".into());
            }
        }
        let check = Arc::new(Check::new());
        checks.insert(id.into(), check.clone());
        Ok(check)
    }

    pub fn begin(&self, id: &str, total: usize) -> Result<Arc<Check>, String> {
        let check = self.get_or_insert(id)?;
        {
            let mut state = check.state.lock().unwrap();
            if check.cancel.load(Ordering::Relaxed) {
                return Err("安全检查已取消".into());
            }
            if state.started {
                return Err("安全检查标识已经使用".into());
            }
            state.started = true;
            state.progress.targets_total = total;
        }
        Ok(check)
    }

    pub fn progress(&self, id: &str) -> Option<CleanupCheckProgress> {
        let check = self.checks.lock().unwrap().get(id).cloned()?;
        let progress = check.state.lock().unwrap().progress.clone();
        Some(progress)
    }

    pub fn cancel(
        &self,
        id: &str,
        previews: &Mutex<HashMap<String, CleanupPreview>>,
    ) -> Result<(), String> {
        // A close can reach Rust before preview_cleanup starts. Retain its cancelled
        // token so a subsequently scheduled command cannot start this request again.
        self.get_or_insert(id)?.cancel(previews);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_before_start_and_after_publication_cannot_leave_an_executable_preview() {
        let checks = PreviewChecks::default();
        let previews = Mutex::new(HashMap::new());
        let early = uuid::Uuid::new_v4().to_string();
        checks.cancel(&early, &previews).unwrap();
        assert!(checks.begin(&early, 1).is_err());
        let id = uuid::Uuid::new_v4().to_string();
        let check = checks.begin(&id, 1).unwrap();
        let preview = CleanupPreview {
            id: "published".into(),
            scan_id: "scan".into(),
            created: chrono::Utc::now().timestamp(),
            items: vec![],
            pending_bytes: 0,
            requires_extra_confirmation: false,
            policy_fingerprint: String::new(),
        };
        check.finish(Ok(preview), &previews).unwrap();
        assert_eq!(previews.lock().unwrap().len(), 1);
        checks.cancel(&id, &previews).unwrap();
        assert!(previews.lock().unwrap().is_empty());
        assert!(checks.begin(&id, 1).is_err());
        let next = checks.begin(&uuid::Uuid::new_v4().to_string(), 1).unwrap();
        assert!(!next.cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn cancelling_active_work_blocks_late_progress_and_publication() {
        let checks = PreviewChecks::default();
        let previews = Mutex::new(HashMap::new());
        let id = uuid::Uuid::new_v4().to_string();
        let check = checks.begin(&id, 1).unwrap();
        assert!(checks.begin(&id, 1).is_err());
        let mut progress = checks.progress(&id).unwrap();
        progress.stage = CleanupCheckStage::Filesystem;
        checks.cancel(&id, &previews).unwrap();
        check.report(progress);
        assert!(check
            .finish(Err("late failure".into()), &previews)
            .unwrap_err()
            .contains("取消"));
        assert_eq!(
            checks.progress(&id).unwrap().stage,
            CleanupCheckStage::Cancelled
        );
    }
}
