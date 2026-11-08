use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Serialize, de::DeserializeOwned};

use crate::WorkbenchError;

/// Where the workbench keeps repositories, tasks, and task checkouts:
/// `$GEWU_HOME`, else `$HOME/.gewu`.
#[derive(Clone, Debug)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The home the environment names.
    #[must_use]
    pub fn from_env() -> Self {
        let root = std::env::var_os("GEWU_HOME").map_or_else(
            || {
                std::env::var_os("HOME")
                    .map_or_else(|| PathBuf::from("."), PathBuf::from)
                    .join(".gewu")
            },
            PathBuf::from,
        );
        Self::new(root)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn repositories(&self) -> PathBuf {
        self.root.join("repositories.json")
    }

    pub(crate) fn tasks(&self) -> PathBuf {
        self.root.join("tasks")
    }

    pub(crate) fn task(&self, id: &str) -> PathBuf {
        self.tasks().join(id)
    }

    pub(crate) fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, WorkbenchError> {
        match fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Written to a sibling and renamed, so a crash never leaves half a file.
    pub(crate) fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), WorkbenchError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
        fs::rename(temporary, path)?;
        Ok(())
    }
}

/// Milliseconds since the Unix epoch.
#[must_use]
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
