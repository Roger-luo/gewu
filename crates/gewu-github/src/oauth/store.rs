//! Where the reviewer's OAuth token is kept between launches.

use std::sync::Mutex;

use crate::GitHubError;

pub trait TokenStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, GitHubError>;
    fn set(&self, token: &str) -> Result<(), GitHubError>;
    fn delete(&self) -> Result<(), GitHubError>;
}

/// The system keychain: the macOS Keychain through `/usr/bin/security`, or
/// the Windows credential store.
pub struct Keychain {
    #[cfg(target_os = "macos")]
    store: super::security::SecurityCli,
    #[cfg(not(target_os = "macos"))]
    entry: keyring::Entry,
}

impl Keychain {
    /// Named like `gh`'s `gh:github.com`.
    const SERVICE: &'static str = "gewu:github.com";
    const ACCOUNT: &'static str = "oauth";

    #[cfg(target_os = "macos")]
    pub fn open() -> Result<Self, GitHubError> {
        Ok(Self {
            store: super::security::SecurityCli::new(
                "/usr/bin/security",
                Self::SERVICE,
                Self::ACCOUNT,
            ),
        })
    }

    #[cfg(not(target_os = "macos"))]
    pub fn open() -> Result<Self, GitHubError> {
        keyring::Entry::new(Self::SERVICE, Self::ACCOUNT)
            .map(|entry| Self { entry })
            .map_err(keychain)
    }
}

#[cfg(target_os = "macos")]
impl TokenStore for Keychain {
    fn get(&self) -> Result<Option<String>, GitHubError> {
        self.store.get()
    }

    fn set(&self, token: &str) -> Result<(), GitHubError> {
        self.store.set(token)
    }

    fn delete(&self) -> Result<(), GitHubError> {
        self.store.delete()
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain(error: keyring::Error) -> GitHubError {
    GitHubError::Keychain(error.to_string())
}

#[cfg(not(target_os = "macos"))]
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

/// Remembers what `S` holds after the first read, so the keychain is read
/// once per run. macOS can ask the reviewer before every keychain read (an
/// unsigned development build always does), and Gewu calls GitHub often.
pub struct Cached<S> {
    store: S,
    /// `None` until read; then the token, or `Some(None)` for none saved.
    token: Mutex<Option<Option<String>>>,
}

impl<S> Cached<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            token: Mutex::new(None),
        }
    }

    fn token(&self) -> std::sync::MutexGuard<'_, Option<Option<String>>> {
        self.token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl<S: TokenStore> TokenStore for Cached<S> {
    fn get(&self) -> Result<Option<String>, GitHubError> {
        // Held across the read, so concurrent calls do not each ask.
        let mut token = self.token();
        if let Some(known) = token.as_ref() {
            return Ok(known.clone());
        }
        // A refused read is not remembered: the next call may ask again.
        let read = self.store.get()?;
        *token = Some(read.clone());
        Ok(read)
    }

    fn set(&self, token: &str) -> Result<(), GitHubError> {
        self.store.set(token)?;
        *self.token() = Some(Some(token.to_string()));
        Ok(())
    }

    fn delete(&self) -> Result<(), GitHubError> {
        self.store.delete()?;
        *self.token() = Some(None);
        Ok(())
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{Cached, MemoryStore, TokenStore};
    use crate::GitHubError;

    /// Counts reads, as a keychain prompt would.
    #[derive(Default)]
    struct Counting {
        inner: MemoryStore,
        reads: AtomicUsize,
    }

    impl TokenStore for &Counting {
        fn get(&self) -> Result<Option<String>, GitHubError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.inner.get()
        }
        fn set(&self, token: &str) -> Result<(), GitHubError> {
            self.inner.set(token)
        }
        fn delete(&self) -> Result<(), GitHubError> {
            self.inner.delete()
        }
    }

    #[test]
    fn reads_the_store_once_per_run() {
        let counting = Counting::default();
        counting.inner.set("saved").expect("set");
        let cached = Cached::new(&counting);
        for _ in 0..5 {
            assert_eq!(cached.get().expect("get").as_deref(), Some("saved"));
        }
        assert_eq!(counting.reads.load(Ordering::SeqCst), 1);

        cached.delete().expect("delete");
        assert_eq!(cached.get().expect("get"), None);
        cached.set("new").expect("set");
        assert_eq!(cached.get().expect("get").as_deref(), Some("new"));
        assert_eq!(
            counting.reads.load(Ordering::SeqCst),
            1,
            "writes need no read"
        );
    }

    #[test]
    fn nothing_saved_is_remembered_too() {
        let counting = Counting::default();
        let cached = Cached::new(&counting);
        assert_eq!(cached.get().expect("get"), None);
        assert_eq!(cached.get().expect("get"), None);
        assert_eq!(counting.reads.load(Ordering::SeqCst), 1);
    }
}
