use cleaner_domain::{FileRecord, InstalledApp, Settings};
use cleaner_platform::{
    filesystem::{OFFLINE, RECALL, REPARSE},
    normalize, within,
};
use std::path::Path;

#[derive(Clone)]
pub struct SafetyPolicy {
    pub settings: Settings,
    pub system_roots: Vec<String>,
    pub cloud_roots: Vec<String>,
    pub container_roots: Vec<String>,
}

impl SafetyPolicy {
    pub fn new(settings: Settings) -> Self {
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let mut roots = vec![
            std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()),
            format!("{system_drive}\\Recovery"),
            format!("{system_drive}\\Boot"),
            format!("{system_drive}\\EFI"),
        ];
        for key in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Ok(p) = std::env::var(key) {
                for child in [
                    "Windows Defender",
                    "Windows Defender Advanced Threat Protection",
                    "Windows NT",
                    "WindowsPowerShell",
                ] {
                    roots.push(format!("{p}\\{child}"));
                }
            }
        }
        if let Ok(data) = std::env::var("ProgramData") {
            for child in [
                "Windows",
                "Windows Defender",
                "Crypto",
                "Protect",
                "Vault",
                "Network",
            ] {
                roots.push(format!("{data}\\Microsoft\\{child}"));
            }
        }
        let mut containers: Vec<String> = [
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "PUBLIC",
            "ProgramData",
        ]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .collect();
        if let Ok(user) = std::env::var("USERPROFILE") {
            for child in [
                "Desktop",
                "Documents",
                "Downloads",
                "Pictures",
                "Music",
                "Videos",
                "AppData",
            ] {
                containers.push(format!("{user}\\{child}"));
            }
        }
        Self {
            settings,
            system_roots: roots,
            cloud_roots: cleaner_platform::inventory::cloud_roots(),
            container_roots: containers.into_iter().map(|p| normalize(&p)).collect(),
        }
    }

    pub fn is_unprotected(&self, path: &str) -> bool {
        is_unprotected_by_paths(
            path,
            &self.settings.protected_paths,
            &self.settings.unprotected_paths,
        )
    }

    pub fn reason(&self, file: &FileRecord) -> Option<String> {
        let p = normalize(&file.path);
        if self.is_unprotected(&p) {
            return None;
        }
        if p.len() <= 3 || p.starts_with("\\\\") || file.path.contains('\0') {
            return Some("卷根目录或非本地普通路径，禁止回收".into());
        }
        if file.attributes & (REPARSE | OFFLINE | RECALL) != 0 {
            return Some("链接、云占位或离线文件，仅允许查看".into());
        }
        if file.attributes & 0x4 != 0 {
            return Some("Windows 系统属性文件，禁止通用清理".into());
        }
        let components: Vec<_> = p.split('\\').collect();
        if components.iter().any(|c| {
            matches!(
                *c,
                "$recycle.bin"
                    | "system volume information"
                    | "$extend"
                    | "$windows.~bt"
                    | "windows.old"
                    | "windowsapps"
                    | "wpsystem"
                    | "driverstore"
            )
        }) {
            return Some("Windows 管理区域，只能通过系统入口处理".into());
        }
        if self.system_roots.iter().any(|r| within(&p, r)) {
            return Some("系统关键内容，禁止直接回收".into());
        }
        if self.container_roots.contains(&p) {
            return Some("用户数据根目录，必须查看并选择具体子项".into());
        }
        if self.settings.protected_paths.iter().any(|r| within(&p, r)) {
            return Some("你已保护此路径".into());
        }
        if self.settings.ignored_paths.iter().any(|r| within(&p, r)) {
            return Some("你已忽略此路径".into());
        }
        let name = file.name.to_lowercase();
        if name.starts_with("ntuser.")
            || matches!(
                name.as_str(),
                "pagefile.sys" | "hiberfil.sys" | "swapfile.sys" | "bootmgr"
            )
        {
            return Some("系统状态文件，禁止回收".into());
        }
        None
    }

    pub fn installed_reason(&self, file: &FileRecord, apps: &[InstalledApp]) -> Option<String> {
        if self.is_unprotected(&file.path) {
            return None;
        }
        apps.iter()
            .find(|a| {
                self.specific_install_root(&a.install_location)
                    && within(&file.path, &a.install_location)
            })
            .map(|a| format!("属于已安装程序 {}，请从应用设置管理", a.name))
    }
    pub fn specific_install_root(&self, path: &str) -> bool {
        let path = normalize(path);
        path.len() > 3
            && !self.container_roots.contains(&path)
            && !self.system_roots.iter().any(|p| normalize(p) == path)
            && !["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
                .iter()
                .filter_map(|key| std::env::var(key).ok())
                .any(|root| normalize(&root) == path)
    }

    pub fn can_sample(&self, file: &FileRecord) -> bool {
        if file.is_dir || !self.can_send_metadata(file) {
            return false;
        }
        let p = normalize(&file.path);
        if sensitive_path(&p)
            || p.contains("\\user data\\")
            || p.contains("\\mozilla\\firefox\\profiles\\")
        {
            return false;
        }
        matches!(
            Path::new(&file.path)
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase)
                .as_deref(),
            Some("json" | "ini" | "toml" | "yaml" | "yml" | "txt" | "md" | "cfg")
        )
    }

    pub fn can_send_metadata(&self, file: &FileRecord) -> bool {
        self.reason(file).is_none()
            && !sensitive_path(&file.path)
            && !self.cloud_roots.iter().any(|r| within(&file.path, r))
            && !self
                .settings
                .excluded_llm_paths
                .iter()
                .any(|r| within(&file.path, r))
    }
}

pub(crate) fn is_unprotected_by_paths(
    path: &str,
    protected_paths: &[String],
    unprotected_paths: &[String],
) -> bool {
    let deepest = |paths: &[String]| {
        paths
            .iter()
            .filter(|root| within(path, root))
            .map(|root| normalize(root).len())
            .max()
    };
    let protected = deepest(protected_paths);
    deepest(unprotected_paths)
        .is_some_and(|unprotected| protected.is_none_or(|protected| unprotected >= protected))
}

pub fn sensitive_path(path: &str) -> bool {
    let p = normalize(path);
    p.split('\\').any(|c| {
        c == ".ssh"
            || c == ".aws"
            || c == ".gnupg"
            || c == ".env"
            || c.starts_with(".env.")
            || c.contains("wallet")
            || c.contains("password")
            || c.contains("credential")
            || c == "login data"
            || c == "cookies"
            || c == "key4.db"
            || c == "logins.json"
            || c == "history"
            || c == "wechat files"
            || c == "weixin"
            || c == "tencent files"
    }) || [
        ".pem",
        ".pfx",
        ".key",
        ".kdbx",
        ".sqlite",
        ".db",
        ".sqlite-wal",
        ".sqlite-shm",
        ".db-wal",
        ".db-shm",
    ]
    .iter()
    .any(|ext| p.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_content_remains_protected() {
        let policy = SafetyPolicy::new(Settings::default());
        for path in [
            "C:\\Windows\\System32\\drivers\\disk.sys",
            "D:\\$Recycle.Bin\\a",
        ] {
            let f = FileRecord {
                path: path.into(),
                ..Default::default()
            };
            assert!(policy.reason(&f).is_some(), "{path}");
        }
    }
    #[test]
    fn application_data_is_recyclable_but_private_data_is_not_sent_to_ai() {
        let policy = SafetyPolicy::new(Settings::default());
        for path in [
            "D:\\data\\.env",
            "D:\\data\\wallet.dat",
            "C:\\Users\\15830\\AppData\\Roaming\\Kiro\\User\\globalStorage\\kiro.kiroagent\\index\\docs.sqlite",
            "C:\\Users\\15830\\AppData\\Roaming\\Kiro\\User\\globalStorage\\kiro.kiroagent\\index\\index.sqlite-wal",
        ] {
            let file = FileRecord { path: path.into(), ..Default::default() };
            assert!(policy.reason(&file).is_none(), "{path}");
            assert!(!policy.can_send_metadata(&file), "{path}");
            assert!(!policy.can_sample(&file), "{path}");
        }
        for key in ["ProgramFiles", "ProgramData"] {
            let root = std::env::var(key).unwrap();
            assert!(policy
                .reason(&FileRecord {
                    path: format!("{root}\\ExampleApp\\data.db"),
                    ..Default::default()
                })
                .is_none());
        }
        let root = std::env::var("ProgramFiles").unwrap();
        assert!(policy
            .reason(&FileRecord {
                path: format!("{root}\\Windows Defender\\MsMpEng.exe"),
                ..Default::default()
            })
            .is_some());
    }
    #[test]
    fn regular_user_data_remains_reviewable() {
        let policy = SafetyPolicy::new(Settings::default());
        assert!(policy
            .reason(&FileRecord {
                path: "D:\\archive\\old-video.mp4".into(),
                ..Default::default()
            })
            .is_none());
    }
    #[test]
    fn link_and_cloud_flags_always_block() {
        let p = SafetyPolicy::new(Settings::default());
        for flag in [REPARSE, OFFLINE, RECALL] {
            assert!(p
                .reason(&FileRecord {
                    path: "D:\\cache\\x".into(),
                    attributes: flag,
                    ..Default::default()
                })
                .is_some());
        }
    }

    #[test]
    fn explicit_exception_overrides_system_and_user_protection_for_its_subtree() {
        let mut settings = Settings::default();
        settings.protected_paths.push("D:\\data".into());
        settings.unprotected_paths.push("D:\\data\\allowed".into());
        let policy = SafetyPolicy::new(settings);
        let allowed = FileRecord {
            path: "D:\\data\\allowed\\.env".into(),
            attributes: 0x4,
            ..Default::default()
        };
        assert!(policy.reason(&allowed).is_none());
        assert!(policy
            .reason(&FileRecord {
                path: "D:\\data\\blocked\\.env".into(),
                ..Default::default()
            })
            .is_some());
    }
}
