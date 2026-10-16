use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::ProviderError;

/// Persists a provider access token in the OS secret store. Implementations
/// must never log or serialize the token. Entries are named by the account
/// id a [`crate::id::ConnectionKey`] derives, so a provider can also read the
/// legacy entry a pre-multi-provider install wrote.
pub trait TokenStore: Send + Sync {
    fn get(&self, id: &str) -> Result<Option<String>, ProviderError>;
    fn set(&self, id: &str, token: &str) -> Result<(), ProviderError>;
    fn delete(&self, id: &str) -> Result<(), ProviderError>;
}

const SERVICE: &str = "gitau";

/// OS keychain backed store (Windows Credential Manager, macOS Keychain,
/// freedesktop Secret Service).
pub struct KeyringTokenStore;

impl TokenStore for KeyringTokenStore {
    fn get(&self, id: &str) -> Result<Option<String>, ProviderError> {
        let entry = entry(id)?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keyring_failure(error)),
        }
    }

    fn set(&self, id: &str, token: &str) -> Result<(), ProviderError> {
        let entry = entry(id)?;
        entry.set_password(token).map_err(keyring_failure)
    }

    fn delete(&self, id: &str) -> Result<(), ProviderError> {
        let entry = entry(id)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keyring_failure(error)),
        }
    }
}

fn entry(id: &str) -> Result<keyring::Entry, ProviderError> {
    keyring::Entry::new(SERVICE, id).map_err(keyring_failure)
}

fn keyring_failure(error: keyring::Error) -> ProviderError {
    ProviderError::Keyring {
        message: error.to_string(),
    }
}

/// In-memory store for tests and non-persistent contexts.
#[derive(Default)]
pub struct MemoryTokenStore {
    tokens: Mutex<HashMap<String, String>>,
}

impl TokenStore for MemoryTokenStore {
    fn get(&self, id: &str) -> Result<Option<String>, ProviderError> {
        Ok(self.tokens.lock().unwrap().get(id).cloned())
    }

    fn set(&self, id: &str, token: &str) -> Result<(), ProviderError> {
        self.tokens
            .lock()
            .unwrap()
            .insert(id.to_owned(), token.to_owned());
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), ProviderError> {
        self.tokens.lock().unwrap().remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_deletes_idempotently() {
        let store = MemoryTokenStore::default();
        assert_eq!(store.get("github:github.com").unwrap(), None);
        store.set("github:github.com", "tok-1").unwrap();
        assert_eq!(
            store.get("github:github.com").unwrap(),
            Some("tok-1".into())
        );
        store.delete("github:github.com").unwrap();
        store.delete("github:github.com").unwrap();
        assert_eq!(store.get("github:github.com").unwrap(), None);
    }
}
