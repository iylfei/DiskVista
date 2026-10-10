use super::{decode, Store, FIELDS};
use crate::{
    application_index::ApplicationIndex, classified_query::Classifier, rules::RuleSet,
    safety::SafetyPolicy,
};
use anyhow::Result;
use cleaner_domain::{FileRecord, Scan};
use cleaner_platform::normalize;
use rusqlite::params;

const POLICY_VERSION: &str = "system-critical-v1";

impl Store {
    /// Upgrade cached protections before exposing snapshots at startup. Each scan is atomic;
    /// a failed upgrade can be retried safely without trusting old ancestor flags.
    pub fn refresh_protection_policy(&self) -> Result<()> {
        if self.get::<String>("protection-policy-version")?.as_deref() == Some(POLICY_VERSION) {
            return Ok(());
        }
        let policy = SafetyPolicy::new(self.settings()?);
        let rules = RuleSet::load(policy.settings.community_enabled)?;
        let scans: Vec<Scan> = {
            let connection = self.connection()?;
            let mut statement = connection.prepare("SELECT data FROM scans")?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|row| Ok(serde_json::from_str(&row?)?))
                .collect::<Result<_>>()?;
            rows
        };
        for scan in scans {
            let apps = ApplicationIndex::for_scan(self, &scan.id, &policy)?;
            let classifier = Classifier::new(&scan, &rules, &policy, &apps);
            let mut connection = self.connection()?;
            let tx = connection.transaction()?;
            {
                // Path order visits children before parents, including across page boundaries.
                let mut entries = tx.prepare(&format!(
                    "SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND path_key<COALESCE(?2,char(1114111)) AND (risk='protected' OR blocked=1 OR is_dir=1) ORDER BY path_key DESC LIMIT 512"
                ))?;
                let mut children = tx.prepare_cached(
                    "SELECT COALESCE(MAX(blocked OR risk='protected'),0) FROM entries WHERE scan_id=?1 AND parent_key=?2 AND path_key<>?2"
                )?;
                let mut update = tx.prepare_cached(
                    "UPDATE entries SET blocked=?2,risk=?3,owner=?4,rule_id=?5,assessment=?6 WHERE id=?1"
                )?;
                let mut after: Option<String> = None;
                loop {
                    let rows: Vec<FileRecord> = entries
                        .query_map(params![scan.id, after], decode)?
                        .collect::<rusqlite::Result<_>>()?;
                    if rows.is_empty() {
                        break;
                    }
                    after = rows.last().map(|file| normalize(&file.path));
                    for mut file in rows {
                        file.has_blocked_children = file.is_dir
                            && children
                                .query_row(params![scan.id, normalize(&file.path)], |row| {
                                    row.get::<_, bool>(0)
                                })?;
                        classifier.apply(&mut file);
                        update.execute(params![
                            file.id,
                            file.has_blocked_children,
                            file.assessment.risk,
                            file.assessment.owner,
                            file.assessment.rule_id,
                            serde_json::to_string(&file.assessment)?
                        ])?;
                    }
                }
            }
            tx.commit()?;
        }
        self.put("protection-policy-version", &POLICY_VERSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::{Assessment, Settings};

    fn old_entry(path: &str, directory: bool) -> FileRecord {
        FileRecord {
            path: path.into(),
            parent: path
                .rsplit_once('\\')
                .map(|(parent, _)| parent)
                .unwrap_or("")
                .into(),
            name: path.rsplit('\\').next().unwrap().into(),
            is_dir: directory,
            complete: true,
            enumerated: true,
            has_blocked_children: directory,
            assessment: Assessment {
                risk: "protected".into(),
                protected_reason: Some("旧保护规则".into()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn cached_app_protection_is_removed_while_system_manual_and_incomplete_stay_blocked() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("protection.sqlite")).unwrap();
        let scan = Scan {
            id: "s".into(),
            root: "C:\\".into(),
            status: "complete".into(),
            ..Default::default()
        };
        store.save_scan(&scan).unwrap();
        let user = "C:\\Users\\15830\\AppData\\Roaming\\Kiro\\User";
        let database = format!("{user}\\docs.sqlite");
        let mut entries = vec![
            old_entry("C:\\", true),
            old_entry(user, true),
            old_entry(&database, false),
            old_entry("C:\\Windows", true),
            old_entry("C:\\Windows\\System32\\drivers\\disk.sys", false),
            old_entry("C:\\manual", true),
            old_entry("C:\\manual\\keep.db", false),
            old_entry("C:\\unfinished", true),
        ];
        entries.last_mut().unwrap().complete = false;
        let installed = std::env::var("ProgramFiles").unwrap();
        let app = format!("{installed}\\ExampleApp");
        entries.push(old_entry(&app, true));
        entries.push(old_entry(&format!("{app}\\app.exe"), false));
        store
            .save_apps(
                "s",
                &[cleaner_domain::InstalledApp {
                    id: "app".into(),
                    name: "ExampleApp".into(),
                    publisher: String::new(),
                    install_location: app.clone(),
                    source: "fixture".into(),
                    last_used: None,
                }],
            )
            .unwrap();
        let mut settings = Settings::default();
        settings.protected_paths.push("C:\\manual".into());
        store.put("settings", &settings).unwrap();
        Store::insert_batch(&mut store.connection().unwrap(), "s", &entries).unwrap();
        store.refresh_protection_policy().unwrap();
        for path in [user, &database, &app] {
            let file = store.by_path("s", path).unwrap();
            assert_eq!(file.assessment.risk, "review", "{path}");
            assert!(file.assessment.protected_reason.is_none(), "{path}");
            assert!(!file.has_blocked_children, "{path}");
        }
        for path in [
            "C:\\",
            "C:\\Windows",
            "C:\\manual",
            "C:\\manual\\keep.db",
            "C:\\unfinished",
        ] {
            assert_eq!(
                store.by_path("s", path).unwrap().assessment.risk,
                "protected",
                "{path}"
            );
        }
        let observer = super::super::RevisionObserver::new(store.clone());
        let revision = observer.revision().unwrap();
        store.refresh_protection_policy().unwrap();
        assert_eq!(observer.revision().unwrap(), revision);
    }

    #[test]
    fn protection_refresh_visits_descendants_before_ancestors_across_pages() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("paged.sqlite")).unwrap();
        store
            .save_scan(&Scan {
                id: "s".into(),
                root: "D:\\".into(),
                ..Default::default()
            })
            .unwrap();
        let mut entries = vec![old_entry("D:\\Kiro\\User", true)];
        entries
            .extend((0..600).map(|i| old_entry(&format!("D:\\Kiro\\User\\{i:04}.sqlite"), false)));
        Store::insert_batch(&mut store.connection().unwrap(), "s", &entries).unwrap();
        store.refresh_protection_policy().unwrap();
        let parent = store.by_path("s", "D:\\Kiro\\User").unwrap();
        assert!(!parent.has_blocked_children);
        assert_eq!(parent.assessment.risk, "review");
    }
}
