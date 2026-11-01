use serde::{Deserialize, Serialize};

use crate::SourceIdentity;

use super::{ContextError, ContextReader};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryCursor {
    pub source: SourceIdentity,
    pub index: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryEntry {
    pub id: String,
    pub path: String,
    pub body_lines: usize,
    pub patch_bytes: usize,
    pub patch_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryPage {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub total_changes: usize,
    pub entries: Vec<InventoryEntry>,
    pub next: Option<InventoryCursor>,
}

impl ContextReader<'_> {
    pub fn inventory(
        &self,
        cursor: Option<&InventoryCursor>,
    ) -> Result<InventoryPage, ContextError> {
        let start = match cursor {
            Some(cursor)
                if cursor.source == self.input.source
                    && cursor.index < self.input.changes.len() =>
            {
                cursor.index
            }
            Some(_) => return Err(ContextError::InvalidCursor),
            None => 0,
        };
        let mut page = InventoryPage {
            schema_version: 1,
            source: self.input.source.clone(),
            total_changes: self.input.changes.len(),
            entries: vec![],
            next: None,
        };
        for (index, change) in self.input.changes.iter().enumerate().skip(start) {
            let previous = page.next.clone();
            page.entries.push(InventoryEntry {
                id: change.id.clone(),
                path: change.path.clone(),
                body_lines: change.body_lines(),
                patch_bytes: change.patch.len(),
                patch_sha256: change.patch_sha256(),
            });
            page.next = (index + 1 < self.input.changes.len()).then(|| InventoryCursor {
                source: self.input.source.clone(),
                index: index + 1,
            });
            if Self::size(&page)? > self.max_bytes {
                page.entries.pop();
                page.next = previous;
                if page.entries.is_empty() {
                    return Err(self.too_small());
                }
                break;
            }
        }
        self.bounded(page)
    }
}
