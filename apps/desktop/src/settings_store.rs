use anyhow::{anyhow, Result};
use cleaner_domain::{LlmSettings, Settings};
use cleaner_engine::store::Store;
use cleaner_platform::credentials::{self, Credential};

pub fn provider(base_url: &str) -> Result<String> {
    if base_url.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(cleaner_llm::client::endpoint(base_url)?
        .as_str()
        .trim_end_matches('/')
        .to_owned())
}

pub fn load_key(settings: &LlmSettings) -> Result<Option<String>> {
    let expected = provider(&settings.base_url)?;
    Ok(matching_key(credentials::load()?, &expected))
}

fn matching_key(value: Option<Credential>, expected: &str) -> Option<String> {
    value
        .filter(|value| !expected.is_empty() && value.provider == expected)
        .map(|value| value.secret)
}

trait Vault {
    fn read(&self) -> Result<Option<Credential>>;
    fn write(&self, value: Option<&Credential>) -> Result<()>;
}
struct WindowsVault;
impl Vault for WindowsVault {
    fn read(&self) -> Result<Option<Credential>> {
        credentials::load()
    }
    fn write(&self, value: Option<&Credential>) -> Result<()> {
        credentials::save(value)
    }
}

pub fn save(store: &Store, previous: &Settings, next: &Settings, key: Option<&str>) -> Result<()> {
    persist(previous, next, key, &WindowsVault, || {
        store.put("settings", next)
    })
}

fn persist(
    previous: &Settings,
    next: &Settings,
    key: Option<&str>,
    vault: &impl Vault,
    write_settings: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let previous_provider = provider(&previous.llm.base_url)?;
    let next_provider = provider(&next.llm.base_url)?;
    if key.is_none() && previous_provider == next_provider {
        return write_settings();
    }
    let replacement = key.filter(|key| !key.is_empty()).map(|key| Credential {
        provider: next_provider,
        secret: key.to_owned(),
    });
    if let Some(value) = &replacement {
        if value.provider.is_empty() {
            return Err(anyhow!("保存密钥前请填写服务地址"));
        }
        credentials::validate(value)?;
    }
    let original = vault.read()?;
    vault.write(replacement.as_ref())?;
    if let Err(error) = write_settings() {
        if vault.write(original.as_ref()).is_err() {
            return Err(anyhow!(
                "设置保存失败，凭据恢复也失败；请重新填写密钥。未匹配服务地址的密钥不会发送。"
            ));
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct FakeVault {
        value: RefCell<Option<Credential>>,
        writes: Cell<u32>,
        fail_at: u32,
    }
    impl Vault for FakeVault {
        fn read(&self) -> Result<Option<Credential>> {
            Ok(self.value.borrow().clone())
        }
        fn write(&self, value: Option<&Credential>) -> Result<()> {
            let count = self.writes.get() + 1;
            self.writes.set(count);
            if count == self.fail_at {
                return Err(anyhow!("vault failure"));
            }
            *self.value.borrow_mut() = value.cloned();
            Ok(())
        }
    }
    fn fixture() -> (Settings, Settings, FakeVault) {
        let mut old = Settings::default();
        old.llm.base_url = "https://old.example/v1".into();
        let mut next = old.clone();
        next.llm.base_url = "https://new.example/v1".into();
        let vault = FakeVault {
            value: RefCell::new(Some(Credential {
                provider: provider(&old.llm.base_url).unwrap(),
                secret: "old-test-key".into(),
            })),
            writes: Cell::new(0),
            fail_at: 0,
        };
        (old, next, vault)
    }
    #[test]
    fn failed_settings_save_restores_original_credentials() {
        for key in [Some("new-test-key"), Some(""), None] {
            let (old, next, vault) = fixture();
            let before = vault.read().unwrap();
            assert!(persist(&old, &next, key, &vault, || Err(anyhow!("database full"))).is_err());
            assert!(vault.read().unwrap() == before);
        }
    }
    #[test]
    fn invalid_key_and_vault_failure_do_not_write_settings_or_erase_old_key() {
        let (old, next, mut vault) = fixture();
        let before = vault.read().unwrap();
        let too_long = "x".repeat(2401);
        assert!(persist(&old, &next, Some(&too_long), &vault, || panic!(
            "must not write"
        ))
        .is_err());
        assert_eq!(vault.writes.get(), 0);
        vault.fail_at = 1;
        assert!(persist(&old, &next, Some("new"), &vault, || panic!(
            "must not write"
        ))
        .is_err());
        assert!(vault.read().unwrap() == before);
    }
    #[test]
    fn even_failed_compensation_keeps_the_credential_bound_to_the_new_provider() {
        let (old, next, mut vault) = fixture();
        vault.fail_at = 2;
        assert!(persist(&old, &next, Some("new"), &vault, || Err(anyhow!(
            "database full"
        )))
        .is_err());
        let value = vault.read().unwrap().unwrap();
        assert!(matching_key(Some(value), &provider(&old.llm.base_url).unwrap()).is_none());
    }
    #[test]
    fn successful_save_and_same_provider_preservation() {
        let (old, mut next, vault) = fixture();
        persist(&old, &next, Some("new"), &vault, || Ok(())).unwrap();
        assert_eq!(
            vault.read().unwrap().unwrap().provider,
            provider(&next.llm.base_url).unwrap()
        );
        next.llm.base_url = "https://old.example/v1/chat/completions/".into();
        assert_eq!(
            provider(&old.llm.base_url).unwrap(),
            provider(&next.llm.base_url).unwrap()
        );
        persist(&old, &next, None, &vault, || Ok(())).unwrap();
        assert_eq!(vault.writes.get(), 1);
    }
}
