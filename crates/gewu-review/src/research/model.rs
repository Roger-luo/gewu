use serde::{Deserialize, Serialize};

use crate::{SourceIdentity, context::SourcePage};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResearchPolicy {
    pub page_bytes: usize,
    pub request_bytes: usize,
    pub checkpoint_bytes: usize,
    /// Limits accepted batches, not provider calls or retries. A provider
    /// adapter must separately account for every actual inference request.
    pub max_batches: usize,
}

impl Default for ResearchPolicy {
    fn default() -> Self {
        Self {
            page_bytes: 12_288,
            request_bytes: 24_576,
            checkpoint_bytes: 4096,
            max_batches: 256,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// Byte offset in the original quote's normalized UTF-8 body, not in the page.
    pub offset: usize,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub summary: String,
    pub evidence: Vec<Evidence>,
    pub questions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchNotes {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub request_id: String,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResearchTask {
    pub schema_version: u32,
    pub id: String,
    pub instructions: String,
    pub page: SourcePage,
}

/// Untrusted serialized state. `ResearchRun::resume` revalidates every record
/// against its original request before allowing new work. No cursor is trusted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResearchLedger {
    pub(super) schema_version: u32,
    pub(super) source: SourceIdentity,
    pub(super) manifest_sha256: String,
    pub(super) policy: ResearchPolicy,
    pub(super) records: Vec<ResearchNotes>,
}
