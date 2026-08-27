use crate::{filesystem, wide};
use anyhow::{anyhow, bail, Result};
use std::sync::{Arc, Mutex};
use windows::{
    core::{implement, Ref, HRESULT, PCWSTR},
    Win32::{
        Foundation::E_ABORT, Storage::FileSystem::GetVolumeNameForVolumeMountPointW,
        System::Com::*, UI::Shell::*,
    },
};
use winreg::{enums::*, RegKey};

#[derive(Default)]
struct Receipt {
    started: bool,
    recycled: bool,
    message: String,
}
#[implement(IFileOperationProgressSink)]
struct Sink {
    check: Box<dyn Fn() -> Result<()> + Send>,
    receipt: Arc<Mutex<Receipt>>,
}
#[allow(non_snake_case)]
impl IFileOperationProgressSink_Impl for Sink_Impl {
    fn StartOperations(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, _: HRESULT) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreDeleteItem(&self, flags: u32, _: Ref<'_, IShellItem>) -> windows::core::Result<()> {
        if flags & TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32 == 0 {
            self.receipt.lock().unwrap().message = "Shell 未承诺回收，已阻止操作".into();
            return Err(E_ABORT.into());
        }
        if let Err(e) = (self.check)() {
            self.receipt.lock().unwrap().message = format!("最终核验失败：{e:#}");
            return Err(E_ABORT.into());
        }
        self.receipt.lock().unwrap().started = true;
        Ok(())
    }
    fn PostDeleteItem(
        &self,
        flags: u32,
        _: Ref<'_, IShellItem>,
        result: HRESULT,
        created: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        let mut receipt = self.receipt.lock().unwrap();
        receipt.recycled = receipt.started
            && result.is_ok()
            && flags & TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32 != 0
            && created.is_some();
        if !receipt.recycled {
            receipt.message = format!(
                "Shell 未提供完整回收凭据（HRESULT 0x{:08x}）",
                result.0 as u32
            );
        }
        Ok(())
    }
    fn PreRenameItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Err(E_ABORT.into())
    }
    fn PostRenameItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Err(E_ABORT.into())
    }
    fn PostMoveItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        Err(E_ABORT.into())
    }
    fn PostCopyItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreNewItem(&self, _: u32, _: Ref<'_, IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Err(E_ABORT.into())
    }
    fn PostNewItem(
        &self,
        _: u32,
        _: Ref<'_, IShellItem>,
        _: &PCWSTR,
        _: &PCWSTR,
        _: u32,
        _: HRESULT,
        _: Ref<'_, IShellItem>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _: u32, _: u32) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
}
fn ensure_capacity(capacity_mib: Option<u32>, used: i64, required: u64) -> Result<()> {
    let capacity = u64::from(capacity_mib.ok_or_else(|| {
        anyhow!("无法读取此卷的回收站容量配置，请先在 Windows 回收站属性中确认容量")
    })?) * 1_048_576;
    if used < 0
        || capacity == 0
        || (used as u64)
            .saturating_add(required)
            .saturating_add(1_048_576)
            > capacity
    {
        bail!("回收站剩余容量不足或无法确认；为避免挤出旧项目，已停止回收");
    }
    Ok(())
}
fn availability(path: &str, required_bytes: u64) -> Result<()> {
    let w = wide(&path[..3]);
    let mut name = [0u16; 100];
    unsafe {
        GetVolumeNameForVolumeMountPointW(PCWSTR(w.as_ptr()), &mut name)?;
    }
    let volume =
        String::from_utf16_lossy(&name[..name.iter().position(|c| *c == 0).unwrap_or(name.len())]);
    let guid = volume
        .trim_start_matches("\\\\?\\Volume")
        .trim_end_matches('\\');
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let registry = RegKey::predef(hive);
        if registry
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer")
            .ok()
            .and_then(|k| k.get_value::<u32, _>("NoRecycleFiles").ok())
            == Some(1)
        {
            bail!("系统策略禁用了回收站");
        }
    }
    let mut capacity = None;
    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\BitBucket\\Volume\\{guid}"
    )) {
        if key.get_value::<u32, _>("NukeOnDelete").unwrap_or(0) != 0 {
            bail!("此卷配置为直接删除，已阻止操作");
        }
        capacity = key.get_value::<u32, _>("MaxCapacity").ok();
    }
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        SHQueryRecycleBinW(PCWSTR(w.as_ptr()), &mut info)?;
    }
    ensure_capacity(capacity, info.i64Size, required_bytes)?;
    Ok(())
}
/// Dedicated short-lived STA thread, with no DeleteFile/RemoveDirectory fallback.
pub fn one(
    path: &str,
    required_bytes: u64,
    check: impl Fn() -> Result<()> + Send + 'static,
) -> Result<()> {
    filesystem::validate_local_path(path)?;
    if path.len() <= 3 {
        bail!("不能回收卷根目录");
    }
    availability(path, required_bytes)?;
    let path = path.to_owned();
    std::thread::Builder::new()
        .name("recycle-sta".into())
        .spawn(move || -> Result<()> {
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            }
            struct Com;
            impl Drop for Com {
                fn drop(&mut self) {
                    unsafe {
                        CoUninitialize();
                    }
                }
            }
            let _com = Com;
            let receipt = Arc::new(Mutex::new(Receipt::default()));
            let check_path = path.clone();
            let sink: IFileOperationProgressSink = Sink {
                check: Box::new(move || {
                    availability(&check_path, required_bytes)?;
                    check()
                }),
                receipt: receipt.clone(),
            }
            .into();
            let w = wide(&path);
            unsafe {
                let operation: IFileOperation =
                    CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)?;
                operation.SetOperationFlags(
                    FOFX_RECYCLEONDELETE
                        | FOFX_ADDUNDORECORD
                        | FOFX_EARLYFAILURE
                        | FOF_NOERRORUI
                        | FOF_SILENT
                        | FOF_WANTNUKEWARNING
                        | FOF_NO_CONNECTED_ELEMENTS,
                )?;
                let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None)?;
                operation.DeleteItem(&item, &sink)?;
                let result = operation.PerformOperations();
                let aborted = operation.GetAnyOperationsAborted()?.as_bool();
                let receipt = receipt.lock().unwrap();
                if result.is_err() || aborted || !receipt.recycled {
                    bail!(
                        "{}",
                        if receipt.message.is_empty() {
                            "回收操作取消或未确认成功"
                        } else {
                            &receipt.message
                        }
                    );
                }
            }
            Ok(())
        })?
        .join()
        .map_err(|_| anyhow!("回收线程异常终止"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_full_disabled_or_overflowing_capacity_fails_closed() {
        assert!(ensure_capacity(None, 0, 1).is_err());
        assert!(ensure_capacity(Some(0), 0, 1).is_err());
        assert!(ensure_capacity(Some(10), 9 * 1_048_576, 1).is_err());
        assert!(ensure_capacity(Some(10), 0, u64::MAX).is_err());
        assert!(ensure_capacity(Some(10), -1, 0).is_err());
        assert!(ensure_capacity(Some(10), 1_048_576, 1_048_576).is_ok());
    }
}
