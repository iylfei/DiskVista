use crate::{filesystem, wide};
use anyhow::{anyhow, bail, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
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
    failed: bool,
    skipped: bool,
    message: String,
}
#[implement(IFileOperationProgressSink)]
struct Sink {
    path: String,
    check: Box<dyn Fn() -> Result<u64> + Send>,
    cancel: Arc<AtomicBool>,
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
            let mut receipt = self.receipt.lock().unwrap();
            receipt.failed = true;
            receipt.message = "Shell 未承诺回收，已阻止操作".into();
            return Err(E_ABORT.into());
        }
        if self.cancel.load(Ordering::Relaxed) {
            let mut receipt = self.receipt.lock().unwrap();
            receipt.skipped = true;
            receipt.message = "用户取消，未处理剩余项目".into();
            return Err(E_ABORT.into());
        }
        let required_bytes = match (self.check)() {
            Ok(bytes) => bytes,
            Err(e) => {
                let mut receipt = self.receipt.lock().unwrap();
                receipt.skipped = true;
                receipt.message = format!("执行前复核未通过：{e:#}");
                return Err(E_ABORT.into());
            }
        };
        if let Err(e) = availability(&self.path, required_bytes) {
            let mut receipt = self.receipt.lock().unwrap();
            receipt.failed = true;
            receipt.message = format!("回收站最终核验失败：{e:#}");
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
            receipt.failed = true;
            receipt.message = format!(
                "Shell 未提供完整回收凭据（HRESULT 0x{:08x}）",
                result.0 as u32
            );
            return Err(E_ABORT.into());
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

pub struct Request {
    path: String,
    check: Box<dyn Fn() -> Result<u64> + Send>,
}

impl Request {
    pub fn new(path: impl Into<String>, check: impl Fn() -> Result<u64> + Send + 'static) -> Self {
        Self {
            path: path.into(),
            check: Box::new(check),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Recycled,
    Failed(String),
    Skipped(String),
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

fn validate_requests(requests: &[Request], cancel: &AtomicBool) -> Result<bool> {
    for request in requests {
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        filesystem::validate_local_path(&request.path)?;
        if request.path.len() <= 3 {
            bail!("不能回收卷根目录");
        }
    }
    Ok(true)
}

fn classify_receipts(
    receipts: &[Arc<Mutex<Receipt>>],
    operation_error: Option<&str>,
    aborted: bool,
    cancelled: bool,
) -> Vec<Outcome> {
    let mut stopped = false;
    receipts
        .iter()
        .map(|receipt| {
            let receipt = receipt.lock().unwrap();
            if receipt.recycled {
                return Outcome::Recycled;
            }
            if receipt.failed {
                stopped = true;
                return Outcome::Failed(receipt.message.clone());
            }
            if receipt.skipped {
                stopped = true;
                return Outcome::Skipped(receipt.message.clone());
            }
            if cancelled {
                stopped = true;
                return Outcome::Skipped("用户取消，未处理剩余项目".into());
            }
            if stopped {
                return Outcome::Skipped("前一项无法确认安全回收，已停止批次，未处理此项".into());
            }
            stopped = true;
            let message = operation_error
                .filter(|message| !message.is_empty())
                .map(|message| format!("Shell 回收操作失败：{message}"))
                .unwrap_or_else(|| {
                    if aborted {
                        "回收操作取消或未确认成功".into()
                    } else if receipt.started {
                        "Shell 未返回最终回收凭据".into()
                    } else {
                        "Shell 未开始回收操作".into()
                    }
                });
            Outcome::Failed(message)
        })
        .collect()
}

/// Queue a whole cleanup batch on one short-lived STA thread. There is no
/// DeleteFile/RemoveDirectory fallback, and each target keeps its own final check.
pub fn batch(requests: Vec<Request>, cancel: Arc<AtomicBool>) -> Result<Vec<Outcome>> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok(requests
            .iter()
            .map(|_| Outcome::Skipped("用户取消，未处理剩余项目".into()))
            .collect());
    }
    if !validate_requests(&requests, &cancel)? {
        return Ok(requests
            .iter()
            .map(|_| Outcome::Skipped("用户取消，未处理剩余项目".into()))
            .collect());
    }
    std::thread::Builder::new()
        .name("recycle-sta".into())
        .spawn(move || -> Result<Vec<Outcome>> {
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
                let mut receipts = Vec::with_capacity(requests.len());
                let mut sinks = Vec::with_capacity(requests.len());
                for request in requests {
                    let receipt = Arc::new(Mutex::new(Receipt::default()));
                    let sink: IFileOperationProgressSink = Sink {
                        path: request.path.clone(),
                        check: request.check,
                        cancel: cancel.clone(),
                        receipt: receipt.clone(),
                    }
                    .into();
                    let w = wide(&request.path);
                    let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(w.as_ptr()), None)?;
                    operation.DeleteItem(&item, &sink)?;
                    receipts.push(receipt);
                    sinks.push(sink);
                }
                let result = operation.PerformOperations();
                let aborted = operation.GetAnyOperationsAborted()?.as_bool();
                let operation_error = result.as_ref().err().map(ToString::to_string);
                let outcomes = classify_receipts(
                    &receipts,
                    operation_error.as_deref(),
                    aborted,
                    cancel.load(Ordering::Relaxed),
                );
                drop(operation);
                drop(sinks);
                Ok(outcomes)
            }
        })?
        .join()
        .map_err(|_| anyhow!("回收线程异常终止"))?
}

pub fn one(
    path: &str,
    required_bytes: u64,
    check: impl Fn() -> Result<()> + Send + 'static,
) -> Result<()> {
    let outcomes = batch(
        vec![Request::new(path, move || {
            check()?;
            Ok(required_bytes)
        })],
        Arc::new(AtomicBool::new(false)),
    )?;
    match outcomes.into_iter().next() {
        Some(Outcome::Recycled) => Ok(()),
        Some(Outcome::Failed(message)) | Some(Outcome::Skipped(message)) => bail!("{message}"),
        None => bail!("回收操作没有返回结果"),
    }
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

    #[test]
    fn batch_receipts_keep_success_failure_and_unstarted_items_distinct() {
        let recycled = Arc::new(Mutex::new(Receipt {
            started: true,
            recycled: true,
            ..Default::default()
        }));
        let failed = Arc::new(Mutex::new(Receipt {
            failed: true,
            message: "fixture failure".into(),
            ..Default::default()
        }));
        let pending = Arc::new(Mutex::new(Receipt::default()));
        assert_eq!(
            classify_receipts(&[recycled, failed, pending], Some("aborted"), true, false),
            vec![
                Outcome::Recycled,
                Outcome::Failed("fixture failure".into()),
                Outcome::Skipped("前一项无法确认安全回收，已停止批次，未处理此项".into()),
            ]
        );
    }

    #[test]
    fn cancelled_unstarted_batch_is_all_skipped() {
        let receipts = vec![
            Arc::new(Mutex::new(Receipt::default())),
            Arc::new(Mutex::new(Receipt::default())),
        ];
        assert!(classify_receipts(&receipts, None, true, true)
            .iter()
            .all(|outcome| matches!(outcome, Outcome::Skipped(_))));
    }
}
