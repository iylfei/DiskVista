use crate::{application_index::ApplicationIndex, safety::SafetyPolicy, store::Store};
use anyhow::Result;
use cleaner_domain::{Evidence, InstalledApp};
use cleaner_platform::{normalize, within};
use std::collections::BTreeMap;

mod data_roots;
mod layout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginKind {
    Installation,
    ApplicationData,
    Portable,
}

#[derive(Clone, Debug)]
pub struct ApplicationOrigin {
    pub path: String,
    pub application_key: String,
    pub name: String,
    pub kind: OriginKind,
    pub confidence: &'static str,
    pub evidence: Evidence,
}

impl ApplicationOrigin {
    pub fn unit_kind(&self) -> &'static str {
        match self.kind {
            OriginKind::Installation => "application",
            OriginKind::ApplicationData => "application_data",
            OriginKind::Portable => "possible_application",
        }
    }

    pub fn role(&self) -> &'static str {
        match self.kind {
            OriginKind::Installation => "installation",
            OriginKind::ApplicationData => "application_data",
            OriginKind::Portable => "unconfirmed",
        }
    }
}

pub fn confidence_rank(confidence: &str) -> u8 {
    match confidence {
        "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}

pub fn weaker_confidence(left: &'static str, right: &'static str) -> &'static str {
    if confidence_rank(left) <= confidence_rank(right) {
        left
    } else {
        right
    }
}

pub fn installation_key(path: &str) -> String {
    format!("application:{}", normalize(path))
}

/// One ordering is used by file attribution and space accounting for duplicate records.
pub fn inventory_priority(app: &InstalledApp) -> (u8, String, String) {
    let priority = if app.source.contains("MSIX") {
        4
    } else if app.source.contains("Steam") || app.source.contains("Epic") {
        3
    } else if app.source.contains("快捷方式") {
        1
    } else {
        2
    };
    (priority, app.name.to_lowercase(), app.id.to_lowercase())
}

pub fn generic_container(path: &str) -> bool {
    let path = normalize(path);
    let name = path.rsplit('\\').next().unwrap_or("");
    matches!(
        name,
        "users"
            | "program files"
            | "program files (x86)"
            | "programdata"
            | "appdata"
            | "local"
            | "locallow"
            | "roaming"
            | "programs"
            | "games"
            | "apps"
            | "applications"
            | "downloads"
            | "documents"
            | "desktop"
            | "tools"
            | "common"
            | "common files"
            | "steamapps"
            | "packages"
            | "temp"
            | "tmp"
            | "microsoft"
            | "google"
            | "tencent"
            | "adobe"
            | "jetbrains"
            | "epic games"
    ) || path
        .rsplit_once('\\')
        .is_some_and(|(parent, _)| parent.ends_with("\\users"))
}

pub(super) fn from_inventory(
    apps: &[InstalledApp],
    policy: &SafetyPolicy,
) -> BTreeMap<String, ApplicationOrigin> {
    let mut selected = BTreeMap::<String, &InstalledApp>::new();
    for app in apps {
        if !policy.specific_install_root(&app.install_location)
            || generic_container(&app.install_location)
        {
            continue;
        }
        let key = normalize(&app.install_location);
        if selected
            .get(&key)
            .is_none_or(|old| inventory_priority(app) > inventory_priority(old))
        {
            selected.insert(key, app);
        }
    }
    let mut origins: BTreeMap<_, _> = selected
        .values()
        .map(|app| {
            let path = normalize(&app.install_location);
            (
                path.clone(),
                ApplicationOrigin {
                    application_key: installation_key(&path),
                    path,
                    name: app.name.clone(),
                    kind: OriginKind::Installation,
                    confidence: if app.source.contains("快捷方式") {
                        "medium"
                    } else {
                        "high"
                    },
                    evidence: Evidence {
                        source: app.source.clone(),
                        detail: format!(
                            "{} · {}；可确认安装位置关联，不能证明每个文件的创建进程",
                            app.name, app.install_location
                        ),
                    },
                },
            )
        })
        .collect();
    data_roots::add(apps, &selected, &mut origins);
    origins
}

pub(super) fn add_snapshot_layout(
    store: &Store,
    scan: &str,
    policy: &SafetyPolicy,
    index: &mut ApplicationIndex,
) -> Result<()> {
    layout::add(store, scan, policy, index)
}

pub(super) fn insert_origin(
    origins: &mut BTreeMap<String, ApplicationOrigin>,
    origin: ApplicationOrigin,
) {
    let key = normalize(&origin.path);
    if let Some(old) = origins.get(&key) {
        if old.kind == OriginKind::Installation
            || confidence_rank(old.confidence) >= confidence_rank(origin.confidence)
        {
            return;
        }
    }
    // A guessed data alias cannot displace an unrelated recorded installation.
    if origins.values().any(|old| {
        old.kind == OriginKind::Installation
            && within(&key, &old.path)
            && old.application_key != origin.application_key
    }) {
        return;
    }
    origins.insert(key, origin);
}
