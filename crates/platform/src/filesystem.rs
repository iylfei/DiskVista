use crate::wide;
use anyhow::{anyhow, bail, Context, Result};
use cleaner_domain::{FileRecord, Volume};
use std::{
    ffi::c_void,
    mem::{offset_of, size_of},
    path::Path,
};
use windows::{
    core::PCWSTR,
    Win32::{Foundation::*, Storage::FileSystem::*},
};

pub const REPARSE: u32 = 0x400;
pub const OFFLINE: u32 = 0x1000;
pub const RECALL: u32 = 0x40000 | 0x400000;
pub const DIRECTORY: u32 = 0x10;
fn api_path(path: &str) -> Vec<u16> {
    if path.starts_with("\\\\?\\") {
        wide(path)
    } else {
        wide(&format!("\\\\?\\{}", path.replace('/', "\\")))
    }
}

pub struct FileHandle(pub HANDLE);
impl Drop for FileHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn open_metadata(path: &Path, directory_listing: bool) -> Result<FileHandle> {
    let s = path
        .to_str()
        .ok_or_else(|| anyhow!("路径不是有效 Unicode，已跳过以避免错误操作"))?;
    let w = api_path(s);
    let access = if directory_listing {
        FILE_LIST_DIRECTORY.0 | FILE_READ_ATTRIBUTES.0
    } else {
        FILE_READ_ATTRIBUTES.0
    };
    // OPEN_REPARSE_POINT ensures inspecting a link never follows its target.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )?
    };
    Ok(FileHandle(handle))
}

pub fn file_time(ticks: i64) -> i64 {
    ticks.saturating_sub(116_444_736_000_000_000) / 10_000_000
}
fn ft(t: FILETIME) -> i64 {
    file_time(((t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64) as i64)
}

pub fn inspect(path: &Path) -> Result<FileRecord> {
    let handle =
        open_metadata(path, false).with_context(|| format!("无法读取属性：{}", path.display()))?;
    describe(path, &handle)
}

fn describe(path: &Path, handle: &FileHandle) -> Result<FileRecord> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(handle.0, &mut info)?;
    }
    let mut id = FILE_ID_INFO::default();
    let identity = if unsafe {
        GetFileInformationByHandleEx(
            handle.0,
            FileIdInfo,
            &mut id as *mut _ as *mut c_void,
            size_of::<FILE_ID_INFO>() as u32,
        )
    }
    .is_ok()
    {
        (u128::from_le_bytes(id.FileId.Identifier) != 0).then(|| {
            format!(
                "{:x}:{:032x}",
                id.VolumeSerialNumber,
                u128::from_le_bytes(id.FileId.Identifier)
            )
        })
    } else {
        (info.nFileIndexHigh != 0 || info.nFileIndexLow != 0).then(|| {
            format!(
                "{:x}:{:032x}",
                info.dwVolumeSerialNumber,
                ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64
            )
        })
    };
    let mut standard = FILE_STANDARD_INFO::default();
    let allocated = unsafe {
        GetFileInformationByHandleEx(
            handle.0,
            FileStandardInfo,
            &mut standard as *mut _ as *mut c_void,
            size_of::<FILE_STANDARD_INFO>() as u32,
        )
    }
    .ok()
    .map(|_| standard.AllocationSize.max(0) as u64);
    let is_dir = info.dwFileAttributes & DIRECTORY != 0;
    let logical = if is_dir {
        0
    } else {
        ((info.nFileSizeHigh as u64) << 32) | info.nFileSizeLow as u64
    };
    let path_s = path
        .to_str()
        .ok_or_else(|| anyhow!("路径编码无法安全转换"))?
        .to_owned();
    Ok(FileRecord {
        path: path_s,
        parent: path.parent().and_then(Path::to_str).unwrap_or("").into(),
        name: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path.display().to_string())
            .into(),
        is_dir,
        logical_bytes: logical,
        allocated_bytes: if is_dir { Some(0) } else { allocated },
        modified: ft(info.ftLastWriteTime),
        modified_ticks: ((info.ftLastWriteTime.dwHighDateTime as u64) << 32
            | info.ftLastWriteTime.dwLowDateTime as u64) as i64,
        accessed: ft(info.ftLastAccessTime),
        created: ft(info.ftCreationTime),
        identity,
        attributes: info.dwFileAttributes,
        links: info.nNumberOfLinks,
        file_count: u64::from(!is_dir),
        complete: true,
        enumerated: !is_dir,
        ..Default::default()
    })
}

pub fn read_text_prefix(
    path: &Path,
    expected_identity: &Option<String>,
    maximum: u32,
) -> Result<Vec<u8>> {
    validate_local_path(path.to_str().ok_or_else(|| anyhow!("路径编码无效"))?)?;
    let w = api_path(path.to_str().unwrap());
    // Deny concurrent writes/renames, and never follow a replaced reparse point.
    let handle = FileHandle(unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            FILE_READ_DATA.0 | FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )?
    });
    let info = describe(path, &handle)?;
    if info.is_dir
        || info.attributes & (REPARSE | OFFLINE | RECALL) != 0
        || expected_identity.is_none()
        || &info.identity != expected_identity
    {
        bail!("文件身份或特殊状态变化，拒绝读取");
    }
    let mut bytes = vec![0; maximum.min(4096) as usize];
    let mut read = 0;
    unsafe {
        ReadFile(handle.0, Some(&mut bytes), Some(&mut read), None)?;
    }
    bytes.truncate(read as usize);
    Ok(bytes)
}

/// Bounded 64-KiB native enumeration. The callback can stop a cancelled scan.
pub fn enumerate(path: &Path, mut emit: impl FnMut(FileRecord) -> Result<bool>) -> Result<()> {
    let handle = open_metadata(path, true)?;
    let root = describe(path, &handle)?;
    if root.attributes & (REPARSE | OFFLINE | RECALL) != 0 {
        bail!("特殊目录不展开，避免跟随链接或下载云文件");
    }
    let serial = root
        .identity
        .as_deref()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_owned();
    let mut buffer = vec![0u64; 8192];
    let mut first = true;
    loop {
        let class = if first {
            FileIdExtdDirectoryRestartInfo
        } else {
            FileIdExtdDirectoryInfo
        };
        let result = unsafe {
            GetFileInformationByHandleEx(
                handle.0,
                class,
                buffer.as_mut_ptr().cast(),
                (buffer.len() * 8) as u32,
            )
        };
        if let Err(e) = result {
            if e.code().0 as u32 & 0xffff == ERROR_NO_MORE_FILES.0 {
                return Ok(());
            }
            if first {
                return fallback_enumerate(path, emit);
            }
            return Err(e.into());
        }
        first = false;
        let bytes = buffer.len() * 8;
        let mut offset = 0usize;
        loop {
            let name_offset = offset_of!(FILE_ID_EXTD_DIR_INFO, FileName);
            if offset + name_offset > bytes {
                bail!("目录记录越界");
            }
            // The API supplies aligned records; lengths/offsets are checked before reading names.
            let record = unsafe {
                &*buffer
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .cast::<FILE_ID_EXTD_DIR_INFO>()
            };
            let length = record.FileNameLength as usize;
            if !length.is_multiple_of(2) || offset + name_offset + length > bytes {
                bail!("目录名称长度无效");
            }
            let name_slice = unsafe {
                std::slice::from_raw_parts(
                    buffer
                        .as_ptr()
                        .cast::<u8>()
                        .add(offset + name_offset)
                        .cast::<u16>(),
                    length / 2,
                )
            };
            let name = String::from_utf16(name_slice)
                .context("目录包含无法无损转换的名称，已中止此目录")?;
            if name != "." && name != ".." {
                let child = path.join(&name);
                let is_dir = record.FileAttributes & DIRECTORY != 0;
                let entry = FileRecord {
                    path: child
                        .to_str()
                        .ok_or_else(|| anyhow!("路径编码无效"))?
                        .into(),
                    parent: path.to_str().unwrap_or_default().into(),
                    name,
                    is_dir,
                    logical_bytes: if is_dir {
                        0
                    } else {
                        record.EndOfFile.max(0) as u64
                    },
                    allocated_bytes: if record.FileAttributes & (0x200 | 0x800) != 0 {
                        None
                    } else {
                        Some(if is_dir {
                            0
                        } else {
                            record.AllocationSize.max(0) as u64
                        })
                    },
                    modified: file_time(record.LastWriteTime),
                    modified_ticks: record.LastWriteTime,
                    accessed: file_time(record.LastAccessTime),
                    created: file_time(record.CreationTime),
                    identity: (!serial.is_empty()
                        && u128::from_le_bytes(record.FileId.Identifier) != 0)
                        .then(|| {
                            format!(
                                "{}:{:032x}",
                                serial,
                                u128::from_le_bytes(record.FileId.Identifier)
                            )
                        }),
                    attributes: record.FileAttributes,
                    links: 1,
                    file_count: u64::from(!is_dir),
                    complete: true,
                    enumerated: !is_dir,
                    ..Default::default()
                };
                if !emit(entry)? {
                    return Ok(());
                }
            }
            if record.NextEntryOffset == 0 {
                break;
            }
            let next = record.NextEntryOffset as usize;
            if next < name_offset + length || !next.is_multiple_of(8) || offset + next >= bytes {
                bail!("目录记录偏移无效");
            }
            offset += next;
        }
    }
}

fn fallback_enumerate(path: &Path, mut emit: impl FnMut(FileRecord) -> Result<bool>) -> Result<()> {
    for child in std::fs::read_dir(path)? {
        let child = child?;
        let record = inspect(&child.path())?;
        if !emit(record)? {
            break;
        }
    }
    Ok(())
}

pub fn validate_local_path(path: &str) -> Result<std::path::PathBuf> {
    if path.contains('\0') || path.starts_with("\\\\") || path.starts_with("//") || path.len() < 3 {
        bail!("仅支持本地盘符路径，不支持网络、设备或相对路径");
    }
    let b = path.as_bytes();
    if !b[0].is_ascii_alphabetic()
        || b[1] != b':'
        || !matches!(b[2], b'\\' | b'/')
        || path[2..].contains(':')
    {
        bail!("路径必须是本地绝对路径");
    }
    let volume = wide(&format!("{}:\\", &path[..1]));
    let drive = unsafe { GetDriveTypeW(PCWSTR(volume.as_ptr())) };
    if drive != 2 && drive != 3 {
        bail!("只支持本机固定盘和外接盘");
    }
    let p = Path::new(path);
    for ancestor in p.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        let info = inspect(ancestor)?;
        if info.attributes & (REPARSE | OFFLINE | RECALL) != 0 {
            bail!("路径包含链接、挂载点或云占位，请选择实际本地目录");
        }
    }
    let canonical = std::fs::canonicalize(p)?;
    Ok(std::path::PathBuf::from(
        canonical
            .to_str()
            .ok_or_else(|| anyhow!("路径编码无效"))?
            .trim_start_matches("\\\\?\\"),
    ))
}

pub fn free_space(path: &str) -> Result<u64> {
    let w = wide(path);
    let mut free = 0;
    unsafe {
        GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut free), None, None)?;
    }
    Ok(free)
}

pub fn volumes() -> Vec<Volume> {
    let bits = unsafe { GetLogicalDrives() };
    (0..26)
        .filter_map(|index| {
            if bits & (1 << index) == 0 {
                return None;
            }
            let path = format!("{}:\\", (b'A' + index) as char);
            let w = wide(&path);
            let kind = unsafe { GetDriveTypeW(PCWSTR(w.as_ptr())) };
            if kind != 2 && kind != 3 {
                return None;
            }
            let (mut total, mut free, mut serial) = (0, 0, 0);
            let mut label = [0u16; 260];
            let mut fs = [0u16; 64];
            unsafe {
                GetDiskFreeSpaceExW(PCWSTR(w.as_ptr()), Some(&mut free), Some(&mut total), None)
                    .ok()?;
                GetVolumeInformationW(
                    PCWSTR(w.as_ptr()),
                    Some(&mut label),
                    Some(&mut serial),
                    None,
                    None,
                    Some(&mut fs),
                )
                .ok()?;
            }
            let decode = |s: &[u16]| {
                String::from_utf16_lossy(&s[..s.iter().position(|c| *c == 0).unwrap_or(s.len())])
            };
            Some(Volume {
                path,
                label: decode(&label),
                file_system: decode(&fs),
                total_bytes: total,
                free_bytes: free,
                removable: kind == 2,
                identity: format!("{serial:x}"),
            })
        })
        .collect()
}
