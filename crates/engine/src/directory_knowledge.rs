//! Known directory roles do not claim that one program created every descendant.
use cleaner_platform::normalize;
pub struct DirectoryInfo {
    pub category: &'static str,
    pub purpose: &'static str,
    pub consequence: &'static str,
    pub recovery: &'static str,
}
pub fn describe(path: &str) -> Option<DirectoryInfo> {
    let path = normalize(path);
    let is = |keys: &[&str]| {
        keys.iter()
            .filter_map(|k| std::env::var(k).ok())
            .any(|p| normalize(&p) == path)
    };
    if is(&["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]) {
        Some(DirectoryInfo {
            category: "application_container",
            purpose: "程序安装总目录，包含多个独立应用及共享组件",
            consequence:
                "整体删除会破坏多个应用；请进入应用空间查看各应用占用，并通过系统应用管理处理",
            recovery: "依赖各应用安装程序修复或重新安装，不保证能完整恢复",
        })
    } else if is(&["SystemRoot"]) {
        Some(DirectoryInfo {
            category: "system",
            purpose: "Windows 操作系统目录，包含系统文件、驱动及系统组件",
            consequence: "删除可能导致系统无法启动或功能损坏，只能通过系统入口管理",
            recovery: "可能需要系统修复或备份恢复",
        })
    } else if is(&["APPDATA", "LOCALAPPDATA", "ProgramData"]) {
        Some(DirectoryInfo {
            category: "application_container",
            purpose: "多应用数据总目录，包含配置、缓存及用户数据；应按应用和用途拆分查看",
            consequence: "不能将整个目录视为缓存；删除可能丢失配置和个人数据",
            recovery: "不同应用数据恢复方式不同，不能保证可重新生成",
        })
    } else {
        None
    }
}
