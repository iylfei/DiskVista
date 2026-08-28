use crate::wide;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::Security::Credentials::*,
};
const TARGET: &str = "IntellDiskCleaner/LLM/v1";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    pub provider: String,
    pub secret: String,
}

pub fn validate(value: &Credential) -> Result<()> {
    if value.secret.is_empty() || value.secret.len() > 2400 {
        bail!("密钥过长");
    }
    if serde_json::to_vec(value)?.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
        bail!("服务地址与密钥超过 Windows 凭据存储限制");
    }
    Ok(())
}

pub fn save(value: Option<&Credential>) -> Result<()> {
    let Some(value) = value else { return clear() };
    validate(value)?;
    let mut target = wide(TARGET);
    let mut blob = serde_json::to_vec(value)?;
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

pub fn load() -> Result<Option<Credential>> {
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
        let bytes = if (*credential).CredentialBlobSize == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(
                (*credential).CredentialBlob,
                (*credential).CredentialBlobSize as usize,
            )
        };
        // Unbound credentials from older versions must never be sent to a provider.
        let value = serde_json::from_slice::<Credential>(bytes).ok();
        CredFree(credential.cast());
        value
    };
    Ok(result)
}

fn clear() -> Result<()> {
    let target = wide(TARGET);
    match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(()),
        Err(e) if e.code().0 as u32 & 0xffff == 1168 => Ok(()),
        Err(e) => Err(e.into()),
    }
}
