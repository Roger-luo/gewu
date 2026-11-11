//! Every proofread verdict a run reaches, kept with the chunks as they were
//! when read. `proofread.json` is consumed once acted on, and a rewrite
//! replaces the chunk it judged, so without this log nothing is left to
//! measure a cheaper first reader against.

use std::{fs, io::Write};

use serde::Serialize;

use super::{HarnessStore, HarnessStoreError, Proofread};
use crate::ReviewChunk;

#[derive(Serialize)]
struct Reading<'a> {
    schema_version: u32,
    /// How many times the batch was rewritten before this reading.
    rewrites: u32,
    chunks: &'a [&'a ReviewChunk],
    proofread: &'a Proofread,
}

impl HarnessStore {
    #[must_use]
    pub fn verdicts_path(&self) -> std::path::PathBuf {
        self.root().join("proofreads.jsonl")
    }

    /// Appends one proofread, with the chunks it scored, to the run's log.
    pub fn record_proofread(
        &self,
        rewrites: u32,
        chunks: &[&ReviewChunk],
        proofread: &Proofread,
    ) -> Result<(), HarnessStoreError> {
        let path = self.verdicts_path();
        let reading = Reading {
            schema_version: 1,
            rewrites,
            chunks,
            proofread,
        };
        let mut line = serde_json::to_vec(&reading).map_err(|source| HarnessStoreError::Json {
            path: path.clone(),
            source,
        })?;
        line.push(b'\n');
        let io = |source| HarnessStoreError::Io {
            path: path.clone(),
            source,
        };
        fs::create_dir_all(self.root()).map_err(io)?;
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&line))
            .map_err(io)
    }
}
