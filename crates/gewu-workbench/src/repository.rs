use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{RepositoryView, WorkbenchError};

/// A Git repository the reviewer added, remembered by its absolute path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct RepositoryRecord {
    pub(crate) id: String,
    pub(crate) path: PathBuf,
}

impl RepositoryRecord {
    /// Resolves `path` to its repository's top level, so adding a
    /// subdirectory adds the repository it belongs to.
    pub(crate) fn locate(path: &str) -> Result<Self, WorkbenchError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(WorkbenchError::invalid(
                "enter the path of a Git repository",
            ));
        }
        let expanded = match path.strip_prefix("~/") {
            Some(rest) => std::env::var_os("HOME").map_or_else(
                || PathBuf::from(path),
                |home| PathBuf::from(home).join(rest),
            ),
            None => PathBuf::from(path),
        };
        let directory = expanded.canonicalize().map_err(|_| {
            WorkbenchError::invalid(format!("{} does not exist", expanded.display()))
        })?;
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&directory)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()?;
        if !output.status.success() {
            return Err(WorkbenchError::invalid(format!(
                "{} is not inside a Git repository",
                directory.display()
            )));
        }
        let top = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        let top = top.canonicalize().unwrap_or(top);
        Ok(Self {
            id: Self::id_for(&top),
            path: top,
        })
    }

    fn id_for(path: &Path) -> String {
        let digest = Sha256::digest(path.to_string_lossy().as_bytes());
        format!("repo-{}", &hex::encode(digest)[..12])
    }

    pub(crate) fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    pub(crate) fn git(&self) -> gewu_git::Repository<'_> {
        gewu_git::Repository::new(&self.path)
    }

    pub(crate) fn view(&self) -> RepositoryView {
        let (branch, dirty, error) = if self.path.is_dir() {
            let git = self.git();
            match git.dirty() {
                Ok(dirty) => (git.branch().unwrap_or(None), dirty, None),
                Err(error) => (None, false, Some(error.to_string())),
            }
        } else {
            (
                None,
                false,
                Some("the repository is no longer at this path".to_owned()),
            )
        };
        RepositoryView {
            id: self.id.clone(),
            name: self.name(),
            path: self.path.display().to_string(),
            branch,
            dirty,
            error,
        }
    }
}
