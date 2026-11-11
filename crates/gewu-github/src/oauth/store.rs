//! Where the reviewer's OAuth token is kept between launches.

use std::sync::Mutex;

use crate::GitHubError;

pub trait TokenStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, GitHubError>;
    fn set(&self, token: &str) -> Result<(), GitHubError>;
    fn delete(&self) -> Result<(), GitHubError>;
}

/// The system keychain: the macOS Keychain, or the Windows credential store.
pub struct Keychain {
    entry: keyring::Entry,
}

impl Keychain {
    const SERVICE: &'static str = "gewu";
    const ACCOUNT: &'static str = "github.com";

    pub fn open() -> Result<Self, GitHubError> {
        keyring::Entry::new(Self::SERVICE, Self::ACCOUNT)
            .map(|entry| Self { entry })
            .map_err(keychain)
    }
}

fn keychain(error: keyring::Error) -> GitHubError {
    GitHubError::Keychain(error.to_string())
}

impl TokenStore for Keychain {
    fn get(&self) -> Result<Option<String>, GitHubError> {
        match self.entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(keychain(error)),
        }
    }

    fn set(&self, token: &str) -> Result<(), GitHubError> {
        self.entry.set_password(token).map_err(keychain)
    }

    fn delete(&self) -> Result<(), GitHubError> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(keychain(error)),
        }
    }
}

/// A store that forgets on exit, for tests.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl MemoryStore {
    fn slot(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl TokenStore for MemoryStore {
    fn get(&self) -> Result<Option<String>, GitHubError> {
        Ok(self.slot().clone())
    }

    fn set(&self, token: &str) -> Result<(), GitHubError> {
        *self.slot() = Some(token.to_string());
        Ok(())
    }

    fn delete(&self) -> Result<(), GitHubError> {
        *self.slot() = None;
        Ok(())
    }
}
