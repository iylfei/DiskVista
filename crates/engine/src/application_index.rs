use crate::{
    application_origins::{self, ApplicationOrigin, OriginKind},
    safety::SafetyPolicy,
    store::Store,
};
use anyhow::Result;
use cleaner_domain::{FileRecord, InstalledApp};
use cleaner_platform::normalize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone)]
pub struct ApplicationIndex {
    apps: Vec<InstalledApp>,
    roots: HashMap<String, Vec<usize>>,
    names: HashMap<String, Vec<usize>>,
    origins: BTreeMap<String, ApplicationOrigin>,
}

impl ApplicationIndex {
    pub fn new(apps: &[InstalledApp], policy: &SafetyPolicy) -> Self {
        let mut roots: HashMap<String, Vec<usize>> = HashMap::new();
        let mut names: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, app) in apps.iter().enumerate() {
            if policy.specific_install_root(&app.install_location) {
                roots
                    .entry(normalize(&app.install_location))
                    .or_default()
                    .push(i);
            }
            let matches = names.entry(app.name.to_lowercase()).or_default();
            if matches.len() < 3 {
                matches.push(i);
            }
        }
        Self {
            apps: apps.to_vec(),
            roots,
            names,
            origins: application_origins::from_inventory(apps, policy),
        }
    }

    pub fn for_scan(store: &Store, scan: &str, policy: &SafetyPolicy) -> Result<Self> {
        Self::with_snapshot(store, scan, &store.apps(scan)?, policy)
    }

    pub fn with_snapshot(
        store: &Store,
        scan: &str,
        apps: &[InstalledApp],
        policy: &SafetyPolicy,
    ) -> Result<Self> {
        let mut index = Self::new(apps, policy);
        application_origins::add_snapshot_layout(store, scan, policy, &mut index)?;
        Ok(index)
    }

    fn matching<'a>(&'a self, key: &'a str) -> impl Iterator<Item = usize> + 'a {
        std::iter::once(key)
            .chain(key.match_indices('\\').map(|(i, _)| &key[..i]))
            .filter_map(|ancestor| self.roots.get(ancestor))
            .flat_map(|indices| indices.iter().copied())
    }

    pub fn owner(&self, normalized_path: &str) -> Option<&InstalledApp> {
        self.matching(normalized_path)
            .max_by_key(|&i| {
                (
                    normalize(&self.apps[i].install_location).len(),
                    application_origins::inventory_priority(&self.apps[i]),
                )
            })
            .map(|i| &self.apps[i])
    }

    pub fn origin(&self, normalized_path: &str) -> Option<&ApplicationOrigin> {
        std::iter::once(normalized_path)
            .chain(
                normalized_path
                    .match_indices('\\')
                    .rev()
                    .map(|(i, _)| &normalized_path[..i]),
            )
            .find_map(|path| self.origins.get(path))
    }

    pub fn origins(&self) -> impl Iterator<Item = &ApplicationOrigin> {
        self.origins.values()
    }

    pub fn installation_roots(&self) -> impl Iterator<Item = &str> {
        self.roots.keys().map(String::as_str)
    }

    pub fn named_origin(&self, name: &str) -> Option<&ApplicationOrigin> {
        let matches: Vec<_> = self
            .origins
            .values()
            .filter(|origin| origin.name.eq_ignore_ascii_case(name))
            .collect();
        let identities: BTreeSet<_> = matches
            .iter()
            .map(|origin| &origin.application_key)
            .collect();
        if identities.len() == 1 {
            matches.into_iter().max_by_key(|origin| {
                (
                    origin.kind == OriginKind::Installation,
                    application_origins::confidence_rank(origin.confidence),
                )
            })
        } else {
            None
        }
    }

    pub(super) fn remove_origin(&mut self, path: &str) {
        self.origins.remove(&normalize(path));
    }

    pub(super) fn add_origin(&mut self, origin: ApplicationOrigin) {
        application_origins::insert_origin(&mut self.origins, origin);
    }

    pub fn named(&self, name: &str) -> impl Iterator<Item = &InstalledApp> {
        self.names
            .get(&name.to_lowercase())
            .into_iter()
            .flatten()
            .map(|&i| &self.apps[i])
    }

    pub fn installed_reason(&self, file: &FileRecord) -> Option<String> {
        // Protection previously used the first matching installer record, not the deepest.
        self.matching(&normalize(&file.path))
            .min()
            .map(|i| format!("属于已安装程序 {}，请从应用设置管理", self.apps[i].name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_matching_preserves_boundaries_and_protection() {
        let apps: Vec<_> = [
            ("broad", "D:\\"),
            ("outer", "D:\\Apps\\A"),
            ("inner", "D:\\Apps\\A\\bin"),
            ("duplicate", "D:\\Apps\\A\\bin"),
        ]
        .into_iter()
        .map(|(name, path)| InstalledApp {
            id: name.into(),
            name: name.into(),
            publisher: String::new(),
            install_location: path.into(),
            source: "test".into(),
            last_used: None,
        })
        .collect();
        let policy = SafetyPolicy::new(Default::default());
        let index = ApplicationIndex::new(&apps, &policy);
        for path in [
            "D:\\Apps\\A\\bin\\x.dll",
            "d:/apps/a",
            "D:\\Apps\\AB\\x",
            "D:\\personal",
        ] {
            let file = FileRecord {
                path: path.into(),
                ..Default::default()
            };
            let expected = apps
                .iter()
                .filter(|a| {
                    policy.specific_install_root(&a.install_location)
                        && cleaner_platform::within(path, &a.install_location)
                })
                .max_by_key(|a| {
                    (
                        normalize(&a.install_location).len(),
                        application_origins::inventory_priority(a),
                    )
                });
            assert_eq!(
                index.owner(&normalize(path)).map(|a| &a.id),
                expected.map(|a| &a.id)
            );
            assert_eq!(
                index.installed_reason(&file),
                policy.installed_reason(&file, &apps)
            );
        }
    }

    #[test]
    fn duplicate_installation_record_selection_is_shared_and_order_independent() {
        let mut apps: Vec<_> = [
            ("Shortcut App", "开始菜单快捷方式"),
            ("Registered App", "Windows 卸载清单"),
        ]
        .into_iter()
        .map(|(name, source)| InstalledApp {
            id: name.into(),
            name: name.into(),
            publisher: String::new(),
            install_location: "D:\\Programs\\Example".into(),
            source: source.into(),
            last_used: None,
        })
        .collect();
        let policy = SafetyPolicy::new(Default::default());
        for _ in 0..2 {
            let index = ApplicationIndex::new(&apps, &policy);
            let path = normalize("D:\\Programs\\Example\\data.bin");
            assert_eq!(index.owner(&path).unwrap().name, "Registered App");
            assert_eq!(index.origin(&path).unwrap().name, "Registered App");
            apps.reverse();
        }
    }

    #[test]
    fn precise_msix_family_data_inherits_without_becoming_an_installation() {
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let app = InstalledApp {
            id: "Example.Product_123abc".into(),
            name: "Example Product".into(),
            publisher: String::new(),
            install_location: "C:\\Program Files\\WindowsApps\\Example.Product_1.0_x64__123abc"
                .into(),
            source: "Windows MSIX 包清单".into(),
            last_used: None,
        };
        let policy = SafetyPolicy::new(Default::default());
        let index = ApplicationIndex::new(&[app], &policy);
        let path = normalize(&format!(
            "{local}\\Packages\\Example.Product_123abc\\LocalState\\file.bin"
        ));
        let origin = index.origin(&path).unwrap();
        assert_eq!(origin.name, "Example Product");
        assert_eq!(origin.confidence, "high");
        assert_eq!(origin.kind, OriginKind::ApplicationData);
        assert!(index.owner(&path).is_none());
        assert!(index
            .origin(&normalize(&format!(
                "{local}\\Packages\\Example.Product_123abc-other\\file.bin"
            )))
            .is_none());
    }

    #[test]
    fn ambiguous_data_aliases_do_not_join_independent_installations() {
        let apps: Vec<_> = ["D:\\One\\ExampleEditor", "D:\\Two\\ExampleEditor"]
            .into_iter()
            .map(|path| InstalledApp {
                id: path.into(),
                name: "ExampleEditor".into(),
                publisher: String::new(),
                install_location: path.into(),
                source: "test".into(),
                last_used: None,
            })
            .collect();
        let index = ApplicationIndex::new(&apps, &SafetyPolicy::new(Default::default()));
        let local = std::env::var("LOCALAPPDATA").unwrap();
        assert!(index
            .origin(&normalize(&format!("{local}\\ExampleEditor\\a.bin")))
            .is_none());
        assert!(index.named_origin("ExampleEditor").is_none());
    }

    #[test]
    fn multiple_msix_versions_share_family_data_without_picking_an_installation() {
        let apps: Vec<_> = ["1.0", "2.0"]
            .into_iter()
            .map(|version| InstalledApp {
                id: "Example.Product_123abc".into(),
                name: "Example Product".into(),
                publisher: String::new(),
                install_location: format!(
                    "C:\\Program Files\\WindowsApps\\Example.Product_{version}_x64__123abc"
                ),
                source: "Windows MSIX 包清单".into(),
                last_used: None,
            })
            .collect();
        let index = ApplicationIndex::new(&apps, &SafetyPolicy::new(Default::default()));
        let path = normalize(&format!(
            "{}\\Packages\\Example.Product_123abc\\LocalState\\file.bin",
            std::env::var("LOCALAPPDATA").unwrap()
        ));
        let origin = index.origin(&path).unwrap();
        assert_eq!(origin.name, "Example Product");
        assert_eq!(origin.confidence, "high");
        assert_eq!(origin.application_key, "msix_data:Example.Product_123abc");
        assert!(index.named_origin("Example Product").is_none());
    }
}
