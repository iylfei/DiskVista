use anyhow::{anyhow, bail, Context, Result};
use windows::{
    core::PCWSTR,
    Win32::{
        System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED},
        UI::{
            Shell::{
                Common::ITEMIDLIST, SHOpenFolderAndSelectItems, SHParseDisplayName, ShellExecuteW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
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
    with_reveal_item(path, |item| {
        // With no child array, Shell opens the parent and selects this exact item.
        unsafe { SHOpenFolderAndSelectItems(item, None, 0) }
            .context("Windows 无法在资源管理器中定位此项目")
    })
}

fn with_reveal_item(
    path: &str,
    open: impl FnOnce(*const ITEMIDLIST) -> Result<()> + Send + 'static,
) -> Result<()> {
    let path = crate::filesystem::validate_local_path(path)
        .context("无法定位项目，请检查文件是否仍在原位置")?;
    std::thread::Builder::new()
        .name("reveal-sta".into())
        .spawn(move || -> Result<()> {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
            struct Com;
            impl Drop for Com {
                fn drop(&mut self) {
                    unsafe { CoUninitialize() };
                }
            }
            let _com = Com;
            let name = crate::wide(path.to_str().ok_or_else(|| anyhow!("路径编码无效"))?);
            let mut item = ItemIdList(std::ptr::null_mut());
            unsafe {
                SHParseDisplayName(PCWSTR(name.as_ptr()), None, &mut item.0, 0, None)
                    .context("Windows 无法解析此项目的路径")?;
            }
            anyhow::ensure!(!item.0.is_null(), "Windows 未返回有效的项目位置");
            open(item.0)
        })?
        .join()
        .map_err(|_| anyhow!("资源管理器定位线程异常退出"))?
}

struct ItemIdList(*mut ITEMIDLIST);
impl Drop for ItemIdList {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0.cast())) };
    }
}

#[cfg(test)]
mod tests;
