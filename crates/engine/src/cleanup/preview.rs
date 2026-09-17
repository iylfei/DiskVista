use crate::{safety::SafetyPolicy, store::Store};
use anyhow::{bail, Result};
use cleaner_domain::*;
use cleaner_platform::{filesystem, normalize, within};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct Reporter<F> {
    cancel: Arc<AtomicBool>,
    value: CleanupCheckProgress,
    send: F,
}
impl<F: FnMut(CleanupCheckProgress)> Reporter<F> {
    fn new(cancel: Arc<AtomicBool>, total: usize, send: F) -> Self {
        Self {
            cancel,
            value: CleanupCheckProgress {
                stage: CleanupCheckStage::Preparing,
                targets_done: 0,
                targets_total: total,
                current_path: None,
                checked_entries: 0,
            },
            send,
        }
    }
    fn check(&self) -> Result<()> {
        anyhow::ensure!(!self.cancel.load(Ordering::Relaxed), "安全检查已取消");
        Ok(())
    }
    fn publish(&mut self) -> Result<()> {
        self.check()?;
        (self.send)(self.value.clone());
        self.check()
    }
    fn stage(&mut self, stage: CleanupCheckStage) -> Result<()> {
        self.value.stage = stage;
        self.value.checked_entries = 0;
        self.publish()
    }
}

pub fn preview(store: &Store, scan_id: &str, ids: &[i64]) -> Result<CleanupPreview> {
    preview_with_progress(
        store,
        scan_id,
        ids,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
}

pub fn preview_with_progress(
    store: &Store,
    scan_id: &str,
    ids: &[i64],
    cancel: Arc<AtomicBool>,
    progress: impl FnMut(CleanupCheckProgress),
) -> Result<CleanupPreview> {
    let mut report = Reporter::new(cancel, ids.len(), progress);
    report.stage(CleanupCheckStage::Preparing)?;
    store.require_finished(scan_id)?;
    if ids.is_empty() || ids.len() > 500 {
        bail!("请在待清理清单中选择 1 到 500 项");
    }
    let policy = SafetyPolicy::new(store.settings()?);
    let rules = crate::rules::RuleSet::load(policy.settings.community_enabled)?;
    let apps = crate::application_index::ApplicationIndex::new(&store.apps(scan_id)?, &policy);
    let scan = store.scan(scan_id)?;
    let mut selected = Vec::new();
    for id in ids {
        report.check()?;
        selected.push(store.entry(scan_id, *id)?);
    }
    selected.sort_by_key(|f| f.path.len());
    selected.dedup_by_key(|f| f.id);
    let mut items = Vec::new();
    let mut accepted: Vec<String> = Vec::new();
    report.value.targets_total = selected.len();
    for (index, file) in selected.into_iter().enumerate() {
        report.value.targets_done = index;
        report.value.current_path = Some(file.path.clone());
        report.stage(CleanupCheckStage::Preparing)?;
        if normalize(&file.path) == normalize(&scan.root) {
            items.push(blocked(&file, "扫描根目录不能整体回收"));
            continue;
        }
        if accepted.iter().any(|p| within(&file.path, p)) {
            continue;
        }
        let reason = policy
            .reason(&file)
            .or_else(|| apps.installed_reason(&file))
            .or_else(|| {
                if !file.complete || file.has_blocked_children {
                    Some("目标或其后代扫描不完整/受保护".into())
                } else {
                    None
                }
            });
        if let Some(reason) = reason {
            items.push(blocked(&file, &reason));
            continue;
        }
        report.stage(CleanupCheckStage::Protection)?;
        let current = match filesystem::validate_local_path(&file.path)
            .and_then(|path| filesystem::inspect(&path))
        {
            Ok(current) => current,
            Err(error) => {
                items.push(blocked(&file, &format!("无法读取目标：{error:#}")));
                continue;
            }
        };
        if let Some(reason) = policy
            .reason(&current)
            .or_else(|| apps.installed_reason(&current))
        {
            items.push(blocked(&file, &reason));
            continue;
        }
        let Some(identity) = current.identity else {
            items.push(blocked(&file, "无法识别目标文件"));
            continue;
        };
        // Directory sizes are estimates from the scan; do not reopen every file to count bytes.
        let bytes = if current.is_dir {
            file.allocated_bytes.unwrap_or(file.logical_bytes)
        } else if current.links > 1 {
            0
        } else {
            current.allocated_bytes.unwrap_or(current.logical_bytes)
        };
        report.check()?;
        accepted.push(file.path.clone());
        let risk = rules.classify_indexed(&file, &policy, &apps).risk;
        items.push(CleanupItem {
            entry_id: file.id,
            path: file.path,
            bytes,
            fingerprint: identity,
            risk,
            allowed: true,
            reason: "回收前检查文件占用及受保护路径".into(),
        });
    }
    report.value.targets_done = report.value.targets_total;
    report.value.current_path = None;
    report.stage(CleanupCheckStage::Complete)?;
    let pending_bytes = items.iter().filter(|i| i.allowed).map(|i| i.bytes).sum();
    let requires_extra_confirmation = items.iter().any(|i| i.allowed && i.risk != "low");
    Ok(CleanupPreview {
        id: uuid::Uuid::new_v4().to_string(),
        scan_id: scan_id.into(),
        created: chrono::Utc::now().timestamp(),
        items,
        pending_bytes,
        requires_extra_confirmation,
        policy_fingerprint: format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&policy.settings)?)
        ),
    })
}
fn blocked(file: &FileRecord, reason: &str) -> CleanupItem {
    CleanupItem {
        entry_id: file.id,
        path: file.path.clone(),
        bytes: file.allocated_bytes.unwrap_or(file.logical_bytes),
        fingerprint: String::new(),
        risk: "protected".into(),
        allowed: false,
        reason: reason.into(),
    }
}
