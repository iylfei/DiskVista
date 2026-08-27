use crate::safety::SafetyPolicy;
use cleaner_domain::{FileRecord, InstalledApp};
use cleaner_platform::normalize;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ApplicationIndex {
    apps: Vec<InstalledApp>,
    roots: HashMap<String, Vec<usize>>,
    names: HashMap<String, Vec<usize>>,
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
        }
    }

    fn matching<'a>(&'a self, key: &'a str) -> impl Iterator<Item = usize> + 'a {
        std::iter::once(key)
            .chain(key.match_indices('\\').map(|(i, _)| &key[..i]))
            .filter_map(|ancestor| self.roots.get(ancestor))
            .flat_map(|indices| indices.iter().copied())
    }

    pub fn owner(&self, normalized_path: &str) -> Option<&InstalledApp> {
        // Preserve max_by_key's last-record tie break, including duplicate installer records.
        self.matching(normalized_path)
            .max_by_key(|&i| (self.apps[i].install_location.len(), i))
            .map(|i| &self.apps[i])
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
    fn indexed_matching_preserves_boundaries_duplicates_and_protection() {
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
                .max_by_key(|a| a.install_location.len());
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
}
