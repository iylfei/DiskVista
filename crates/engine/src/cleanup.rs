use crate::{safety::SafetyPolicy, store::Store};
use anyhow::{bail, Result};
use cleaner_domain::*;
use cleaner_platform::{filesystem, inventory, recycle};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
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
fn ensure_not_in_use(live: &[FileRecord], cancel: &AtomicBool) -> Result<()> {
    let paths: Vec<_> = live
        .iter()
        .filter(|f| !f.is_dir)
        .map(|f| f.path.as_str())
        .collect();
    let processes = cleaner_platform::process::locking_paths_checked(&paths, |_| {
        anyhow::ensure!(!cancel.load(Ordering::Relaxed), "用户取消，未处理剩余项目");
        Ok(())
    })?;
    if !processes.is_empty() {
        bail!("正在被使用：{}；请自行关闭应用", processes.join("、"));
    }
    Ok(())
}
fn prepare_recycle(
    path: &str,
    identity: &str,
    policy: &SafetyPolicy,
    apps: &crate::application_index::ApplicationIndex,
    cancel: &AtomicBool,
) -> Result<u64> {
    let live = live_tree_cancellable(path, cancel)?;
    if live.first().and_then(|file| file.identity.as_deref()) != Some(identity) {
        bail!("目标已被替换，请重新选择");
    }
    if let Some(reason) = live
        .iter()
        .find_map(|f| policy.reason(f).or_else(|| apps.installed_reason(f)))
    {
        bail!("回收开始前保护策略阻止：{reason}");
    }
    ensure_not_in_use(&live, cancel)?;
    Ok(live.iter().filter(|f| !f.is_dir).fold(0u64, |total, f| {
        total.saturating_add(f.logical_bytes.max(f.allocated_bytes.unwrap_or(0)))
    }))
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
    let policy = Arc::new(SafetyPolicy::new(store.settings()?));
    if preview.policy_fingerprint
        != format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&policy.settings)?)
        )
    {
        bail!("预览后设置或保护规则发生变化，请重新生成预览");
    }
    let apps = Arc::new(crate::application_index::ApplicationIndex::new(
        &inventory::installed_apps(),
        &policy,
    ));
    let rules = crate::rules::RuleSet::load(policy.settings.community_enabled)?;
    let batch = uuid::Uuid::new_v4().to_string();
    let mut history = Vec::new();
    struct Prepared {
        history_index: usize,
        volume: String,
    }
    let mut prepared = Vec::new();
    let mut requests = Vec::new();
    let mut free_before: HashMap<String, Option<u64>> = HashMap::new();
    for item in &preview.items {
        let mut record = HistoryItem {
            id: uuid::Uuid::new_v4().to_string(),
            batch_id: batch.clone(),
            path: item.path.clone(),
            bytes: item.bytes,
            time: chrono::Utc::now().timestamp(),
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
        if cancel.load(Ordering::Relaxed) || !item.allowed {
            record.message = if item.allowed {
                "用户取消，未处理剩余项目".into()
            } else {
                item.reason.clone()
            };
            history.push(record);
            continue;
        }
        let validation = || -> Result<()> {
            // Fail early for a missing or protected target; enumerate only in the Shell callback.
            let root = filesystem::validate_local_path(&item.path)?;
            let file = filesystem::inspect(&root)?;
            if let Some(reason) = policy
                .reason(&file)
                .or_else(|| apps.installed_reason(&file))
            {
                bail!("保护策略阻止：{reason}");
            }
            Ok(())
        };
        match validation() {
            Err(e) => record.message = format!("执行前复核未通过：{e:#}"),
            Ok(()) => {
                let path = item.path.clone();
                let volume = item.path[..3].to_owned();
                free_before
                    .entry(volume.clone())
                    .or_insert_with(|| filesystem::free_space(&volume).ok());
                let fp = item.fingerprint.clone();
                let policy = Arc::clone(&policy);
                let apps = Arc::clone(&apps);
                let item_cancel = cancel.clone();
                let predelete = move || -> Result<u64> {
                    prepare_recycle(&path, &fp, &policy, &apps, &item_cancel)
                };
                prepared.push(Prepared {
                    history_index: history.len(),
                    volume,
                });
                requests.push(recycle::Request::new(item.path.clone(), predelete));
            }
        }
        history.push(record);
    }

    let outcomes = match recycle::batch(requests, cancel.clone()) {
        Ok(outcomes) => outcomes,
        Err(error) => prepared
            .iter()
            .enumerate()
            .map(|(index, _)| {
                if index == 0 {
                    recycle::Outcome::Failed(format!("回收批次无法启动：{error:#}"))
                } else {
                    recycle::Outcome::Skipped(
                        "前一项无法确认安全回收，已停止批次，未处理此项".into(),
                    )
                }
            })
            .collect(),
    };
    let mut free_deltas: HashMap<String, i64> = free_before
        .into_iter()
        .filter_map(|(volume, before)| {
            let before = before?;
            let after = filesystem::free_space(&volume).ok()?;
            Some((
                volume,
                (after as i128 - before as i128).clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            ))
        })
        .collect();
    for (prepared, outcome) in prepared.into_iter().zip(outcomes) {
        let record = &mut history[prepared.history_index];
        record.time = chrono::Utc::now().timestamp();
        match outcome {
            recycle::Outcome::Recycled => {
                record.status = "recycled".into();
                record.message = "已确认移入 Windows 回收站；空间尚待回收站清空后释放".into();
                record.free_space_delta = free_deltas.remove(&prepared.volume).unwrap_or(0);
            }
            recycle::Outcome::Failed(message) => {
                record.status = "failed".into();
                record.message = format!("回收未确认成功，请检查回收站与原位置：{message}");
            }
            recycle::Outcome::Skipped(message) => record.message = message,
        }
    }
    for record in &history {
        store.add_history_for_scan(record, &preview.scan_id)?;
    }
    Ok(history)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recycle_check_accepts_content_changes_but_blocks_replacements_locks_and_protected_children()
    {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("target");
        std::fs::create_dir(&path).unwrap();
        let child = path.join("note.txt");
        std::fs::write(&child, "before").unwrap();
        let identity = filesystem::inspect(&path).unwrap().identity.unwrap();
        let policy = SafetyPolicy::new(Settings::default());
        let apps = crate::application_index::ApplicationIndex::new(&[], &policy);
        let cancel = AtomicBool::new(false);
        let check = |id: &str| prepare_recycle(path.to_str().unwrap(), id, &policy, &apps, &cancel);
        std::fs::write(&child, "changed since selection").unwrap();
        assert!(check(&identity).is_ok());
        assert!(check("different-file-id")
            .unwrap_err()
            .to_string()
            .contains("替换"));
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&child)
            .unwrap();
        assert!(check(&identity).is_err());
        drop(lock);
        std::fs::write(path.join(".env"), "SYNTHETIC=value").unwrap();
        assert!(check(&identity)
            .unwrap_err()
            .to_string()
            .contains("保护策略"));
        cancel.store(true, Ordering::Relaxed);
        assert!(check(&identity).unwrap_err().to_string().contains("取消"));
    }

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
