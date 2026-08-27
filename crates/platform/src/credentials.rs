use crate::wide;
use anyhow::{bail, Result};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::Security::Credentials::*,
};
const TARGET: &str = "IntellDiskCleaner/LLM/v1";

pub fn save(secret: &str) -> Result<()> {
    if secret.len() > 2400 {
        bail!("密钥过长");
    }
    if secret.is_empty() {
        return clear();
    }
    let mut target = wide(TARGET);
    let mut blob = secret.as_bytes().to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    let result = unsafe { CredWriteW(&credential, 0) };
    blob.fill(0);
    result?;
    Ok(())
}

pub fn load() -> Result<Option<String>> {
    let target = wide(TARGET);
    let mut credential = std::ptr::null_mut();
    let result = unsafe {
        CredReadW(
            PCWSTR(target.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut credential,
        )
    };
    if let Err(e) = result {
        if e.code().0 as u32 & 0xffff == 1168 {
            return Ok(None);
        }
        return Err(e.into());
    }
    let result = unsafe {
        let bytes = std::slice::from_raw_parts(
            (*credential).CredentialBlob,
            (*credential).CredentialBlobSize as usize,
        );
        let text = String::from_utf8(bytes.to_vec());
        CredFree(credential.cast());
        text
    };
    Ok(Some(result?))
}

pub fn clear() -> Result<()> {
    let target = wide(TARGET);
    match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(()),
        Err(e) if e.code().0 as u32 & 0xffff == 1168 => Ok(()),
        Err(e) => Err(e.into()),
    }
}
