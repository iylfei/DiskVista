use super::live_tree_with_progress;
use crate::{context::fingerprint_checked, safety::SafetyPolicy, store::Store};
use anyhow::{bail, Result};
use cleaner_domain::*;
use cleaner_platform::{filesystem, normalize, within};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct Reporter<F> {
    cancel: Arc<AtomicBool>,
    value: CleanupCheckProgress,
    send: F,
    sent: Instant,
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
            sent: Instant::now(),
        }
    }
    fn check(&self) -> Result<()> {
        anyhow::ensure!(!self.cancel.load(Ordering::Relaxed), "安全检查已取消");
        Ok(())
    }
    fn publish(&mut self) -> Result<()> {
        self.check()?;
        (self.send)(self.value.clone());
        self.sent = Instant::now();
        self.check()
    }
    fn stage(&mut self, stage: CleanupCheckStage) -> Result<()> {
        self.value.stage = stage;
        self.value.checked_entries = 0;
        self.publish()
    }
    fn visited(&mut self, count: usize) -> Result<()> {
        self.check()?;
        self.value.checked_entries = count;
        if count.is_multiple_of(64) || self.sent.elapsed() >= Duration::from_millis(100) {
            self.publish()?;
        }
        Ok(())
    }
}

fn check_usage(files: &[FileRecord], progress: impl FnMut(usize) -> Result<()>) -> Result<()> {
    let paths: Vec<_> = files
        .iter()
        .filter(|f| !f.is_dir)
        .map(|f| f.path.as_str())
        .collect();
    let processes = cleaner_platform::process::locking_paths_checked(&paths, progress)?;
    if !processes.is_empty() {
        bail!("正在被使用：{}；请自行关闭应用", processes.join("、"));
    }
    Ok(())
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
        report.stage(CleanupCheckStage::Snapshot)?;
        let descendants = match store.descendants_bounded_with_progress(
            scan_id,
            &file.path,
            100_000,
            report.cancel.clone(),
            |count| report.visited(count),
        ) {
            Ok(files) => files,
            Err(e) => {
                report.check()?;
                items.push(blocked(&file, &format!("无法核验：{e:#}")));
                continue;
            }
        };
        let snapshot_fingerprint = fingerprint_checked(&descendants, || report.check())?;
        drop(descendants);
        report.stage(CleanupCheckStage::Filesystem)?;
        let flag = report.cancel.clone();
        let live = match live_tree_with_progress(&file.path, &flag, |count| report.visited(count)) {
            Ok(live) => live,
            Err(e) => {
                report.check()?;
                items.push(blocked(&file, &format!("无法核验：{e:#}")));
                continue;
            }
        };
        report.stage(CleanupCheckStage::Protection)?;
        let live_fingerprint = fingerprint_checked(&live, || report.check())?;
        if snapshot_fingerprint != live_fingerprint {
            items.push(blocked(&file, "扫描后目标发生变化，请刷新后重新选择"));
            continue;
        }
        let mut protected_reason = None;
        for (index, f) in live.iter().enumerate() {
            report.visited(index + 1)?;
            if let Some(reason) = policy.reason(f).or_else(|| apps.installed_reason(f)) {
                protected_reason = Some(reason);
                break;
            }
        }
        if let Some(reason) = protected_reason {
            items.push(blocked(&file, &format!("当前目标包含受保护内容：{reason}")));
            continue;
        }
        report.stage(CleanupCheckStage::Usage)?;
        if let Err(e) = check_usage(&live, |count| report.visited(count)) {
            report.check()?;
            items.push(blocked(&file, &format!("占用检查未通过：{e:#}")));
            continue;
        }
        // Inspect actual link counts at preview; enumerated directory records do not include them.
        report.stage(CleanupCheckStage::Size)?;
        let mut bytes = 0u64;
        for (index, file) in live.iter().filter(|f| !f.is_dir).enumerate() {
            report.visited(index + 1)?;
            if let Ok(file) = filesystem::inspect(Path::new(&file.path)) {
                if file.links <= 1 {
                    bytes =
                        bytes.saturating_add(file.allocated_bytes.unwrap_or(file.logical_bytes));
                }
            }
        }
        report.check()?;
        accepted.push(file.path.clone());
        let risk = rules.classify_indexed(&file, &policy, &apps).risk;
        items.push(CleanupItem {
            entry_id: file.id,
            path: file.path,
            bytes,
            fingerprint: live_fingerprint,
            risk,
            allowed: true,
            reason: "已核对当前身份、大小、修改时间、后代、保护状态和占用；执行前仍会再次核验"
                .into(),
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
