use crate::{rules::RuleSet, safety::SafetyPolicy, store::Store};
use anyhow::{Context, Result};
use cleaner_domain::*;
use cleaner_platform::{filesystem, inventory, journal, normalize};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
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
mod walk;

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
    let checkpoint = if job.settings.enhanced_scan {
        job.journal_probe
            .as_ref()
            .map(|p| p.checkpoint.clone())
            .or_else(|| journal::checkpoint(&job.root).ok())
    } else {
        None
    };
    let journal_key = format!("journal:{}", normalize(&job.root));
    let previous: Option<SnapshotJournal> = if job.settings.enhanced_scan {
        store.get(&journal_key)?
    } else {
        None
    };
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
    walk::Walker {
        store: &store,
        rules: &rules,
        policy: &policy,
        apps: &apps,
        cancel: &cancel,
        own_data: job
            .database
            .parent()
            .map(|p| p.to_string_lossy().into_owned()),
    }
    .run(&mut scan, &mut progress)?;
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
    if !store.aggregate_cancellable(&job.scan_id, &cancel)? {
        return finish_cancelled(&store, &mut scan, &mut progress);
    }
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
    if cancel.load(Ordering::Relaxed) || !store.propagate_protection(&job.scan_id, &cancel)? {
        return finish_cancelled(&store, &mut scan, &mut progress);
    }
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

fn finish_cancelled(
    store: &Store,
    scan: &mut Scan,
    progress: &mut impl FnMut(&Scan),
) -> Result<()> {
    scan.status = "cancelled".into();
    scan.finished = Some(chrono::Utc::now().timestamp());
    scan.message = "扫描已取消，索引不完整，不能据此清理".into();
    store.save_scan(scan)?;
    progress(scan);
    Ok(())
}
