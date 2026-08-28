use crate::normalize;
use cleaner_domain::InstalledApp;
use std::{collections::BTreeMap, path::Path};
use winreg::{enums::*, RegKey};

pub fn installed_apps() -> Vec<InstalledApp> {
    let mut apps = BTreeMap::new();
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let root = RegKey::predef(hive);
            let Ok(key) = root.open_subkey_with_flags(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
                KEY_READ | view,
            ) else {
                continue;
            };
            for id in key.enum_keys().flatten() {
                let Ok(item) = key.open_subkey(&id) else {
                    continue;
                };
                let Ok(name) = item.get_value::<String, _>("DisplayName") else {
                    continue;
                };
                if name.trim().is_empty() {
                    continue;
                }
                let location = install_location(
                    &item
                        .get_value::<String, _>("InstallLocation")
                        .unwrap_or_default(),
                    &item
                        .get_value::<String, _>("DisplayIcon")
                        .unwrap_or_default(),
                );
                apps.entry(format!("{}|{}", name.to_lowercase(), normalize(&location)))
                    .or_insert(InstalledApp {
                        id,
                        name,
                        publisher: item.get_value("Publisher").unwrap_or_default(),
                        install_location: location,
                        source: "Windows 卸载清单".into(),
                        last_used: None,
                    });
            }
        }
    }
    add_epic(&mut apps);
    add_steam(&mut apps);
    add_msix(&mut apps);
    add_shortcuts(&mut apps);
    apps.into_values().collect()
}

fn add_msix(apps: &mut BTreeMap<String, InstalledApp>) {
    use windows::{core::HSTRING, Management::Deployment::PackageManager};
    let Ok(manager) = PackageManager::new() else {
        return;
    };
    let Ok(packages) = manager.FindPackagesByUserSecurityId(&HSTRING::new()) else {
        return;
    };
    for package in packages {
        let Ok(id) = package.Id() else { continue };
        let name = package
            .DisplayName()
            .map(|s| s.to_string())
            .unwrap_or_else(|_| id.Name().map(|s| s.to_string()).unwrap_or_default());
        let location = package
            .InstalledLocation()
            .and_then(|l| l.Path())
            .map(|s| s.to_string())
            .unwrap_or_default();
        let family = id.FamilyName().map(|s| s.to_string()).unwrap_or_default();
        apps.insert(
            format!("msix:{family}|{}", normalize(&location)),
            InstalledApp {
                id: family,
                name,
                publisher: id.Publisher().map(|s| s.to_string()).unwrap_or_default(),
                install_location: location,
                source: "Windows MSIX 包清单".into(),
                last_used: None,
            },
        );
    }
}
fn add_shortcuts(apps: &mut BTreeMap<String, InstalledApp>) {
    use windows::{
        core::{Interface, PCWSTR},
        Win32::{System::Com::*, UI::Shell::*},
    };
    // Dedicated COM apartment. Resolve is never called, so shortcuts cannot launch or update targets.
    let found = std::thread::spawn(|| -> Vec<InstalledApp> {
        if unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.is_err() {
            return vec![];
        }
        struct Com;
        impl Drop for Com {
            fn drop(&mut self) {
                unsafe { CoUninitialize() }
            }
        }
        let _com = Com;
        let mut dirs: Vec<_> = ["APPDATA", "ProgramData"]
            .iter()
            .filter_map(|key| std::env::var(key).ok())
            .map(|p| {
                (
                    std::path::PathBuf::from(p).join("Microsoft/Windows/Start Menu/Programs"),
                    0,
                )
            })
            .collect();
        let mut found = Vec::new();
        while let Some((dir, depth)) = dirs.pop() {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(meta) = crate::filesystem::inspect(&path) else {
                    continue;
                };
                if meta.attributes
                    & (crate::filesystem::REPARSE
                        | crate::filesystem::OFFLINE
                        | crate::filesystem::RECALL)
                    != 0
                {
                    continue;
                }
                if meta.is_dir && depth < 4 {
                    dirs.push((path, depth + 1));
                    continue;
                }
                if path
                    .extension()
                    .is_none_or(|e| !e.eq_ignore_ascii_case("lnk"))
                {
                    continue;
                }
                let result = (|| -> windows::core::Result<InstalledApp> {
                    unsafe {
                        let link: IShellLinkW =
                            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
                        let persist: IPersistFile = link.cast()?;
                        let w = crate::wide(&meta.path);
                        persist.Load(PCWSTR(w.as_ptr()), STGM_READ)?;
                        let mut target = vec![0u16; 32768];
                        link.GetPath(&mut target, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)?;
                        let target = String::from_utf16_lossy(
                            &target[..target.iter().position(|c| *c == 0).unwrap_or(target.len())],
                        );
                        let location = Path::new(&target)
                            .parent()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        Ok(InstalledApp {
                            id: format!("lnk:{}", meta.path),
                            name: path
                                .file_stem()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            publisher: String::new(),
                            install_location: if target.to_lowercase().ends_with(".exe")
                                && !target.starts_with("\\\\")
                            {
                                if normalize(&location).len() <= 3 {
                                    target
                                } else {
                                    location
                                }
                            } else {
                                String::new()
                            },
                            source: "开始菜单快捷方式（并非安装/创建者证明）".into(),
                            last_used: None,
                        })
                    }
                })();
                if let Ok(app) = result {
                    if !app.install_location.is_empty() {
                        found.push(app)
                    }
                }
            }
        }
        found
    })
    .join()
    .unwrap_or_default();
    for app in found {
        if !apps
            .values()
            .any(|a| normalize(&a.install_location) == normalize(&app.install_location))
        {
            apps.insert(app.id.clone(), app);
        }
    }
}

fn add_epic(apps: &mut BTreeMap<String, InstalledApp>) {
    let root = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into());
    let dir = Path::new(&root).join("Epic/EpicGamesLauncher/Data/Manifests");
    let Ok(files) = std::fs::read_dir(dir) else {
        return;
    };
    for f in files.flatten() {
        if f.path().extension().and_then(|s| s.to_str()) != Some("item") {
            continue;
        }
        if f.metadata().map(|m| m.len() > 2_000_000).unwrap_or(true) {
            continue;
        }
        let Some(value) = std::fs::read_to_string(f.path())
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        else {
            continue;
        };
        let name = value["DisplayName"].as_str().unwrap_or("");
        let location = value["InstallLocation"].as_str().unwrap_or("");
        if name.is_empty() || location.is_empty() {
            continue;
        }
        apps.insert(
            format!(
                "epic:{}|{}",
                value["AppName"].as_str().unwrap_or(name),
                normalize(location)
            ),
            InstalledApp {
                id: value["AppName"].as_str().unwrap_or(name).into(),
                name: name.into(),
                publisher: String::new(),
                install_location: location.into(),
                source: "Epic 应用清单".into(),
                last_used: None,
            },
        );
    }
}

fn quoted_values(text: &str, key: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let tokens: Vec<_> = line.split('"').collect();
            if tokens.len() >= 4 && tokens[1].eq_ignore_ascii_case(key) {
                Some(tokens[3].replace("\\\\", "\\"))
            } else {
                None
            }
        })
        .collect()
}

fn registry_path(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let mut result = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        result.push_str(&rest[..start]);
        let remaining = &rest[start + 1..];
        let end = remaining.find('%')?;
        result.push_str(&std::env::var(&remaining[..end]).ok()?);
        rest = &remaining[end + 1..];
    }
    result.push_str(rest);
    (Path::new(&result).is_absolute() && !result.starts_with("\\\\") && !result.contains('\0'))
        .then_some(result)
}

fn icon_executable(value: &str) -> Option<String> {
    let value = value.trim();
    let path = if let Some(quoted) = value.strip_prefix('"') {
        let end = quoted.find('"')?;
        let suffix = quoted[end + 1..].trim();
        if !suffix.is_empty()
            && !suffix
                .strip_prefix(',')
                .is_some_and(|index| index.trim().parse::<i32>().is_ok())
        {
            return None;
        }
        &quoted[..end]
    } else if let Some((path, index)) = value.rsplit_once(',') {
        if index.trim().parse::<i32>().is_ok() {
            path
        } else {
            value
        }
    } else {
        value
    };
    let path = registry_path(path)?;
    path.to_lowercase().ends_with(".exe").then_some(path)
}

fn install_location(location: &str, icon: &str) -> String {
    let location = registry_path(location);
    if let Some(location) = location.as_ref().filter(|path| normalize(path).len() > 3) {
        return location.clone();
    }
    let Some(executable) = icon_executable(icon) else {
        return String::new();
    };
    let parent = Path::new(&executable)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    if location.is_some() || normalize(&parent).len() <= 3 {
        executable
    } else {
        parent
    }
}

fn add_steam(apps: &mut BTreeMap<String, InstalledApp>) {
    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") else {
        return;
    };
    let Ok(root) = key.get_value::<String, _>("SteamPath") else {
        return;
    };
    let mut roots = vec![root.clone()];
    if let Ok(text) = std::fs::read_to_string(Path::new(&root).join("steamapps/libraryfolders.vdf"))
    {
        roots.extend(quoted_values(&text, "path"));
    }
    roots.sort();
    roots.dedup();
    for root in roots {
        let Ok(files) = std::fs::read_dir(Path::new(&root).join("steamapps")) else {
            continue;
        };
        for f in files.flatten() {
            if f.path().extension().and_then(|s| s.to_str()) != Some("acf") {
                continue;
            }
            if f.metadata().map(|m| m.len() > 2_000_000).unwrap_or(true) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(f.path()) else {
                continue;
            };
            let Some(name) = quoted_values(&text, "name").first().cloned() else {
                continue;
            };
            let Some(dir) = quoted_values(&text, "installdir").first().cloned() else {
                continue;
            };
            if dir.contains("..") || dir.contains(':') || dir.starts_with(['/', '\\']) {
                continue;
            }
            let location = Path::new(&root)
                .join("steamapps/common")
                .join(dir)
                .to_string_lossy()
                .into_owned();
            apps.insert(
                format!("steam:{name}|{}", normalize(&location)),
                InstalledApp {
                    id: quoted_values(&text, "appid")
                        .first()
                        .cloned()
                        .unwrap_or_else(|| name.clone()),
                    name,
                    publisher: String::new(),
                    install_location: location,
                    source: "Steam 应用清单".into(),
                    last_used: None,
                },
            );
        }
    }
}

pub fn cloud_roots() -> Vec<String> {
    let mut roots: Vec<_> = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .collect();
    if let Ok(accounts) =
        RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Microsoft\\OneDrive\\Accounts")
    {
        for name in accounts.enum_keys().flatten() {
            if let Some(folder) = accounts
                .open_subkey(name)
                .ok()
                .and_then(|k| k.get_value::<String, _>("UserFolder").ok())
            {
                roots.push(folder);
            }
        }
    }
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let registry = RegKey::predef(hive);
        if let Ok(providers) = registry
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\SyncRootManager")
        {
            for provider in providers.enum_keys().flatten() {
                if let Ok(users) = providers.open_subkey(format!("{provider}\\UserSyncRoots")) {
                    for (key, _) in users.enum_values().flatten() {
                        if let Ok(path) = users.get_value::<String, _>(&key) {
                            roots.push(path);
                        }
                    }
                }
            }
        }
        if let Ok(providers) = registry.open_subkey("Software\\SyncEngines\\Providers") {
            for provider in providers.enum_keys().flatten() {
                if let Ok(key) = providers.open_subkey(provider) {
                    if let Ok(path) = key.get_value::<String, _>("MountPoint") {
                        roots.push(path);
                    }
                }
            }
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        for name in ["Dropbox", "Google Drive", "iCloudDrive", "iCloud Drive"] {
            let path = Path::new(&profile).join(name);
            if path.exists() {
                roots.push(path.to_string_lossy().into_owned());
            }
        }
    }
    roots.retain(|p| normalize(p).len() > 3);
    roots.sort();
    roots.dedup();
    roots
}

pub fn last_access_policy() -> String {
    let value = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SYSTEM\\CurrentControlSet\\Control\\FileSystem")
        .ok()
        .and_then(|k| k.get_value::<u32, _>("NtfsDisableLastAccessUpdate").ok());
    match value {
        Some(v) if v & 1 != 0 => "NTFS 访问时间更新已禁用或由系统禁用；访问时间仅作弱参考".into(),
        Some(_) => "NTFS 访问时间更新开启，但可能延迟，且不等于用户实际使用".into(),
        None => "无法确定访问时间策略；仅作弱参考".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_icon_parsing_preserves_quoted_commas_and_expands_environment_paths() {
        assert_eq!(
            icon_executable(r#""D:\Apps, data\Editor\editor.exe",-12"#).as_deref(),
            Some(r"D:\Apps, data\Editor\editor.exe")
        );
        assert_eq!(
            icon_executable(r"D:\Apps, data\Editor\editor.exe,0").as_deref(),
            Some(r"D:\Apps, data\Editor\editor.exe")
        );
        assert_eq!(
            install_location("", r#""D:\Apps, data\Editor\editor.exe",0"#),
            r"D:\Apps, data\Editor"
        );
        let programs = std::env::var("ProgramFiles").unwrap();
        assert_eq!(
            install_location(r"%ProgramFiles%\Fixture App", ""),
            format!("{programs}\\Fixture App")
        );
    }

    #[test]
    fn a_drive_root_icon_identifies_only_the_executable_and_commands_are_not_paths() {
        assert_eq!(
            install_location("D:\\", r#""D:\node.exe",0"#),
            r"D:\node.exe"
        );
        assert_eq!(install_location("", r"D:\node.exe,0"), r"D:\node.exe");
        assert!(icon_executable(r#""D:\app.exe" --repair"#).is_none());
        assert!(icon_executable(r"D:\icons\app.ico,0").is_none());
        assert!(icon_executable(r"relative\app.exe,0").is_none());
    }
}
