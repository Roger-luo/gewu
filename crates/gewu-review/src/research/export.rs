use crate::{context::SourcePage, harness::InvestigationFinding};

use super::{
    ResearchError, ResearchLedger, ResearchNotes, budget,
    synthesis::{FindingCursor, FindingPage},
};

pub(super) fn export_findings(
    notes: &ResearchNotes,
    page: &SourcePage,
) -> Vec<InvestigationFinding> {
    notes
        .findings
        .iter()
        .map(|finding| InvestigationFinding {
            change_ids: vec![page.quote.change_id.clone()],
            evidence_spans: finding
                .evidence
                .iter()
                .map(|evidence| {
                    let prefix = &page.text[..evidence.offset - page.offset];
                    let first =
                        page.first_body_line + prefix.bytes().filter(|byte| *byte == b'\n').count();
                    let last = first + evidence.text.bytes().filter(|byte| *byte == b'\n').count()
                        - usize::from(evidence.text.ends_with('\n'));
                    crate::Quote {
                        change_id: page.quote.change_id.clone(),
                        start: first,
                        end: last,
                    }
                })
                .collect(),
            summary: if finding.questions.is_empty() {
                finding.summary.clone()
            } else {
                format!(
                    "{}\nOpen questions: {}",
                    finding.summary,
                    finding.questions.join("; ")
                )
            },
            evidence: finding
                .evidence
                .iter()
                .map(|evidence| {
                    // Notes have already been validated against this page.
                    let prefix = &page.text[..evidence.offset - page.offset];
                    let first =
                        page.first_body_line + prefix.bytes().filter(|byte| *byte == b'\n').count();
                    let newlines = evidence.text.bytes().filter(|byte| *byte == b'\n').count();
                    let last = first + newlines - usize::from(evidence.text.ends_with('\n'));
                    format!(
                        "{} [{} body lines {}-{}, bytes {}..{}]:\n{}",
                        page.path,
                        page.quote.change_id,
                        first,
                        last,
                        evidence.offset,
                        evidence.offset + evidence.text.len(),
                        evidence.text,
                    )
                })
                .collect(),
        })
        .collect()
}

/// Reserve the largest possible cursor/index envelope before accepting notes.
/// An accepted finding must remain retrievable under the configured page cap.
pub(super) fn check_retrievable(
    finding: &InvestigationFinding,
    ledger: &ResearchLedger,
) -> Result<(), ResearchError> {
    let page = FindingPage {
        source: ledger.source.clone(),
        ledger_sha256: "0".repeat(64),
        start_index: usize::MAX,
        total_findings: usize::MAX,
        findings: vec![finding.clone()],
        next: Some(FindingCursor {
            ledger_sha256: "0".repeat(64),
            index: usize::MAX,
        }),
    };
    budget(
        "exported finding bytes",
        serde_json::to_vec(&page)?.len(),
        ledger.policy.request_bytes,
    )
}
