use serde::{Deserialize, Serialize};

use crate::{
    SourceIdentity,
    harness::{InvestigationCheckpoint, InvestigationFinding},
};

use super::{ResearchError, ResearchRun, budget};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FindingCursor {
    pub ledger_sha256: String,
    pub index: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FindingPage {
    pub source: SourceIdentity,
    pub ledger_sha256: String,
    pub start_index: usize,
    pub total_findings: usize,
    pub findings: Vec<InvestigationFinding>,
    pub next: Option<FindingCursor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupportedText {
    pub text: String,
    /// Zero-based indexes in the completed investigation's findings.
    pub findings: Vec<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroundedOverview {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub ledger_sha256: String,
    pub intent: SupportedText,
    pub changes: Vec<SupportedText>,
    pub concepts: Vec<SupportedText>,
}

impl ResearchRun<'_> {
    pub fn finding_page(
        &self,
        cursor: Option<&FindingCursor>,
        max_bytes: usize,
    ) -> Result<FindingPage, ResearchError> {
        let findings = self.findings()?;
        let hash = self.completed_hash()?.to_owned();
        let start = match cursor {
            Some(cursor) if cursor.ledger_sha256 == hash && cursor.index < findings.len() => {
                cursor.index
            }
            Some(_) => return Err(ResearchError::Stale),
            None => 0,
        };
        let mut page = FindingPage {
            source: self.ledger().source.clone(),
            ledger_sha256: hash.clone(),
            start_index: start,
            total_findings: findings.len(),
            findings: vec![],
            next: None,
        };
        let limit = max_bytes.min(self.ledger().policy.request_bytes);
        for (index, finding) in findings.iter().enumerate().skip(start) {
            let previous = page.next.clone();
            page.findings.push(finding.clone());
            page.next = (index + 1 < page.total_findings).then(|| FindingCursor {
                ledger_sha256: hash.clone(),
                index: index + 1,
            });
            if serde_json::to_vec(&page)?.len() > limit {
                page.findings.pop();
                page.next = previous;
                if page.findings.is_empty() {
                    return Err(ResearchError::Budget {
                        kind: "finding page bytes",
                        actual: limit.saturating_add(1),
                        limit,
                    });
                }
                break;
            }
        }
        budget(
            "finding page bytes",
            serde_json::to_vec(&page)?.len(),
            limit,
        )?;
        Ok(page)
    }

    pub fn assemble(
        &self,
        overview: &GroundedOverview,
    ) -> Result<InvestigationCheckpoint, ResearchError> {
        budget(
            "overview bytes",
            serde_json::to_vec(overview)?.len(),
            self.ledger().policy.checkpoint_bytes,
        )?;
        let findings = self.findings()?;
        if overview.schema_version != 1
            || overview.source != self.ledger().source
            || overview.ledger_sha256 != self.completed_hash()?
        {
            return Err(ResearchError::Stale);
        }
        let claims = std::iter::once(&overview.intent)
            .chain(&overview.changes)
            .chain(&overview.concepts);
        if overview.changes.is_empty()
            || overview.changes.len() > 5
            || claims.into_iter().any(|claim| {
                claim.text.trim().is_empty()
                    || claim.findings.is_empty()
                    || claim.findings.iter().any(|index| *index >= findings.len())
            })
        {
            return Err(ResearchError::InvalidOverview);
        }
        Ok(InvestigationCheckpoint {
            schema_version: 1,
            source: overview.source.clone(),
            intent: overview.intent.text.clone(),
            changes: overview
                .changes
                .iter()
                .map(|claim| claim.text.clone())
                .collect(),
            concepts: overview
                .concepts
                .iter()
                .map(|claim| claim.text.clone())
                .collect(),
            illustration: None,
            findings: findings.to_vec(),
        })
    }
}
