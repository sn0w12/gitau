use crate::error::{GitError, Result};

/// Persists the GitHub access token in the OS secret store. Implementations
/// must never log or serialize the token.
pub trait TokenStore: Send + Sync {
    fn get(&self) -> Result<Option<String>>;
    fn set(&self, token: &str) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

const SERVICE: &str = "gitau";
const ACCOUNT: &str = "github";

/// OS keychain backed store (Windows Credential Manager, macOS Keychain,
/// freedesktop Secret Service).
pub struct KeyringTokenStore;

impl TokenStore for KeyringTokenStore {
    fn get(&self) -> Result<Option<String>> {
        let entry = entry()?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_failure(error)),
        }
    }

    fn set(&self, token: &str) -> Result<()> {
        let entry = entry()?;
        entry.set_password(token).map_err(keyring_failure)
    }

    fn delete(&self) -> Result<()> {
        let entry = entry()?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_failure(error)),
        }
    }
}

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(keyring_failure)
}

fn keyring_failure(error: keyring::Error) -> GitError {
    GitError::internal(format!("token storage failure: {error}"))
}

/// In-memory store for tests and non-persistent contexts.
#[derive(Default)]
pub struct MemoryTokenStore {
    token: std::sync::Mutex<Option<String>>,
}

impl TokenStore for MemoryTokenStore {
    fn get(&self) -> Result<Option<String>> {
        Ok(self.token.lock().unwrap().clone())
    }

    fn set(&self, token: &str) -> Result<()> {
        *self.token.lock().unwrap() = Some(token.to_owned());
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        *self.token.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_deletes_idempotently() {
        let store = MemoryTokenStore::default();
        assert_eq!(store.get().unwrap(), None);
        store.set("tok-1").unwrap();
        assert_eq!(store.get().unwrap(), Some("tok-1".into()));
        store.set("tok-2").unwrap();
        assert_eq!(store.get().unwrap(), Some("tok-2".into()));
        store.delete().unwrap();
        store.delete().unwrap();
        assert_eq!(store.get().unwrap(), None);
    }
}
