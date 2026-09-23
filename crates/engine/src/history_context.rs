use crate::{context::redact_path, safety::SafetyPolicy, store::Store};
use anyhow::Result;
use cleaner_domain::{FileRecord, HistoryItem, HistoryReference, InstalledApp};
use cleaner_platform::{normalize, within};
use std::collections::HashSet;

pub const HISTORY_DAYS: i64 = 180;
pub const HISTORY_POOL_LIMIT: usize = 200;
pub const HISTORY_REFERENCE_LIMIT: usize = 6;
pub const MANUAL_HISTORY_REFERENCE_LIMIT: usize = 100;
pub const HISTORY_JSON_LIMIT: usize = 8 * 1024;

struct HistoricalEntry {
    item: HistoryItem,
    features: Features,
}

#[derive(Default)]
pub struct HistoryPool {
    entries: Vec<HistoricalEntry>,
    valid_until: Option<i64>,
    manual: bool,
}

impl HistoryPool {
    pub fn load(store: &Store, policy: &SafetyPolicy, apps: &[InstalledApp]) -> Result<Self> {
        if !policy.settings.llm.history_reference_enabled {
            return Ok(Self::default());
        }
        let now = chrono::Utc::now().timestamp();
        let manual = policy.settings.history_reference_ids.is_some();
        let rows = if let Some(ids) = &policy.settings.history_reference_ids {
            let connection = store.connection()?;
            let mut query = connection.prepare("SELECT data FROM history WHERE id=?1 AND time<=?2 AND json_extract(data,'$.status')='recycled'")?;
            let mut rows = Vec::new();
            for id in ids.iter().take(MANUAL_HISTORY_REFERENCE_LIMIT) {
                let mut found = query.query(rusqlite::params![id, now])?;
                if let Some(row) = found.next()? {
                    rows.push(serde_json::from_str::<HistoryItem>(
                        &row.get::<_, String>(0)?,
                    )?);
                }
            }
            rows
        } else {
            store.recent_recycled_history(now - HISTORY_DAYS * 86_400, now, HISTORY_POOL_LIMIT)?
        };
        let mut pool = Self::from_items_with_mode(rows, policy, apps, now, manual);
        let future: Option<i64> = store.connection()?.query_row(
            "SELECT MIN(time) FROM history WHERE time>?1 AND json_extract(data,'$.status')='recycled'", [now], |row| row.get(0))?;
        if let Some(future) = future {
            pool.valid_until = Some(pool.valid_until.map_or(future, |old| old.min(future)));
        }
        Ok(pool)
    }

    #[cfg(test)]
    fn from_items(
        rows: Vec<HistoryItem>,
        policy: &SafetyPolicy,
        apps: &[InstalledApp],
        now: i64,
    ) -> Self {
        Self::from_items_with_mode(rows, policy, apps, now, false)
    }

    fn from_items_with_mode(
        mut rows: Vec<HistoryItem>,
        policy: &SafetyPolicy,
        apps: &[InstalledApp],
        now: i64,
        manual: bool,
    ) -> Self {
        let valid_until = rows
            .iter()
            .map(|item| {
                item.time
                    .saturating_add(HISTORY_DAYS * 86_400)
                    .saturating_add(1)
            })
            .filter(|&time| time > now)
            .min()
            .filter(|_| !manual);
        rows.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| b.id.cmp(&a.id)));
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for item in rows
            .into_iter()
            .filter(|row| {
                row.status == "recycled"
                    && (0..=now).contains(&row.time)
                    && (manual || row.time >= now - HISTORY_DAYS * 86_400)
            })
            .take(HISTORY_POOL_LIMIT)
        {
            if item.id.is_empty() || item.id.len() > 128 || item.path.len() > 4096 {
                continue;
            }
            let path_key = normalize(&item.path);
            if !seen.insert(path_key.clone()) {
                continue;
            }
            let snapshot = item.snapshot.as_ref();
            let file = FileRecord {
                path: item.path.clone(),
                name: path_key.rsplit('\\').next().unwrap_or("").into(),
                is_dir: snapshot.is_some_and(|value| value.is_dir),
                complete: true,
                ..Default::default()
            };
            let may_be_directory = snapshot.is_none_or(|value| value.is_dir);
            let protected_overlap = !policy.is_unprotected(&file.path)
                && policy
                    .settings
                    .protected_paths
                    .iter()
                    .chain(&policy.settings.ignored_paths)
                    .chain(&policy.settings.excluded_llm_paths)
                    .chain(&policy.system_roots)
                    .chain(&policy.cloud_roots)
                    .any(|path| {
                        within(&file.path, path) || (may_be_directory && within(path, &file.path))
                    });
            if policy.reason(&file).is_some()
                || policy.installed_reason(&file, apps).is_some()
                || protected_overlap
                || (may_be_directory
                    && apps.iter().any(|app| {
                        policy.specific_install_root(&app.install_location)
                            && within(&app.install_location, &file.path)
                    }))
                || !is_absolute_local(&path_key)
            {
                continue;
            }
            let features = Features::new(
                &item.path,
                snapshot.and_then(|value| value.owner.as_deref()),
                snapshot.map(|value| value.category.as_str()),
                snapshot.map(|value| value.is_dir),
            );
            entries.push(HistoricalEntry { item, features });
        }
        Self {
            entries,
            valid_until,
            manual,
        }
    }

    pub fn references_truncated(&self, references: &[HistoryReference]) -> bool {
        self.manual && references.len() < self.entries.len()
    }

    pub fn valid_until(&self) -> Option<i64> {
        self.valid_until
    }

    pub fn relevance(&self, file: &FileRecord) -> u32 {
        let features = Features::from_file(file);
        self.entries
            .iter()
            .map(|entry| similarity(&features, &entry.features).0)
            .max()
            .unwrap_or(0)
    }

    pub fn references(&self, file: &FileRecord) -> Vec<HistoryReference> {
        let features = Features::from_file(file);
        let mut ranked: Vec<_> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let (score, mut basis) = similarity(&features, &entry.features);
                if score == 0 && self.manual {
                    basis.push("手动选择的回收历史，未发现与当前文件明确匹配的线索".into());
                }
                (score > 0 || self.manual).then_some((score, basis, entry))
            })
            .collect();
        if self.manual {
            ranked.sort_by(|a, b| {
                b.2.item
                    .time
                    .cmp(&a.2.item.time)
                    .then_with(|| a.2.item.id.cmp(&b.2.item.id))
            });
        } else {
            ranked.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| b.2.item.time.cmp(&a.2.item.time))
                    .then_with(|| a.2.item.id.cmp(&b.2.item.id))
            });
        }
        let mut references = Vec::new();
        for (_, basis, entry) in ranked {
            let snapshot = entry.item.snapshot.as_ref();
            let reference = HistoryReference {
                id: format!("history:{}", entry.item.id),
                path: redact_path(&entry.item.path),
                bytes: entry.item.bytes,
                recycled_at: entry.item.time,
                owner: snapshot.and_then(|value| bounded_label(value.owner.as_deref())),
                category: snapshot
                    .and_then(|value| bounded_label(Some(&value.category)))
                    .filter(|value| !value.is_empty() && value != "unknown"),
                match_basis: basis,
            };
            references.push(reference);
            if serde_json::to_vec(&references).map_or(true, |json| json.len() > HISTORY_JSON_LIMIT)
            {
                references.pop();
                continue;
            }
            if references.len()
                == if self.manual {
                    MANUAL_HISTORY_REFERENCE_LIMIT
                } else {
                    HISTORY_REFERENCE_LIMIT
                }
            {
                break;
            }
        }
        references
    }
}

fn bounded_label(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty() && value.len() <= 300)
        .map(crate::context::redact_text)
}

fn is_absolute_local(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() > 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\'
}

struct Features {
    path: String,
    parent: String,
    family: String,
    extension: String,
    anchor: String,
    owner: Option<String>,
    category: Option<String>,
    is_dir: Option<bool>,
}

impl Features {
    fn from_file(file: &FileRecord) -> Self {
        Self::new(
            &file.path,
            file.assessment.owner.as_deref(),
            Some(&file.assessment.category),
            Some(file.is_dir),
        )
    }

    fn new(path: &str, owner: Option<&str>, category: Option<&str>, is_dir: Option<bool>) -> Self {
        let path = normalize(path);
        let (parent, name) = path.rsplit_once('\\').unwrap_or(("", &path));
        let (stem, extension) = if is_dir == Some(true) {
            (name, "")
        } else {
            name.rsplit_once('.').unwrap_or((name, ""))
        };
        let family = name_tokens(stem).join(" ");
        let parent_parts: Vec<_> = parent.split('\\').collect();
        let protected_prefix = if parent_parts.len() >= 3 && parent_parts[1] == "users" {
            3
        } else {
            1
        };
        let anchor = parent_parts
            .iter()
            .enumerate()
            .rev()
            .find(|(index, part)| *index >= protected_prefix && meaningful(part))
            .map(|(index, _)| parent_parts[..=index].join("\\"))
            .unwrap_or_default();
        let parent = parent.to_owned();
        let extension = extension.to_owned();
        Self {
            parent,
            path,
            family,
            extension,
            anchor,
            owner: owner
                .filter(|value| {
                    !matches!(
                        value.trim().to_lowercase().as_str(),
                        "" | "unknown" | "未知" | "未知来源" | "windows / 应用临时数据"
                    )
                })
                .map(|value| value.trim().to_lowercase()),
            category: category
                .filter(|value| {
                    !matches!(
                        *value,
                        "" | "unknown" | "application" | "application_container"
                    )
                })
                .map(str::to_owned),
            is_dir,
        }
    }
}

fn meaningful(value: &str) -> bool {
    value.chars().count() >= 3
        && value.chars().any(char::is_alphabetic)
        && !(value.len() >= 8 && value.chars().all(|c| c.is_ascii_hexdigit()))
        && !matches!(
            value,
            "users"
                | "appdata"
                | "local"
                | "roaming"
                | "locallow"
                | "programdata"
                | "programs"
                | "program files"
                | "program files (x86)"
                | "apps"
                | "cache"
                | "caches"
                | "log"
                | "logs"
                | "temp"
                | "tmp"
                | "backup"
                | "backups"
                | "downloads"
                | "download"
                | "documents"
                | "desktop"
                | "data"
                | "copy"
                | "old"
                | "new"
                | "update"
                | "setup"
                | "installer"
                | "release"
                | "x64"
                | "x86"
                | "win"
                | "windows"
                | "bin"
                | "debug"
                | "build"
                | "app"
                | "application"
                | "readme"
                | "index"
                | "config"
                | "settings"
                | "resources"
                | "packages"
                | "version"
                | "versions"
                | "lib"
                | "libs"
                | "files"
                | "content"
                | "assets"
                | "dist"
                | "out"
                | "obj"
                | "src"
                | "runtime"
                | "runtimes"
                | "node_modules"
                | "builds"
        )
}

fn name_tokens(value: &str) -> Vec<&str> {
    value
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| meaningful(token))
        .collect()
}

fn similarity(current: &Features, old: &Features) -> (u32, Vec<String>) {
    if current.path == old.path {
        return (120, vec!["相同路径曾成功回收".into()]);
    }
    if current.is_dir.zip(old.is_dir).is_some_and(|(a, b)| a != b) {
        return (0, vec![]);
    }
    let family_matches = !current.family.is_empty()
        && current.family == old.family
        && current.extension == old.extension;
    if current.parent.len() > 3 && current.parent == old.parent && family_matches {
        return (100, vec!["位于相同目录且名称特征相似".into()]);
    }
    if current.owner.is_some() && current.owner == old.owner {
        if current.category.is_some() && current.category == old.category {
            return (90, vec!["所属应用与用途分类相同".into()]);
        }
        if family_matches {
            return (85, vec!["所属应用相同且名称特征相似".into()]);
        }
    }
    if family_matches && !current.anchor.is_empty() && current.anchor == old.anchor {
        return (75, vec!["路径归属线索与名称特征相似".into()]);
    }
    (0, vec![])
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::{Assessment, HistoryEntrySnapshot, Settings};

    fn history(id: &str, path: &str, time: i64) -> HistoryItem {
        HistoryItem {
            id: id.into(),
            batch_id: "batch".into(),
            path: path.into(),
            bytes: 1024,
            time,
            status: "recycled".into(),
            message: String::new(),
            free_space_delta: 0,
            snapshot: None,
        }
    }
    fn file(path: &str) -> FileRecord {
        FileRecord {
            path: path.into(),
            complete: true,
            ..Default::default()
        }
    }

    #[test]
    fn default_history_remains_recent_and_relevant_while_manual_selection_can_include_old_unrelated_rows(
    ) {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(directory.path().join("history.sqlite")).unwrap();
        let now = chrono::Utc::now().timestamp();
        store
            .add_history(&history(
                "related",
                "D:\\HistoryFixture\\bundle-1.zip",
                now - 1,
            ))
            .unwrap();
        store
            .add_history(&history(
                "old",
                "D:\\OtherFixture\\holiday.zip",
                now - 181 * 86_400,
            ))
            .unwrap();
        let mut failed = history("failed", "D:\\OtherFixture\\failed.zip", now - 1);
        failed.status = "failed".into();
        store.add_history(&failed).unwrap();
        store
            .add_history(&history(
                "protected",
                "D:\\PrivateFixture\\secret.zip",
                now - 1,
            ))
            .unwrap();
        let mut settings = Settings::default();
        settings.llm.history_reference_enabled = true;
        settings.protected_paths.push("D:\\PrivateFixture".into());
        let target = file("D:\\HistoryFixture\\bundle-2.zip");
        let default = HistoryPool::load(&store, &SafetyPolicy::new(settings.clone()), &[]).unwrap();
        assert_eq!(
            default
                .references(&target)
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            vec!["history:related"]
        );
        settings.history_reference_ids =
            Some(vec!["old".into(), "failed".into(), "protected".into()]);
        let manual = HistoryPool::load(&store, &SafetyPolicy::new(settings.clone()), &[]).unwrap();
        let references = manual.references(&target);
        assert_eq!(
            references.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["history:old"]
        );
        assert!(references[0]
            .match_basis
            .iter()
            .any(|part| part.contains("未发现")));
        settings.history_reference_ids = Some(vec![]);
        assert!(HistoryPool::load(&store, &SafetyPolicy::new(settings), &[])
            .unwrap()
            .references(&target)
            .is_empty());
    }
    #[test]
    fn only_recent_successful_unprotected_relevant_history_is_used() {
        let now = 200 * 86_400;
        let mut settings = Settings::default();
        settings.excluded_llm_paths.push("D:\\Acme\\private".into());
        settings.protected_paths.push("D:\\Acme\\protected".into());
        let policy = SafetyPolicy::new(settings);
        let mut rows = vec![
            history("match", "D:\\Acme\\bundle-1.zip", now - 1),
            history("old", "D:\\Acme\\bundle-2.zip", 0),
            history("future", "D:\\Acme\\bundle-3.zip", now + 1),
            history("skip", "D:\\Acme\\bundle-4.zip", now - 2),
            history("fail", "D:\\Acme\\bundle-5.zip", now - 3),
            history("excluded", "D:\\Acme\\private\\bundle-6.zip", now - 4),
            history("protected", "D:\\Acme\\protected\\bundle-7.zip", now - 5),
            history("sensitive", "D:\\Acme\\.ssh\\bundle-8.zip", now - 6),
        ];
        rows[3].status = "skipped".into();
        rows[4].status = "failed".into();
        let pool = HistoryPool::from_items(rows, &policy, &[], now);
        let refs = pool.references(&file("D:\\Acme\\bundle-9.zip"));
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "history:match");
        assert!(pool.references(&file("D:\\Other\\holiday.zip")).is_empty());
    }
    #[test]
    fn repeated_paths_are_deduplicated_and_reference_count_and_bytes_are_bounded() {
        let now = 200 * 86_400;
        let policy = SafetyPolicy::new(Settings::default());
        let mut rows: Vec<_> = (0..20)
            .map(|i| {
                history(
                    &i.to_string(),
                    &format!("D:\\Acme\\bundle-{i}.zip"),
                    now - i,
                )
            })
            .collect();
        rows.push(history("repeat", "D:\\ACME\\bundle-0.zip", now - 21));
        let pool = HistoryPool::from_items(rows, &policy, &[], now);
        let refs = pool.references(&file("D:\\Acme\\bundle-100.zip"));
        assert_eq!(refs.len(), HISTORY_REFERENCE_LIMIT);
        assert!(!refs.iter().any(|r| r.id == "history:repeat"));
        assert!(serde_json::to_vec(&refs).unwrap().len() <= HISTORY_JSON_LIMIT);
        let rows: Vec<_> = (0..20)
            .map(|i| {
                history(
                    &i.to_string(),
                    &format!("D:\\{}\\Acme\\bundle-{i}.zip", "long".repeat(600)),
                    now - i,
                )
            })
            .collect();
        let refs = HistoryPool::from_items(rows, &policy, &[], now).references(&file(&format!(
            "D:\\{}\\Acme\\bundle-100.zip",
            "long".repeat(600)
        )));
        assert!(!refs.is_empty());
        assert!(refs.len() < HISTORY_REFERENCE_LIMIT);
        assert!(serde_json::to_vec(&refs).unwrap().len() <= HISTORY_JSON_LIMIT);
    }
    #[test]
    fn extension_alone_does_not_match_but_known_application_and_category_can() {
        let now = 200 * 86_400;
        let policy = SafetyPolicy::new(Settings::default());
        let mut row = history("h", "D:\\Example\\unrelated.zip", now);
        row.snapshot = Some(HistoryEntrySnapshot {
            name: "unrelated.zip".into(),
            is_dir: false,
            owner: Some("Example".into()),
            category: "cache".into(),
            rule_id: None,
        });
        let pool = HistoryPool::from_items(vec![row], &policy, &[], now);
        let mut current = file("D:\\Example\\holiday.zip");
        assert_eq!(pool.relevance(&current), 0);
        current.assessment = Assessment {
            owner: Some("Example".into()),
            category: "cache".into(),
            ..Default::default()
        };
        assert!(pool.relevance(&current) > 0);
        current.assessment.category = "personal".into();
        assert_eq!(pool.relevance(&current), 0);
    }
    #[test]
    fn shared_storage_root_does_not_link_unrelated_applications() {
        let now = 200 * 86_400;
        let row = history("h", "D:\\Apps\\Alpha\\cache\\productbundle-1.zip", now);
        let pool =
            HistoryPool::from_items(vec![row], &SafetyPolicy::new(Settings::default()), &[], now);
        assert_eq!(
            pool.relevance(&file("D:\\Apps\\Beta\\cache\\productbundle-2.zip")),
            0
        );
        assert!(pool.relevance(&file("D:\\Apps\\Alpha\\backup\\productbundle-2.zip")) > 0);
    }
    #[test]
    fn shared_temporary_owner_does_not_count_as_the_same_application() {
        let now = 200 * 86_400;
        let owner = "Windows / 应用临时数据";
        let mut row = history("h", "D:\\Temp\\Alpha\\payload-1.tmp", now);
        row.snapshot = Some(HistoryEntrySnapshot {
            name: "payload-1.tmp".into(),
            is_dir: false,
            owner: Some(owner.into()),
            category: "temporary".into(),
            rule_id: Some("user-temp".into()),
        });
        let pool =
            HistoryPool::from_items(vec![row], &SafetyPolicy::new(Settings::default()), &[], now);
        let mut current = file("D:\\Temp\\Beta\\payload-2.tmp");
        current.assessment.owner = Some(owner.into());
        current.assessment.category = "temporary".into();
        assert_eq!(pool.relevance(&current), 0);
        current.path = "D:\\Temp\\Alpha\\payload-2.tmp".into();
        assert!(pool.relevance(&current) > 0);
    }
    #[test]
    fn legacy_history_has_no_invented_application_snapshot() {
        let row: HistoryItem = serde_json::from_str(r#"{"id":"old","batchId":"b","path":"D:\\Example\\bundle-1.zip","bytes":1,"time":100,"status":"recycled","message":"","freeSpaceDelta":0}"#).unwrap();
        assert!(row.snapshot.is_none());
        let pool =
            HistoryPool::from_items(vec![row], &SafetyPolicy::new(Settings::default()), &[], 101);
        let refs = pool.references(&file("D:\\Example\\bundle-2.zip"));
        assert_eq!(refs.len(), 1);
        assert!(refs[0].owner.is_none());
    }
    #[test]
    fn unknown_old_directory_cannot_include_a_current_protected_descendant() {
        let now = 200 * 86_400;
        let mut settings = Settings::default();
        settings
            .excluded_llm_paths
            .push("D:\\Example\\bundle\\private".into());
        let row = history("old", "D:\\Example\\bundle", now);
        let pool = HistoryPool::from_items(vec![row], &SafetyPolicy::new(settings), &[], now);
        assert!(pool.references(&file("D:\\Example\\bundle")).is_empty());
    }
}
