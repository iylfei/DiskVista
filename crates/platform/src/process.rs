use anyhow::Result;
use std::os::windows::io::AsRawHandle;
use windows::Win32::{Foundation::*, System::JobObjects::*};

/// Prevent simultaneous desktop instances from recovering or mutating each other's index.
pub fn single_instance() -> Result<Option<crate::filesystem::FileHandle>> {
    use windows::{core::PCWSTR, Win32::System::Threading::CreateMutexW};
    let name = crate::wide("Local\\IntellDiskCleaner-v1");
    let handle =
        crate::filesystem::FileHandle(unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr()))? });
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(None);
    }
    Ok(Some(handle))
}

pub struct WorkerJob(HANDLE);
// Kernel job handles may be used and closed from any thread. Ownership is unique.
unsafe impl Send for WorkerJob {}
unsafe impl Sync for WorkerJob {}
impl WorkerJob {
    pub fn attach(child: &std::process::Child) -> Result<Self> {
        let handle = unsafe { CreateJobObjectW(None, None)? };
        let job = Self(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )?;
            AssignProcessToJobObject(handle, HANDLE(child.as_raw_handle()))?;
        }
        Ok(job)
    }
}
impl Drop for WorkerJob {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Restart Manager registers files, not directories. Use the verified descendant list.
pub fn locking_paths(paths: &[&str]) -> Result<Vec<String>> {
    locking_paths_checked(paths, |_| Ok(()))
}

/// Check cancellation between Restart Manager calls and release its session on error.
pub fn locking_paths_checked(
    paths: &[&str],
    mut check: impl FnMut(usize) -> Result<()>,
) -> Result<Vec<String>> {
    check(0)?;
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::System::RestartManager::*,
    };
    let mut handle = 0;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    let result = unsafe { RmStartSession(&mut handle, None, PWSTR(key.as_mut_ptr())) };
    if result != ERROR_SUCCESS {
        anyhow::bail!("无法启动占用检查：{}", result.0);
    }
    struct Guard(u32);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                let _ = RmEndSession(self.0);
            }
        }
    }
    let _guard = Guard(handle);
    for (index, chunk) in paths.chunks(256).enumerate() {
        check(index * 256)?;
        let wide: Vec<_> = chunk.iter().map(|p| crate::wide(p)).collect();
        let resources: Vec<_> = wide.iter().map(|p| PCWSTR(p.as_ptr())).collect();
        let result = unsafe { RmRegisterResources(handle, Some(&resources), None, None) };
        if result != ERROR_SUCCESS {
            anyhow::bail!("无法注册占用检查：{}", result.0);
        }
        check((index * 256 + chunk.len()).min(paths.len()))?;
    }
    let mut needed = 0;
    let mut count = 0;
    let mut reason = 0;
    let first = unsafe { RmGetList(handle, &mut needed, &mut count, None, &mut reason) };
    check(paths.len())?;
    if first != ERROR_MORE_DATA && first != ERROR_SUCCESS {
        anyhow::bail!("无法检查文件占用：{}", first.0);
    }
    if needed == 0 {
        return Ok(Vec::new());
    }
    let mut infos = vec![RM_PROCESS_INFO::default(); needed as usize];
    count = needed;
    let code = unsafe {
        RmGetList(
            handle,
            &mut needed,
            &mut count,
            Some(infos.as_mut_ptr()),
            &mut reason,
        )
    };
    check(paths.len())?;
    if code != ERROR_SUCCESS {
        anyhow::bail!("占用列表在读取时变化，请稍后重试");
    }
    Ok(infos[..count as usize]
        .iter()
        .map(|i| {
            let end = i
                .strAppName
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(i.strAppName.len());
            let name = String::from_utf16_lossy(&i.strAppName[..end]);
            if name.is_empty() {
                format!("PID {}", i.Process.dwProcessId)
            } else {
                name
            }
        })
        .collect())
}
