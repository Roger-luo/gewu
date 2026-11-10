use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

use super::{Expansion, HarnessState, InvestigationCheckpoint, Proofread, Revision, WorkList};

#[derive(Debug, Error)]
pub enum HarnessStoreError {
    #[error("could not access harness artifact `{path}`: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid harness artifact `{path}`: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

#[derive(Clone, Debug)]
pub struct HarnessStore {
    root: PathBuf,
}

impl HarnessStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn state_path(&self) -> PathBuf {
        self.root.join("state.json")
    }

    #[must_use]
    pub fn investigation_path(&self) -> PathBuf {
        self.root.join("investigation.json")
    }

    #[must_use]
    pub fn work_list_path(&self) -> PathBuf {
        self.root.join("worklist.json")
    }

    #[must_use]
    pub fn expansion_path(&self) -> PathBuf {
        self.root.join("expansion.json")
    }

    pub fn state(&self) -> Result<Option<HarnessState>, HarnessStoreError> {
        self.read_optional(&self.state_path())
    }

    pub fn save_state(&self, state: &HarnessState) -> Result<(), HarnessStoreError> {
        self.write_atomic(&self.state_path(), state)
    }

    pub fn investigation(&self) -> Result<Option<InvestigationCheckpoint>, HarnessStoreError> {
        self.read_optional(&self.investigation_path())
    }

    pub fn save_investigation(
        &self,
        checkpoint: &InvestigationCheckpoint,
    ) -> Result<(), HarnessStoreError> {
        self.write_atomic(&self.investigation_path(), checkpoint)
    }

    pub fn work_list(&self) -> Result<Option<WorkList>, HarnessStoreError> {
        self.read_optional(&self.work_list_path())
    }

    pub fn save_work_list(&self, list: &WorkList) -> Result<(), HarnessStoreError> {
        self.write_atomic(&self.work_list_path(), list)
    }

    #[must_use]
    pub fn revision_path(&self) -> PathBuf {
        self.root.join("revision.json")
    }

    pub fn expansion(&self) -> Result<Option<Expansion>, HarnessStoreError> {
        self.read_optional(&self.expansion_path())
    }

    pub fn discard_expansion(&self) -> Result<(), HarnessStoreError> {
        self.discard(&self.expansion_path())
    }

    pub fn revision(&self) -> Result<Option<Revision>, HarnessStoreError> {
        self.read_optional(&self.revision_path())
    }

    pub fn discard_revision(&self) -> Result<(), HarnessStoreError> {
        self.discard(&self.revision_path())
    }

    #[must_use]
    pub fn proofread_path(&self) -> PathBuf {
        self.root.join("proofread.json")
    }

    pub fn proofread(&self) -> Result<Option<Proofread>, HarnessStoreError> {
        self.read_optional(&self.proofread_path())
    }

    pub fn discard_proofread(&self) -> Result<(), HarnessStoreError> {
        self.discard(&self.proofread_path())
    }

    fn discard(&self, path: &Path) -> Result<(), HarnessStoreError> {
        if path.exists() {
            fs::remove_file(path).map_err(|source| HarnessStoreError::Io {
                path: path.to_owned(),
                source,
            })?;
        }
        Ok(())
    }

    pub fn reset(&self) -> Result<(), HarnessStoreError> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root).map_err(|source| HarnessStoreError::Io {
                path: self.root.clone(),
                source,
            })?;
        }
        Ok(())
    }

    fn read_optional<T: DeserializeOwned>(
        &self,
        path: &Path,
    ) -> Result<Option<T>, HarnessStoreError> {
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read(path).map_err(|source| HarnessStoreError::Io {
            path: path.to_owned(),
            source,
        })?;
        serde_json::from_slice(&contents)
            .map(Some)
            .map_err(|source| HarnessStoreError::Json {
                path: path.to_owned(),
                source,
            })
    }

    fn write_atomic<T: Serialize>(&self, path: &Path, value: &T) -> Result<(), HarnessStoreError> {
        fs::create_dir_all(&self.root).map_err(|source| HarnessStoreError::Io {
            path: self.root.clone(),
            source,
        })?;
        let temporary = path.with_extension("json.tmp");
        let contents =
            serde_json::to_vec_pretty(value).map_err(|source| HarnessStoreError::Json {
                path: path.to_owned(),
                source,
            })?;
        fs::write(&temporary, contents).map_err(|source| HarnessStoreError::Io {
            path: temporary.clone(),
            source,
        })?;
        fs::rename(&temporary, path).map_err(|source| HarnessStoreError::Io {
            path: path.to_owned(),
            source,
        })
    }
}
