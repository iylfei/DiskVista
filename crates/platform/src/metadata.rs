use anyhow::Result;
use cleaner_domain::Evidence;
use windows::{
    core::PCWSTR,
    Win32::{Foundation::HWND, Security::WinTrust::*, Storage::FileSystem::*},
};
pub fn executable_evidence(path: &str) -> Result<Vec<Evidence>> {
    let p = std::path::Path::new(path);
    let file = crate::filesystem::inspect(p)?;
    if file.is_dir
        || file.attributes
            & (crate::filesystem::REPARSE | crate::filesystem::OFFLINE | crate::filesystem::RECALL)
            != 0
        || p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("exe"))
    {
        return Ok(vec![]);
    }
    let w = crate::wide(path);
    let mut evidence = Vec::new();
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(w.as_ptr()), None);
        if size > 0 && size < 4_194_304 {
            let mut bytes = vec![0u8; size as usize];
            if GetFileVersionInfoW(PCWSTR(w.as_ptr()), None, size, bytes.as_mut_ptr().cast())
                .is_ok()
            {
                let root = crate::wide("\\");
                let mut ptr = std::ptr::null_mut();
                let mut len = 0;
                if VerQueryValueW(
                    bytes.as_ptr().cast(),
                    PCWSTR(root.as_ptr()),
                    &mut ptr,
                    &mut len,
                )
                .as_bool()
                    && len as usize >= std::mem::size_of::<VS_FIXEDFILEINFO>()
                {
                    let v = std::ptr::read_unaligned(ptr.cast::<VS_FIXEDFILEINFO>());
                    evidence.push(Evidence {
                        source: "程序版本资源".into(),
                        detail: format!(
                            "{}.{}.{}.{}（资源声明，不是创建来源证明）",
                            v.dwFileVersionMS >> 16,
                            v.dwFileVersionMS & 65535,
                            v.dwFileVersionLS >> 16,
                            v.dwFileVersionLS & 65535
                        ),
                    });
                }
            }
        }
        let mut file = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: PCWSTR(w.as_ptr()),
            ..Default::default()
        };
        let mut data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            dwStateAction: WTD_STATEACTION_VERIFY,
            dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
            Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
            ..Default::default()
        };
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let result = WinVerifyTrust(
            HWND(std::ptr::null_mut()),
            &mut action,
            &mut data as *mut _ as *mut _,
        );
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        let _ = WinVerifyTrust(
            HWND(std::ptr::null_mut()),
            &mut action,
            &mut data as *mut _ as *mut _,
        );
        evidence.push(Evidence {
            source: "离线数字签名检查".into(),
            detail: if result == 0 {
                "本机缓存信任链验证通过；不访问网络，不代表文件可以删除".into()
            } else {
                format!(
                    "无法离线确认签名有效性（0x{:08x}），不据此降低风险",
                    result as u32
                )
            },
        });
    }
    Ok(evidence)
}
