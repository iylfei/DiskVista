use crate::{
    application_index::ApplicationIndex, rules::RuleSet, safety::SafetyPolicy, store::Store,
};
use anyhow::{bail, ensure, Result};
use cleaner_platform::normalize;

pub fn apply(store: &Store, scan_id: &str, entry_id: i64, kind: &str, value: &str) -> Result<()> {
    let scan = store.require_finished(scan_id)?;
    let target = store.entry(scan_id, entry_id)?;
    let mut settings = store.settings()?;
    let paths = match kind {
        "protect" => {
            settings
                .unprotected_paths
                .retain(|p| normalize(p) != normalize(&target.path));
            Some(&mut settings.protected_paths)
        }
        "unprotect" => {
            ensure!(
                normalize(&target.path) != normalize(&scan.root),
                "扫描根目录不能解除保护"
            );
            settings
                .protected_paths
                .retain(|p| normalize(p) != normalize(&target.path));
            Some(&mut settings.unprotected_paths)
        }
        "ignore" => Some(&mut settings.ignored_paths),
        "exclude_llm" => Some(&mut settings.excluded_llm_paths),
        "label" => {
            ensure!(
                !value.trim().is_empty() && value.len() <= 300,
                "标注长度无效"
            );
            settings.labels.insert(target.path.clone(), value.into());
            None
        }
        _ => bail!("未知标注操作"),
    };
    if let Some(paths) = paths {
        if !paths
            .iter()
            .any(|p| normalize(p) == normalize(&target.path))
        {
            paths.push(target.path.clone());
        }
    }
    store.put("settings", &settings)?;
    if kind == "exclude_llm" {
        return Ok(());
    }
    let rules = RuleSet::load(settings.community_enabled)?;
    let policy = SafetyPolicy::new(settings);
    let apps = ApplicationIndex::new(&store.apps(scan_id)?, &policy);
    let mut after = String::new();
    loop {
        let files = store.subtree_page(scan_id, &target.path, &after)?;
        if files.is_empty() {
            break;
        }
        let mut updates = Vec::with_capacity(files.len());
        for file in files {
            after = normalize(&file.path);
            let mut assessment = rules.classify_indexed(&file, &policy, &apps);
            if normalize(&file.path) == normalize(&scan.root) {
                assessment.risk = "protected".into();
                assessment.protected_reason = Some("扫描根目录不能整体清理".into());
            }
            updates.push((file.id, assessment));
        }
        store.update_assessments(&updates)?;
    }
    store.refresh_subtree_protection(scan_id, &target.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::*;

    #[test]
    fn annotations_cross_pages_keep_siblings_and_propagate_protection() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("index.db")).unwrap();
        store
            .save_scan(&Scan {
                id: "s".into(),
                root: "D:\\fixture".into(),
                status: "complete".into(),
                ..Default::default()
            })
            .unwrap();
        let mut files: Vec<_> = [
            "D:\\fixture",
            "D:\\fixture\\cache",
            "D:\\fixture\\cache-copy",
        ]
        .into_iter()
        .map(|path| FileRecord {
            path: path.into(),
            parent: path.rsplit_once('\\').unwrap().0.into(),
            is_dir: true,
            complete: true,
            enumerated: true,
            assessment: Assessment {
                risk: "review".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .collect();
        files.extend((0..1025).map(|i| FileRecord {
            path: format!("D:\\fixture\\cache\\{i:04}"),
            parent: "D:\\fixture\\cache".into(),
            complete: true,
            enumerated: true,
            logical_bytes: 1,
            allocated_bytes: Some(1),
            file_count: 1,
            assessment: Assessment {
                risk: "review".into(),
                ..Default::default()
            },
            ..Default::default()
        }));
        Store::insert_batch(&mut store.connection().unwrap(), "s", &files).unwrap();
        store.aggregate("s").unwrap();
        let target = store.by_path("s", "D:\\fixture\\cache").unwrap();
        apply(&store, "s", target.id, "exclude_llm", "").unwrap();
        assert_eq!(
            store.by_path("s", &target.path).unwrap().assessment.risk,
            "review"
        );
        apply(&store, "s", target.id, "protect", "").unwrap();
        assert!(store
            .descendants("s", &target.path)
            .unwrap()
            .iter()
            .all(|f| f.assessment.risk == "protected"));
        assert_eq!(
            store
                .by_path("s", "D:\\fixture\\cache-copy")
                .unwrap()
                .assessment
                .risk,
            "review"
        );
        let root = store.by_path("s", "D:\\fixture").unwrap();
        assert!(root.has_blocked_children);
        assert_eq!(root.logical_bytes, 1025);
        apply(&store, "s", target.id, "protect", "").unwrap();
        assert_eq!(store.settings().unwrap().protected_paths.len(), 1);

        apply(&store, "s", target.id, "unprotect", "").unwrap();
        let settings = store.settings().unwrap();
        assert!(settings.protected_paths.is_empty());
        assert_eq!(settings.unprotected_paths, vec![target.path]);
        assert!(store
            .descendants("s", "D:\\fixture\\cache")
            .unwrap()
            .iter()
            .all(|f| f.assessment.risk != "protected"));
    }
}
