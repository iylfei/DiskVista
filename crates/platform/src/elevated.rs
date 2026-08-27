//! Narrow read-only USN helper protocol. No arbitrary paths, commands or database writes.
use crate::{
    filesystem::FileHandle,
    journal::{self, Checkpoint, Probe},
    wide,
};
use anyhow::{bail, Result};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Pipes::*, Threading::*},
        UI::{Shell::*, WindowsAndMessaging::SW_HIDE},
    },
};

pub fn query(worker: &Path, drive: char, nonce: &str, old: Option<&Checkpoint>) -> Result<Probe> {
    validate(drive, nonce)?;
    let pipe_name = wide(&format!("\\\\.\\pipe\\IntellDiskCleaner-{nonce}"));
    let pipe = FileHandle(unsafe {
        CreateNamedPipeW(
            PCWSTR(pipe_name.as_ptr()),
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            65536,
            65536,
            0,
            None,
        )
    });
    if pipe.0 == INVALID_HANDLE_VALUE {
        bail!("无法创建只读 Helper 通道");
    }
    let exe = wide(
        worker
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Helper 路径编码无效"))?,
    );
    let verb = wide("runas");
    let params = wide(&format!(
        "--journal {drive} {nonce} {} {} {}",
        old.map(|c| c.journal_id).unwrap_or(0),
        old.map(|c| c.first_usn).unwrap_or(0),
        old.map(|c| c.next_usn).unwrap_or(0)
    ));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(exe.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info)?;
    }
    let process = FileHandle(info.hProcess);
    let expected_pid = unsafe { GetProcessId(process.0) };
    if expected_pid == 0 {
        bail!("无法验证只读 Helper 身份，回退完整扫描");
    }
    let start = Instant::now();
    let mut output = Vec::new();
    let mut buffer = [0u8; 65536];
    loop {
        let _ = unsafe { ConnectNamedPipe(pipe.0, None) };
        let mut client_pid = 0;
        let authenticated = unsafe { GetNamedPipeClientProcessId(pipe.0, &mut client_pid) }.is_ok();
        if authenticated && client_pid != expected_pid {
            bail!("只读 Helper 通道进程身份不匹配，回退完整扫描");
        }
        let mut count = 0;
        let result = unsafe { ReadFile(pipe.0, Some(&mut buffer), Some(&mut count), None) };
        if count > 0 {
            if !authenticated {
                bail!("只读 Helper 通道身份未经验证，回退完整扫描");
            }
            output.extend_from_slice(&buffer[..count as usize]);
            if output.len() > 4_194_304 {
                bail!("Helper 响应超过限制");
            }
            if output.ends_with(b"\n") {
                break;
            }
        }
        if result.is_err() {
            let mut code = 0;
            unsafe {
                GetExitCodeProcess(process.0, &mut code)?;
            }
            if code != 259 {
                bail!("只读 Helper 未返回完整数据，回退完整扫描");
            }
        }
        if start.elapsed() > Duration::from_secs(90) {
            bail!("Helper 超时，回退完整扫描");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(serde_json::from_slice(&output)?)
}
fn validate(drive: char, nonce: &str) -> Result<()> {
    if !drive.is_ascii_alphabetic()
        || nonce.len() != 36
        || !nonce.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        bail!("Helper 只接受盘符和一次性通道 ID");
    }
    Ok(())
}
pub fn helper(args: &[String]) -> Result<()> {
    if args.len() != 6 {
        bail!("Helper 参数错误");
    }
    let drive = args[1]
        .chars()
        .next()
        .ok_or_else(|| anyhow::anyhow!("盘符缺失"))?;
    if args[1].len() != 1 {
        bail!("非法盘符");
    }
    validate(drive, &args[2])?;
    let id: u64 = args[3].parse()?;
    let root = format!("{drive}:\\");
    let old = if id == 0 {
        None
    } else {
        Some(Checkpoint {
            journal_id: id,
            first_usn: args[4].parse()?,
            next_usn: args[5].parse()?,
            volume: format!("{}:", drive.to_ascii_uppercase()),
        })
    };
    let result = journal::probe(&root, old.as_ref())?;
    let mut body = serde_json::to_vec(&result)?;
    body.push(b'\n');
    let pipe = wide(&format!("\\\\.\\pipe\\IntellDiskCleaner-{}", args[2]));
    let handle = FileHandle(unsafe {
        CreateFileW(
            PCWSTR(pipe.as_ptr()),
            FILE_GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )?
    });
    let mut written = 0;
    unsafe {
        WriteFile(handle.0, Some(&body), Some(&mut written), None)?;
    }
    if written as usize != body.len() {
        bail!("Helper 响应未完整写入");
    }
    Ok(())
}
