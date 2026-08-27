use crate::{rules::RuleSet, safety::SafetyPolicy, store::Store};
use anyhow::{Context, Result};
use cleaner_domain::*;
use cleaner_platform::{filesystem, inventory, journal, normalize, within};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanJob {
    pub database: PathBuf,
    pub scan_id: String,
    pub root: String,
    pub settings: Settings,
    #[serde(default)]
    pub journal_probe: Option<journal::Probe>,
}
#[derive(Serialize, Deserialize)]
pub struct SnapshotJournal {
    pub scan_id: String,
    pub root_identity: Option<String>,
    pub checkpoint: journal::Checkpoint,
}
// The bounded queue costs at most 512 records; inline entries avoid one heap allocation per file.
#[allow(clippy::large_enum_variant)]
enum Event {
    Entry(FileRecord),
    Done(i64, Option<String>),
}

pub fn run(job: ScanJob, cancel: Arc<AtomicBool>, mut progress: impl FnMut(&Scan)) -> Result<()> {
    let store = Store::open(&job.database)?;
    let root_path = filesystem::validate_local_path(&job.root)?;
    let policy = SafetyPolicy::new(job.settings.clone());
    let rules = RuleSet::load(job.settings.community_enabled)?;
    let apps = inventory::installed_apps();
    store.save_apps(&job.scan_id, &apps)?;
    let apps = crate::application_index::ApplicationIndex::new(&apps, &policy);
    let mut root = filesystem::inspect(&root_path)?;
    root.assessment = rules.classify_indexed(&root, &policy, &apps);
    root.assessment.risk = "protected".into();
    root.assessment.protected_reason = Some("扫描根目录不作为整体清理目标，请逐层选择".into());
    root.complete = !root.is_dir;
    let mut scan = store.scan(&job.scan_id)?;
    scan.status = "scanning".into();
    scan.message = "正在读取文件信息".into();
    let checkpoint = job
        .journal_probe
        .as_ref()
        .map(|p| p.checkpoint.clone())
        .or_else(|| journal::checkpoint(&job.root).ok());
    let journal_key = format!("journal:{}", normalize(&job.root));
    let previous: Option<SnapshotJournal> = store.get(&journal_key)?;
    let mut reused = false;
    if let (Some(old), Some(current)) = (&previous, &checkpoint) {
        if old.root_identity == root.identity
            && store
                .scan(&old.scan_id)
                .is_ok_and(|s| s.status == "complete" && s.issues == 0)
        {
            if let Some(changed) = job
                .journal_probe
                .as_ref()
                .and_then(|p| p.changed_parents.clone())
                .or_else(|| journal::changed_parents(&job.root, &old.checkpoint, current).ok())
            {
                match crate::incremental::seed(&store, &old.scan_id, &job.scan_id, &root, &changed)
                {
                    Ok(count) => {
                        reused = true;
                        scan.mode = if count == 0 {
                            "USN 验证复用"
                        } else {
                            "USN 增量扫描"
                        }
                        .into();
                        scan.message =
                            format!("现有 Journal 校验通过，重新核对 {count} 个变化子树");
                    }
                    Err(_) => {
                        store
                            .connection()?
                            .execute("DELETE FROM entries WHERE scan_id=?1", [&job.scan_id])?;
                        scan.message = "变化目录无法可靠复用，回退完整扫描".into();
                    }
                }
            }
        }
    }
    if !reused {
        scan.mode = "完整扫描".into();
        let mut conn = store.connection()?;
        Store::insert_batch(&mut conn, &job.scan_id, &[root])?;
    }
    {
        let mut conn = store.connection()?;
        let concurrency = filesystem::volumes()
            .iter()
            .find(|v| within(&job.root, &v.path))
            .map(|v| if v.removable { 1 } else { 4 })
            .unwrap_or(1);
        let mut last = Instant::now();
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let pending = store.pending(&job.scan_id, concurrency)?;
            if pending.is_empty() {
                break;
            }
            let (sender, receiver) = mpsc::sync_channel(512);
            std::thread::scope(|scope| -> Result<()> {
                for directory in pending {
                    let sender = sender.clone();
                    let cancel = cancel.clone();
                    scope.spawn(move || {
                        let result = filesystem::enumerate(Path::new(&directory.path), |file| {
                            if cancel.load(Ordering::Relaxed) {
                                return Ok(false);
                            }
                            Ok(sender.send(Event::Entry(file)).is_ok())
                        });
                        let error = if cancel.load(Ordering::Relaxed) {
                            Some("扫描已取消，目录不完整".into())
                        } else {
                            result.err().map(|e| format!("{e:#}"))
                        };
                        let _ = sender.send(Event::Done(directory.id, error));
                    });
                }
                drop(sender);
                let mut batch = Vec::with_capacity(512);
                let mut done = Vec::new();
                for event in receiver {
                    match event {
                        Event::Entry(mut f) => {
                            // NTFS directory-entry timestamps can lag the directory handle's
                            // timestamps after a child was created. Use one handle read per
                            // directory so later safety fingerprints compare the same source.
                            if f.is_dir
                                && f.attributes
                                    & (filesystem::REPARSE
                                        | filesystem::OFFLINE
                                        | filesystem::RECALL)
                                    == 0
                            {
                                match filesystem::inspect(Path::new(&f.path)) {
                                    Ok(fresh) if fresh.identity == f.identity => f = fresh,
                                    _ => {
                                        f.issue = Some("目录身份变化或元数据无法读取".into());
                                        f.enumerated = true;
                                        f.complete = false;
                                        scan.issues += 1;
                                    }
                                }
                            }
                            f.assessment = rules.classify_indexed(&f, &policy, &apps);
                            if f.is_dir {
                                scan.directories += 1;
                            } else {
                                scan.files += 1;
                                scan.logical_bytes =
                                    scan.logical_bytes.saturating_add(f.logical_bytes);
                                scan.allocated_bytes = scan
                                    .allocated_bytes
                                    .saturating_add(f.allocated_bytes.unwrap_or(0));
                            }
                            if f.is_dir {
                                f.complete = false;
                            }
                            let own_data = job
                                .database
                                .parent()
                                .is_some_and(|p| within(&f.path, &p.to_string_lossy()));
                            if f.attributes
                                & (filesystem::REPARSE | filesystem::OFFLINE | filesystem::RECALL)
                                != 0
                                || own_data
                            {
                                f.enumerated = true;
                                f.complete = false;
                                f.issue = Some(
                                    if own_data {
                                        "应用自身索引目录不展开"
                                    } else {
                                        "链接、挂载点或云占位项不展开"
                                    }
                                    .into(),
                                );
                                scan.issues += 1;
                            }
                            batch.push(f);
                            if batch.len() >= 512 {
                                Store::insert_batch(&mut conn, &job.scan_id, &batch)?;
                                batch.clear();
                            }
                        }
                        Event::Done(id, error) => done.push((id, error)),
                    }
                    if last.elapsed() >= Duration::from_millis(250) {
                        store.save_scan(&scan)?;
                        progress(&scan);
                        last = Instant::now();
                    }
                }
                if !batch.is_empty() {
                    Store::insert_batch(&mut conn, &job.scan_id, &batch)?;
                }
                for (id, error) in done {
                    store.finish_directory(&job.scan_id, id, error)?;
                }
                Ok(())
            })?;
        }
    }
    if cancel.load(Ordering::Relaxed) {
        scan.status = "cancelled".into();
        scan.finished = Some(chrono::Utc::now().timestamp());
        scan.message = "扫描已取消，索引不完整，不能据此清理".into();
        store.save_scan(&scan)?;
        progress(&scan);
        return Ok(());
    }
    scan.status = "aggregating".into();
    scan.message = "汇总目录、硬链接和不完整区域".into();
    store.save_scan(&scan)?;
    progress(&scan);
    store.aggregate(&job.scan_id)?;
    // Re-evaluate rules using descendant activity, with bounded result pages.
    let mut after = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let page = store.page_after(&job.scan_id, after, !reused)?;
        if page.is_empty() {
            break;
        }
        let mut updates = Vec::new();
        for f in page {
            after = f.id;
            if normalize(&f.path) == normalize(&job.root) {
                continue;
            }
            let mut a = rules.classify_indexed(&f, &policy, &apps);
            if !f.complete || f.has_blocked_children {
                a.risk = "protected".into();
                a.protected_reason = Some(
                    if !f.complete {
                        "扫描不完整，不能安全清理整个目标"
                    } else {
                        "目录内包含受保护对象，请逐层查看"
                    }
                    .into(),
                );
            }
            updates.push((f.id, a));
        }
        store.update_assessments(&updates)?;
    }
    store.aggregate(&job.scan_id)?;
    let (files, dirs, issues) = store.stats(&job.scan_id)?;
    scan.files = files;
    scan.directories = dirs;
    scan.issues = issues;
    let root = store.by_path(&job.scan_id, &job.root)?;
    let mut root_assessment = root.assessment.clone();
    root_assessment.risk = "protected".into();
    root_assessment.protected_reason = Some("扫描根目录不能整体清理".into());
    store.update_assessment(root.id, &root_assessment)?;
    scan.logical_bytes = root.logical_bytes;
    scan.allocated_bytes = root.allocated_bytes.unwrap_or(scan.allocated_bytes);
    scan.finished = Some(chrono::Utc::now().timestamp());
    scan.status = if cancel.load(Ordering::Relaxed) {
        "cancelled"
    } else {
        "complete"
    }
    .into();
    scan.message = if scan.status == "cancelled" {
        "扫描已取消，不能据此执行清理".into()
    } else {
        format!(
            "扫描完成；{} 个不完整区域。最后访问时间只作为弱参考。",
            issues
        )
    };
    store.save_scan(&scan)?;
    if scan.status == "complete" {
        if let Some(checkpoint) = checkpoint {
            store.put(
                &journal_key,
                &SnapshotJournal {
                    scan_id: job.scan_id,
                    root_identity: root.identity,
                    checkpoint,
                },
            )?;
        }
    }
    progress(&scan);
    Ok(())
}

pub fn create_scan(store: &Store, root: &str) -> Result<Scan> {
    let root = filesystem::validate_local_path(root).context("扫描路径不可用")?;
    let scan = Scan {
        id: uuid::Uuid::new_v4().to_string(),
        root: root.to_string_lossy().into_owned(),
        started: chrono::Utc::now().timestamp(),
        status: "queued".into(),
        mode: "完整扫描".into(),
        ..Default::default()
    };
    store.save_scan(&scan)?;
    Ok(scan)
}
