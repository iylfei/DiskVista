use crate::{context::fingerprint, safety::SafetyPolicy, store::Store};
use anyhow::{bail, Result};
use cleaner_domain::*;
use cleaner_platform::{filesystem, inventory, recycle};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

mod preview;
pub use preview::{preview, preview_with_progress};

pub fn live_tree(path: &str) -> Result<Vec<FileRecord>> {
    live_tree_cancellable(path, &AtomicBool::new(false))
}

pub fn live_tree_cancellable(path: &str, cancel: &AtomicBool) -> Result<Vec<FileRecord>> {
    live_tree_with_progress(path, cancel, |_| Ok(()))
}

fn live_tree_with_progress(
    path: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize) -> Result<()>,
) -> Result<Vec<FileRecord>> {
    if cancel.load(Ordering::Relaxed) {
        bail!("用户取消，未处理剩余项目");
    }
    let root = filesystem::validate_local_path(path)?;
    let mut out = Vec::new();
    let mut queue = VecDeque::from([root]);
    while let Some(path) = queue.pop_front() {
        if cancel.load(Ordering::Relaxed) {
            bail!("用户取消，未处理剩余项目");
        }
        let file = filesystem::inspect(&path)?;
        progress(out.len() + queue.len() + 1)?;
        if file.identity.is_none() {
            bail!("文件系统未提供可靠身份，无法安全复核此目标");
        }
        if out.len() + queue.len() > 100_000 {
            bail!("单个目标超过十万项，请展开目录并选择更小的清理范围");
        }
        if file.is_dir {
            if file.attributes & (filesystem::REPARSE | filesystem::OFFLINE | filesystem::RECALL)
                != 0
            {
                bail!("目标含链接、离线或云占位目录");
            }
            out.push(file);
            filesystem::enumerate(&path, |child| {
                if cancel.load(Ordering::Relaxed) {
                    bail!("用户取消，未处理剩余项目");
                }
                if child.identity.is_none() {
                    bail!("子项没有可靠文件身份，请逐层查看");
                }
                if out.len() + queue.len() > 100_000 {
                    bail!("单个目标超过十万项，请选择更小的清理范围");
                }
                if child.is_dir {
                    queue.push_back(Path::new(&child.path).to_owned())
                } else {
                    out.push(child)
                }
                progress(out.len() + queue.len())?;
                Ok(true)
            })?;
        } else {
            out.push(file);
        }
    }
    Ok(out)
}
fn ensure_not_in_use(live: &[FileRecord]) -> Result<()> {
    let paths: Vec<_> = live
        .iter()
        .filter(|f| !f.is_dir)
        .map(|f| f.path.as_str())
        .collect();
    let processes = cleaner_platform::process::locking_paths(&paths)?;
    if !processes.is_empty() {
        bail!("正在被使用：{}；请自行关闭应用", processes.join("、"));
    }
    Ok(())
}
pub fn execute(
    store: &Store,
    preview: &CleanupPreview,
    ack: bool,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<HistoryItem>> {
    if chrono::Utc::now().timestamp() - preview.created > 600 {
        bail!("清理预览已超过 10 分钟，请重新生成");
    }
    if preview.requires_extra_confirmation && !ack {
        bail!("未知或个人数据需要额外确认");
    }
    store.require_finished(&preview.scan_id)?;
    let policy = SafetyPolicy::new(store.settings()?);
    if preview.policy_fingerprint
        != format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&policy.settings)?)
        )
    {
        bail!("预览后设置或保护规则发生变化，请重新生成预览");
    }
    let apps = crate::application_index::ApplicationIndex::with_snapshot(
        store,
        &preview.scan_id,
        &inventory::installed_apps(),
        &policy,
    )?;
    let rules = crate::rules::RuleSet::load(policy.settings.community_enabled)?;
    let batch = uuid::Uuid::new_v4().to_string();
    let mut history = Vec::new();
    let mut aborted = false;
    for item in &preview.items {
        let now = chrono::Utc::now().timestamp();
        let mut record = HistoryItem {
            id: uuid::Uuid::new_v4().to_string(),
            batch_id: batch.clone(),
            path: item.path.clone(),
            bytes: item.bytes,
            time: now,
            status: "skipped".into(),
            message: String::new(),
            free_space_delta: 0,
            snapshot: store
                .entry(&preview.scan_id, item.entry_id)
                .ok()
                .map(|mut file| {
                    file.assessment = rules.classify_indexed(&file, &policy, &apps);
                    HistoryEntrySnapshot {
                        name: file.name,
                        is_dir: file.is_dir,
                        owner: file.assessment.owner,
                        category: file.assessment.category,
                        rule_id: file.assessment.rule_id,
                    }
                }),
        };
        if aborted || cancel.load(Ordering::Relaxed) || !item.allowed {
            record.message = if aborted {
                "前一项无法确认安全回收，已停止批次，未处理此项".into()
            } else if item.allowed {
                "用户取消，未处理剩余项目".into()
            } else {
                item.reason.clone()
            };
            store.add_history_for_scan(&record, &preview.scan_id)?;
            history.push(record);
            continue;
        }
        let validation = || -> Result<u64> {
            let live = live_tree_cancellable(&item.path, &cancel)?;
            if fingerprint(&live) != item.fingerprint {
                bail!("目标身份、大小、时间或目录内容已变化");
            }
            if let Some(reason) = live
                .iter()
                .find_map(|f| policy.reason(f).or_else(|| apps.installed_reason(f)))
            {
                bail!("保护策略阻止：{reason}");
            }
            ensure_not_in_use(&live)?;
            Ok(live.iter().filter(|f| !f.is_dir).fold(0u64, |total, f| {
                total.saturating_add(f.logical_bytes.max(f.allocated_bytes.unwrap_or(0)))
            }))
        };
        match validation() {
            Err(e) => record.message = format!("执行前复核未通过：{e:#}"),
            Ok(required_bytes) => {
                let before = filesystem::free_space(&item.path[..3]).ok();
                let path = item.path.clone();
                let fp = item.fingerprint.clone();
                let policy = policy.clone();
                let apps = apps.clone();
                let item_cancel = cancel.clone();
                let predelete = move || -> Result<()> {
                    let live = live_tree_cancellable(&path, &item_cancel)?;
                    if fingerprint(&live) != fp {
                        bail!("回收开始前目标再次变化");
                    }
                    if let Some(reason) = live
                        .iter()
                        .find_map(|f| policy.reason(f).or_else(|| apps.installed_reason(f)))
                    {
                        bail!("回收开始前保护策略阻止：{reason}");
                    }
                    ensure_not_in_use(&live)?;
                    Ok(())
                };
                match recycle::one(&item.path, required_bytes, predelete) {
                    Ok(()) => {
                        record.status = "recycled".into();
                        record.message =
                            "已确认移入 Windows 回收站；空间尚待回收站清空后释放".into();
                        if let (Some(a), Ok(b)) = (before, filesystem::free_space(&item.path[..3]))
                        {
                            record.free_space_delta = (b as i128 - a as i128)
                                .clamp(i64::MIN as i128, i64::MAX as i128)
                                as i64;
                        }
                    }
                    Err(e) => {
                        aborted = true;
                        record.status = "failed".into();
                        record.message = format!("回收未确认成功，请检查回收站与原位置：{e:#}");
                    }
                }
            }
        }
        record.time = chrono::Utc::now().timestamp();
        store.add_history_for_scan(&record, &preview.scan_id)?;
        history.push(record);
    }
    Ok(history)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_is_never_target() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::open(d.path().join("db")).unwrap();
        let scan = Scan {
            id: "s".into(),
            root: "D:\\".into(),
            status: "complete".into(),
            ..Default::default()
        };
        store.save_scan(&scan).unwrap();
        let mut c = store.connection().unwrap();
        Store::insert_batch(
            &mut c,
            "s",
            &[FileRecord {
                path: "D:\\".into(),
                is_dir: true,
                complete: true,
                enumerated: true,
                ..Default::default()
            }],
        )
        .unwrap();
        assert!(!preview(&store, "s", &[1]).unwrap().items[0].allowed);
    }
}
