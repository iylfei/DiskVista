use crate::{context::fingerprint, safety::SafetyPolicy, store::Store};
use anyhow::{bail, Result};
use cleaner_domain::*;
use cleaner_platform::{filesystem, inventory, normalize, recycle, within};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub fn live_tree(path: &str) -> Result<Vec<FileRecord>> {
    let root = filesystem::validate_local_path(path)?;
    let mut out = Vec::new();
    let mut queue = VecDeque::from([root]);
    while let Some(path) = queue.pop_front() {
        let file = filesystem::inspect(&path)?;
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
pub fn preview(store: &Store, scan_id: &str, ids: &[i64]) -> Result<CleanupPreview> {
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
        selected.push(store.entry(scan_id, *id)?);
    }
    selected.sort_by_key(|f| f.path.len());
    selected.dedup_by_key(|f| f.id);
    let mut items = Vec::new();
    let mut accepted: Vec<String> = Vec::new();
    for file in selected {
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
        let descendants = match store.descendants_bounded(scan_id, &file.path, 100_000) {
            Ok(files) => files,
            Err(e) => {
                items.push(blocked(&file, &format!("无法核验：{e:#}")));
                continue;
            }
        };
        let live = match live_tree(&file.path) {
            Ok(live) => live,
            Err(e) => {
                items.push(blocked(&file, &format!("无法核验：{e:#}")));
                continue;
            }
        };
        if fingerprint(&descendants) != fingerprint(&live) {
            items.push(blocked(&file, "扫描后目标发生变化，请刷新后重新选择"));
            continue;
        }
        if let Some(reason) = live
            .iter()
            .find_map(|f| policy.reason(f).or_else(|| apps.installed_reason(f)))
        {
            items.push(blocked(&file, &format!("当前目标包含受保护内容：{reason}")));
            continue;
        }
        if let Err(e) = ensure_not_in_use(&live) {
            items.push(blocked(&file, &format!("占用检查未通过：{e:#}")));
            continue;
        }
        // Inspect actual link counts at preview; enumerated directory records do not include them.
        let bytes = live
            .iter()
            .filter(|f| !f.is_dir)
            .filter_map(|f| filesystem::inspect(Path::new(&f.path)).ok())
            .filter(|f| f.links <= 1)
            .map(|f| f.allocated_bytes.unwrap_or(f.logical_bytes))
            .sum();
        accepted.push(file.path.clone());
        let risk = rules.classify_indexed(&file, &policy, &apps).risk;
        items.push(CleanupItem {
            entry_id: file.id,
            path: file.path,
            bytes,
            fingerprint: fingerprint(&live),
            risk,
            allowed: true,
            reason: "已核对当前身份、大小、修改时间、后代、保护状态和占用；执行前仍会再次核验"
                .into(),
        });
    }
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
    let apps =
        crate::application_index::ApplicationIndex::new(&inventory::installed_apps(), &policy);
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
        };
        if aborted || cancel.load(Ordering::Relaxed) || !item.allowed {
            record.message = if aborted {
                "前一项无法确认安全回收，已停止批次，未处理此项".into()
            } else if item.allowed {
                "用户取消，未处理剩余项目".into()
            } else {
                item.reason.clone()
            };
            store.add_history(&record)?;
            history.push(record);
            continue;
        }
        let validation = || -> Result<u64> {
            let live = live_tree(&item.path)?;
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
                let predelete = move || -> Result<()> {
                    let live = live_tree(&path)?;
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
        store.add_history(&record)?;
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
