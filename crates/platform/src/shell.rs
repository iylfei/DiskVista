use anyhow::{bail, Result};
use windows::{
    core::PCWSTR,
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
};
pub fn open_system(target: &str) -> Result<()> {
    let location = match target {
        "recycle" => "shell:RecycleBinFolder",
        "storage" => "ms-settings:storagesense",
        "apps" => "ms-settings:appsfeatures",
        "cloud" => "ms-settings:backup",
        _ => bail!("系统入口不在允许列表中"),
    };
    let w = crate::wide(location);
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::null(),
            PCWSTR(w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        bail!("Windows 无法打开此入口");
    }
    Ok(())
}
pub fn reveal(path: &str) -> Result<()> {
    crate::filesystem::validate_local_path(path)?;
    if path.contains('"') {
        bail!("路径包含非法引号");
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,\"{path}\""))
        .spawn()?;
    Ok(())
}
