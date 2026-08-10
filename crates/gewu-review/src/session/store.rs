use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::{ReviewEvent, ReviewSession};

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("could not access review session: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid event at line {line}: {source}")]
    InvalidEvent {
        line: usize,
        source: serde_json::Error,
    },
}

#[derive(Clone, Debug)]
pub struct EventLog {
    path: PathBuf,
}

impl EventLog {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, event: &ReviewEvent) -> Result<(), SessionError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, event)
            .map_err(|source| SessionError::InvalidEvent { line: 0, source })?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        Ok(())
    }

    pub fn load(&self) -> Result<ReviewSession, SessionError> {
        if !self.path.exists() {
            return Ok(ReviewSession::default());
        }
        let file = File::open(&self.path)?;
        let mut session = ReviewSession::default();
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event =
                serde_json::from_str(&line).map_err(|source| SessionError::InvalidEvent {
                    line: index + 1,
                    source,
                })?;
            session.apply(&event);
        }
        Ok(session)
    }
}
