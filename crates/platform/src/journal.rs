use crate::{filesystem::FileHandle, wide};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use windows::{
    core::PCWSTR,
    Win32::{
        Storage::FileSystem::*,
        System::{Ioctl::*, IO::DeviceIoControl},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub journal_id: u64,
    pub first_usn: i64,
    pub next_usn: i64,
    pub volume: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    pub checkpoint: Checkpoint,
    pub changed_parents: Option<Vec<u128>>,
}
pub fn probe(path: &str, old: Option<&Checkpoint>) -> Result<Probe> {
    let current = checkpoint(path)?;
    let changed_parents = old.and_then(|old| changed_parents(path, old, &current).ok());
    Ok(Probe {
        checkpoint: current,
        changed_parents,
    })
}

fn open_volume(path: &str) -> Result<FileHandle> {
    let volume = wide(&format!("\\\\.\\{}:", &path[..1]));
    Ok(FileHandle(unsafe {
        CreateFileW(
            PCWSTR(volume.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )?
    }))
}

pub fn checkpoint(path: &str) -> Result<Checkpoint> {
    let handle = open_volume(path)?;
    let mut data = USN_JOURNAL_DATA_V0::default();
    let mut returned = 0;
    unsafe {
        DeviceIoControl(
            handle.0,
            FSCTL_QUERY_USN_JOURNAL,
            None,
            0,
            Some(&mut data as *mut _ as *mut _),
            std::mem::size_of_val(&data) as u32,
            Some(&mut returned),
            None,
        )?;
    }
    Ok(Checkpoint {
        journal_id: data.UsnJournalID,
        first_usn: data.FirstUsn,
        next_usn: data.NextUsn,
        volume: path[..2].to_uppercase(),
    })
}

/// Read-only journal catch-up; no journal creation or resizing is ever performed.
pub fn changed_parents(path: &str, old: &Checkpoint, current: &Checkpoint) -> Result<Vec<u128>> {
    if old.journal_id != current.journal_id
        || old.volume != current.volume
        || old.next_usn < current.first_usn
        || old.next_usn > current.next_usn
    {
        bail!("USN 日志缺口，必须完整扫描");
    }
    let handle = open_volume(path)?;
    let mut cursor = old.next_usn;
    let mut parents = std::collections::BTreeSet::new();
    let mut buffer = vec![0u64; 8192];
    while cursor < current.next_usn {
        let request = READ_USN_JOURNAL_DATA_V0 {
            StartUsn: cursor,
            ReasonMask: u32::MAX,
            ReturnOnlyOnClose: 0,
            Timeout: 0,
            BytesToWaitFor: 0,
            UsnJournalID: current.journal_id,
        };
        let mut returned = 0;
        unsafe {
            DeviceIoControl(
                handle.0,
                FSCTL_READ_USN_JOURNAL,
                Some(&request as *const _ as *const _),
                std::mem::size_of_val(&request) as u32,
                Some(buffer.as_mut_ptr().cast()),
                (buffer.len() * 8) as u32,
                Some(&mut returned),
                None,
            )?;
        }
        if returned < 8 {
            bail!("USN 返回记录不完整");
        }
        let next = buffer[0] as i64;
        if next <= cursor {
            bail!("USN 游标未推进");
        }
        let bytes =
            unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), returned as usize) };
        let mut offset = 8;
        while offset + 8 <= bytes.len() {
            let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into()?) as usize;
            let version = u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into()?);
            if len < 32 || offset + len > bytes.len() {
                bail!("USN 记录长度无效");
            }
            let (file, parent) = match version {
                2 if len >= 60 => (
                    u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into()?) as u128,
                    u64::from_le_bytes(bytes[offset + 16..offset + 24].try_into()?) as u128,
                ),
                3 if len >= 76 => (
                    u128::from_le_bytes(bytes[offset + 8..offset + 24].try_into()?),
                    u128::from_le_bytes(bytes[offset + 24..offset + 40].try_into()?),
                ),
                _ => bail!("不支持的 USN 版本，回退完整扫描"),
            };
            parents.insert(parent);
            parents.insert(file);
            if parents.len() > 100_000 {
                bail!("变化范围过大，回退完整扫描");
            }
            offset += len;
        }
        cursor = next;
    }
    Ok(parents.into_iter().collect())
}
