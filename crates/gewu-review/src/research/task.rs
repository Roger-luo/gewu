use crate::{
    context::SourcePage,
    research::{ResearchError, ResearchNotes, ResearchTask, budget},
};

pub(super) const INSTRUCTIONS: &str = r#"Investigate this frozen source page for a developer unfamiliar with the repository.
Return ONLY JSON: {"schema_version":1,"source":<page.source>,"request_id":<id>,"findings":[{"summary":"concrete source-backed behavior, conditions and limitations","evidence":[{"offset":<UTF-8 byte offset within the original quote body>,"text":"exact short excerpt from page.text"}],"questions":["unresolved cross-file relationship or missing evidence"]}]}.
Account for what the supplied page does. Do not infer whole-file behavior from a fragment.
Evidence offsets start at page.offset, not zero; copied excerpts must match exactly.
Describe observed source, not unexecuted runtime guarantees. Name consumers or dependencies
only when this page establishes them; carry unresolved relationships as questions for synthesis.
Include a finding for distinct consequential behaviors rather than merely listing symbols.
For generated or binary artifacts, state the provenance evidence and the limit of inspection.
Transport page boundaries are not semantic work-item or review-chunk boundaries.
Do not claim the repository is fully investigated or the code approved. These notes are local
evidence for later cross-file synthesis, planning and independent quality assessment.
Every finding needs a nonempty summary and an exact evidence excerpt. Keep the entire response
under the checkpoint byte budget supplied by the caller. An empty source page may cite an empty
excerpt at its offset and explain that no textual behavior can be established."#;

impl ResearchNotes {
    pub(super) fn validate(&self, task: &ResearchTask, limit: usize) -> Result<(), ResearchError> {
        budget("checkpoint bytes", serde_json::to_vec(self)?.len(), limit)?;
        if self.schema_version != 1 || self.source != task.page.source {
            return Err(ResearchError::Stale);
        }
        if self.request_id != task.id {
            return Err(ResearchError::WrongRequest);
        }
        if self.findings.is_empty() {
            return Err(ResearchError::InvalidFinding);
        }
        for finding in &self.findings {
            if finding.summary.trim().is_empty()
                || finding.evidence.is_empty()
                || finding
                    .evidence
                    .iter()
                    .any(|evidence| !matches(&task.page, evidence.offset, &evidence.text))
            {
                return Err(ResearchError::InvalidFinding);
            }
        }
        Ok(())
    }
}

fn matches(page: &SourcePage, offset: usize, text: &str) -> bool {
    let Some(start) = offset.checked_sub(page.offset) else {
        return false;
    };
    if text.is_empty() {
        return page.text.is_empty() && start == 0;
    }
    let Some(end) = start.checked_add(text.len()) else {
        return false;
    };
    page.text.get(start..end) == Some(text)
}
