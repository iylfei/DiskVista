use super::{generic_container, insert_origin, installation_key, ApplicationOrigin, OriginKind};
use cleaner_domain::{Evidence, InstalledApp};
use cleaner_platform::normalize;
use std::collections::{BTreeMap, BTreeSet};

struct KnownData {
    name: &'static str,
    aliases: &'static [&'static str],
    locations: &'static [(&'static str, &'static str)],
}

// These are attribution paths, not deletion rules. Shared vendor roots are excluded.
const KNOWN_DATA: &[KnownData] = &[
    KnownData {
        name: "Google Chrome",
        aliases: &["google chrome", "chrome"],
        locations: &[("LOCALAPPDATA", "Google\\Chrome\\User Data")],
    },
    KnownData {
        name: "Microsoft Edge",
        aliases: &["microsoft edge", "edge"],
        locations: &[("LOCALAPPDATA", "Microsoft\\Edge\\User Data")],
    },
    KnownData {
        name: "Mozilla Firefox",
        aliases: &["mozilla firefox", "firefox"],
        locations: &[
            ("APPDATA", "Mozilla\\Firefox"),
            ("LOCALAPPDATA", "Mozilla\\Firefox"),
        ],
    },
    KnownData {
        name: "Visual Studio Code",
        aliases: &["visual studio code", "microsoft visual studio code", "code"],
        locations: &[("APPDATA", "Code"), ("USERPROFILE", ".vscode")],
    },
    KnownData {
        name: "Cursor",
        aliases: &["cursor"],
        locations: &[("APPDATA", "Cursor"), ("USERPROFILE", ".cursor")],
    },
    KnownData {
        name: "Discord",
        aliases: &["discord"],
        locations: &[("APPDATA", "discord")],
    },
    KnownData {
        name: "Spotify",
        aliases: &["spotify"],
        locations: &[("APPDATA", "Spotify"), ("LOCALAPPDATA", "Spotify\\Storage")],
    },
    KnownData {
        name: "OBS Studio",
        aliases: &["obs studio", "obs-studio"],
        locations: &[("APPDATA", "obs-studio")],
    },
    KnownData {
        name: "npm",
        aliases: &["npm"],
        locations: &[("LOCALAPPDATA", "npm-cache"), ("APPDATA", "npm-cache")],
    },
    KnownData {
        name: "pip",
        aliases: &["pip"],
        locations: &[("LOCALAPPDATA", "pip\\Cache")],
    },
];

fn alias_key(value: &str) -> String {
    let value = value.trim().to_lowercase();
    value
        .trim_end_matches(" (user)")
        .trim_end_matches(" (system)")
        .trim_end_matches(" (x64)")
        .trim_end_matches(" (x86)")
        .to_owned()
}

fn aliases(app: &InstalledApp) -> BTreeSet<String> {
    let mut aliases = BTreeSet::from([alias_key(&app.name)]);
    let location = normalize(&app.install_location);
    if let Some(name) = location.rsplit('\\').next() {
        if !name.ends_with(".exe")
            && !generic_container(name)
            && !matches!(
                name,
                "app" | "application" | "bin" | "resources" | "uninstall"
            )
            && name.len() >= 4
            && name.chars().any(char::is_alphabetic)
        {
            aliases.insert(alias_key(name));
        }
    }
    aliases
}

fn location(environment: &str, relative: &str) -> Option<String> {
    let base = std::env::var(environment).ok()?;
    Some(normalize(&format!("{base}\\{relative}")))
}

pub(super) fn add(
    apps: &[InstalledApp],
    installations: &BTreeMap<String, &InstalledApp>,
    origins: &mut BTreeMap<String, ApplicationOrigin>,
) {
    let mut known_aliases = BTreeSet::new();
    for known in KNOWN_DATA {
        known_aliases.extend(known.aliases.iter().map(|a| a.to_string()));
        let matches: Vec<_> = installations
            .values()
            .filter(|app| {
                aliases(app)
                    .iter()
                    .any(|name| known.aliases.contains(&name.as_str()))
            })
            .collect();
        for &(environment, relative) in known.locations {
            let Some(path) = location(environment, relative) else {
                continue;
            };
            let (key, name) = if let [app] = matches.as_slice() {
                (installation_key(&app.install_location), app.name.clone())
            } else if matches.is_empty() {
                (
                    format!("known_data:{}", known.name.to_lowercase()),
                    known.name.into(),
                )
            } else {
                // An ambiguous installation is never joined to another copy by display name.
                (format!("application_data:{path}"), known.name.into())
            };
            insert_origin(
                origins,
                ApplicationOrigin {
                    path,
                    application_key: key,
                    name,
                    kind: OriginKind::ApplicationData,
                    confidence: "medium",
                    evidence: Evidence {
                        source: "已知应用数据位置".into(),
                        detail: format!(
                            "{} 的常见数据位置 %{}%\\{}；是路径关联，不能证明创建进程，也不代表全部是缓存",
                            known.name, environment, relative
                        ),
                    },
                },
            );
        }
    }

    for app in apps.iter().filter(|a| a.source.contains("MSIX")) {
        if app.id.is_empty()
            || app.id.len() > 255
            || !app
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            continue;
        }
        let Some(path) = location("LOCALAPPDATA", &format!("Packages\\{}", app.id)) else {
            continue;
        };
        let family_installations: BTreeSet<_> = apps
            .iter()
            .filter(|other| other.source.contains("MSIX") && other.id == app.id)
            .filter_map(|other| {
                installations
                    .get(&normalize(&other.install_location))
                    .map(|_| normalize(&other.install_location))
            })
            .collect();
        let key = if family_installations.len() == 1 {
            installation_key(family_installations.iter().next().unwrap())
        } else {
            format!("msix_data:{}", app.id)
        };
        insert_origin(
            origins,
            ApplicationOrigin {
                path,
                application_key: key,
                name: app.name.clone(),
                kind: OriginKind::ApplicationData,
                confidence: "high",
                evidence: Evidence {
                    source: "Windows 包家族标识".into(),
                    detail: format!(
                        "包家族 {} 与其独立用户数据目录精确对应；不据此判断具体文件的创建进程或删除安全性",
                        app.id
                    ),
                },
            },
        );
    }

    let mut by_alias: BTreeMap<String, BTreeMap<String, &InstalledApp>> = BTreeMap::new();
    for app in installations.values() {
        for alias in aliases(app) {
            if alias.len() < 4
                || alias.len() > 100
                || alias.contains(['\\', '/', ':'])
                || generic_container(&alias)
                || matches!(alias.as_str(), "app" | "application" | "bin" | "resources")
                || known_aliases.contains(&alias)
            {
                continue;
            }
            by_alias
                .entry(alias)
                .or_default()
                .insert(normalize(&app.install_location), app);
        }
    }
    for (alias, matches) in by_alias {
        if matches.len() != 1 {
            continue;
        }
        let app = matches.values().next().unwrap();
        for environment in ["APPDATA", "LOCALAPPDATA"] {
            let Some(path) = location(environment, &alias) else {
                continue;
            };
            insert_origin(
                origins,
                ApplicationOrigin {
                    path,
                    application_key: installation_key(&app.install_location),
                    name: app.name.clone(),
                    kind: OriginKind::ApplicationData,
                    confidence: "low",
                    evidence: Evidence {
                        source: "应用数据目录名称线索".into(),
                        detail: format!(
                            "%{environment}% 下独立目录 {alias} 与应用 {} 的名称或安装目录名唯一对应；仅推测关联，未确认创建者",
                            app.name
                        ),
                    },
                },
            );
        }
    }
}
