//! API keys, one per provider, in the system keychain (macOS Keychain; Windows Credential
//! Manager; Linux kernel keyring). Service [`SERVICE`], account = the provider id.
//!
//! Keys live only there: never in settings files, logs, crash reports or command replies. Code
//! that shows a key shows [`crate::masked`] (the last four characters).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::AssistantError;

/// The keychain service name keys are stored under.
pub const SERVICE: &str = "ai.spert.margin.assistant";

/// Where keys are kept.
pub trait KeyStore: Send + Sync {
    /// Whether keys can be stored here at all (false: the key field says so and keeps nothing).
    fn supported(&self) -> bool;
    /// The key for `account`, if one is stored.
    fn get(&self, account: &str) -> Result<Option<String>, AssistantError>;
    fn set(&self, account: &str, key: &str) -> Result<(), AssistantError>;
    /// Remove the key; removing a missing key is not an error.
    fn remove(&self, account: &str) -> Result<(), AssistantError>;
}

/// Keys held in memory only, forgotten when the app quits: tests, the screenshot tool, and hosts
/// with no keychain wired in.
#[derive(Default)]
pub struct MemoryKeys(Mutex<HashMap<String, String>>);

impl MemoryKeys {
    fn with<R>(&self, f: impl FnOnce(&mut HashMap<String, String>) -> R) -> R {
        let mut g = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut g)
    }
}

impl KeyStore for MemoryKeys {
    fn supported(&self) -> bool {
        true
    }
    fn get(&self, account: &str) -> Result<Option<String>, AssistantError> {
        Ok(self.with(|m| m.get(account).cloned()))
    }
    fn set(&self, account: &str, key: &str) -> Result<(), AssistantError> {
        self.with(|m| m.insert(account.to_string(), key.to_string()));
        Ok(())
    }
    fn remove(&self, account: &str) -> Result<(), AssistantError> {
        self.with(|m| m.remove(account));
        Ok(())
    }
}

/// The system keychain, where `keyring` has a native store (macOS, Windows, Linux). Elsewhere
/// (the BSDs, the browser) [`KeyStore::supported`] is false and nothing is stored.
#[derive(Default)]
pub struct SystemKeys;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod system {
    use super::*;

    fn entry(account: &str) -> Result<keyring::Entry, AssistantError> {
        keyring::Entry::new(SERVICE, account).map_err(fail)
    }

    // The keyring error never carries the secret; it is still kept short.
    fn fail(e: keyring::Error) -> AssistantError {
        let why = match e {
            keyring::Error::NoStorageAccess(_) => "the keychain is locked or Margin was not allowed to use it".to_string(),
            keyring::Error::PlatformFailure(_) => "the keychain reported an error".to_string(),
            other => crate::wire::scrub(&other.to_string(), None),
        };
        AssistantError::Unsupported(format!("couldn't use the keychain: {why}"))
    }

    impl KeyStore for SystemKeys {
        fn supported(&self) -> bool {
            true
        }
        fn get(&self, account: &str) -> Result<Option<String>, AssistantError> {
            match entry(account)?.get_password() {
                Ok(k) => Ok(Some(k)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(fail(e)),
            }
        }
        fn set(&self, account: &str, key: &str) -> Result<(), AssistantError> {
            entry(account)?.set_password(key).map_err(fail)
        }
        fn remove(&self, account: &str) -> Result<(), AssistantError> {
            match entry(account)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(fail(e)),
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod system {
    use super::*;

    fn none() -> AssistantError {
        AssistantError::Unsupported("this system has no keychain Margin can use, so the key can't be stored".into())
    }

    impl KeyStore for SystemKeys {
        fn supported(&self) -> bool {
            false
        }
        fn get(&self, _account: &str) -> Result<Option<String>, AssistantError> {
            Ok(None)
        }
        fn set(&self, _account: &str, _key: &str) -> Result<(), AssistantError> {
            Err(none())
        }
        fn remove(&self, _account: &str) -> Result<(), AssistantError> {
            Ok(())
        }
    }
}
