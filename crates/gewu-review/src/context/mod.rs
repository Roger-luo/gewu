//! Bounded evidence transport over an immutable, complete review manifest.
//!
//! Pages are transport units, not review chunks. Their cursors retain the
//! original source identity and quote coordinates; no sliced `Change` is made.
//! Limits include serialized JSON metadata and escaping, but not any caller's
//! surrounding prompt. Callers must budget the complete model request too.

mod body;
mod inventory;
mod search;
mod source;

use std::{collections::HashMap, sync::OnceLock};

use serde::Serialize;
use thiserror::Error;

use crate::{Change, ReviewInput};

use body::BodyIndex;

pub use inventory::{InventoryCursor, InventoryEntry, InventoryPage};
pub use search::{SearchCursor, SearchMatch, SearchPage};
pub use source::{SourceCursor, SourcePage};

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("search needs a nonempty literal query of at most 256 UTF-8 bytes")]
    InvalidQuery,
    #[error("context budget of {budget} bytes cannot hold the next entry or source character")]
    BudgetTooSmall { budget: usize },
    #[error("context cursor is stale or outside the requested source/span")]
    InvalidCursor,
    #[error("unknown change or invalid hunk-body span")]
    InvalidQuote,
    #[error("could not encode context: {0}")]
    Encoding(#[from] serde_json::Error),
}

/// Read-only projection; the manifest remains the coverage authority.
pub struct ContextReader<'a> {
    input: &'a ReviewInput,
    max_bytes: usize,
    bodies: HashMap<&'a str, (&'a Change, OnceLock<BodyIndex<'a>>)>,
}

impl<'a> ContextReader<'a> {
    #[must_use]
    pub fn new(input: &'a ReviewInput, max_bytes: usize) -> Self {
        Self {
            input,
            max_bytes,
            bodies: input
                .changes
                .iter()
                .map(|change| (change.id.as_str(), (change, OnceLock::new())))
                .collect(),
        }
    }

    fn size(value: &impl Serialize) -> Result<usize, ContextError> {
        Ok(serde_json::to_vec(value)?.len())
    }

    fn bounded<T: Serialize>(&self, value: T) -> Result<T, ContextError> {
        if Self::size(&value)? > self.max_bytes {
            return Err(self.too_small());
        }
        Ok(value)
    }

    fn too_small(&self) -> ContextError {
        ContextError::BudgetTooSmall {
            budget: self.max_bytes,
        }
    }
}
