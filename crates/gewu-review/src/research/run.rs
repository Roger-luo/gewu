use std::sync::OnceLock;

use crate::{
    Quote, ReviewInput,
    context::{ContextReader, SourceCursor},
    harness::InvestigationFinding,
};

use super::{
    ResearchError, ResearchLedger, ResearchNotes, ResearchPolicy, ResearchTask, budget, digest,
    task::INSTRUCTIONS,
};

/// A validated in-process session. Reuse it across calls to retain source indexes.
pub struct ResearchRun<'a> {
    input: &'a ReviewInput,
    reader: ContextReader<'a>,
    ledger: ResearchLedger,
    change: usize,
    cursor: Option<SourceCursor>,
    exported: Vec<InvestigationFinding>,
    completed_hash: OnceLock<String>,
}

impl<'a> ResearchRun<'a> {
    pub fn new(input: &'a ReviewInput, policy: ResearchPolicy) -> Result<Self, ResearchError> {
        if [
            policy.page_bytes,
            policy.request_bytes,
            policy.checkpoint_bytes,
            policy.max_batches,
        ]
        .contains(&0)
        {
            return Err(ResearchError::InvalidPolicy);
        }
        Ok(Self {
            input,
            reader: ContextReader::new(input, policy.page_bytes),
            ledger: ResearchLedger {
                schema_version: 1,
                source: input.source.clone(),
                manifest_sha256: digest(input)?,
                policy,
                records: vec![],
            },
            change: 0,
            cursor: None,
            exported: vec![],
            completed_hash: OnceLock::new(),
        })
    }

    pub fn resume(input: &'a ReviewInput, ledger: ResearchLedger) -> Result<Self, ResearchError> {
        if ledger.schema_version != 1
            || ledger.source != input.source
            || ledger.manifest_sha256 != digest(input)?
        {
            return Err(ResearchError::Stale);
        }
        let mut run = Self::new(input, ledger.policy)?;
        for record in ledger.records {
            run.accept(record)?;
        }
        Ok(run)
    }

    #[must_use]
    pub fn ledger(&self) -> &ResearchLedger {
        &self.ledger
    }

    /// Propose old local notes for author revalidation, never accept them here.
    /// Exact whole-change equality prevents reuse after neighboring edits or
    /// coordinate shifts. Global synthesis must be regenerated for the new input.
    pub fn reuse_candidate(
        &self,
        previous_input: &ReviewInput,
        previous: ResearchLedger,
    ) -> Result<Option<ResearchNotes>, ResearchError> {
        let validated = ResearchRun::resume(previous_input, previous.clone())?;
        if validated.ledger.policy != self.ledger.policy {
            return Ok(None);
        }
        let Some(task) = self.next()? else {
            return Ok(None);
        };
        let change = &self.input.changes[self.change];
        if !previous_input.changes.contains(change) {
            return Ok(None);
        }
        let mut replay = ResearchRun::new(previous_input, previous.policy)?;
        for record in previous.records {
            let old_task = replay.next()?.ok_or(ResearchError::Complete)?;
            if old_task.page.quote == task.page.quote
                && old_task.page.offset == task.page.offset
                && old_task.page.text == task.page.text
            {
                let mut candidate = record;
                candidate.source = task.page.source.clone();
                candidate.request_id = task.id.clone();
                candidate.validate(&task, self.checkpoint_byte_limit())?;
                return Ok(Some(candidate));
            }
            replay.accept(record)?;
        }
        Ok(None)
    }

    #[must_use]
    pub fn checkpoint_byte_limit(&self) -> usize {
        self.ledger.policy.checkpoint_bytes
    }

    pub fn next(&self) -> Result<Option<ResearchTask>, ResearchError> {
        let Some(change) = self.input.changes.get(self.change) else {
            return Ok(None);
        };
        budget(
            "accepted batches",
            self.ledger.records.len() + 1,
            self.ledger.policy.max_batches,
        )?;
        let quote = Quote::whole(&change.id, change.body_lines());
        let page = self.reader.source(&quote, self.cursor.as_ref())?;
        let instructions = format!(
            "{INSTRUCTIONS}\nCheckpoint byte budget: {}.",
            self.ledger.policy.checkpoint_bytes
        );
        let id = digest(&(
            &self.ledger.manifest_sha256,
            &self.ledger.policy,
            &instructions,
            &page,
        ))?;
        let task = ResearchTask {
            schema_version: 1,
            id,
            instructions,
            page,
        };
        budget(
            "request bytes",
            serde_json::to_vec(&task)?.len(),
            self.ledger.policy.request_bytes,
        )?;
        Ok(Some(task))
    }

    pub fn accept(&mut self, notes: ResearchNotes) -> Result<(), ResearchError> {
        let task = self.next()?.ok_or(ResearchError::Complete)?;
        notes.validate(&task, self.ledger.policy.checkpoint_bytes)?;
        let exported = super::export::export_findings(&notes, &task.page);
        for finding in &exported {
            super::export::check_retrievable(finding, &self.ledger)?;
        }
        self.exported.extend(exported);
        self.cursor = task.page.next;
        if self.cursor.is_none() {
            self.change += 1;
        }
        self.ledger.records.push(notes);
        Ok(())
    }

    /// Export attribution for the legacy overview checkpoint only after full
    /// byte accounting. Semantic synthesis is still required from the author.
    pub fn findings(&self) -> Result<&[InvestigationFinding], ResearchError> {
        if self.change != self.input.changes.len() {
            return Err(ResearchError::Incomplete);
        }
        Ok(&self.exported)
    }

    pub(super) fn completed_hash(&self) -> Result<&str, ResearchError> {
        self.findings()?;
        if self.completed_hash.get().is_none() {
            let hash = digest(self.ledger())?;
            let _ = self.completed_hash.set(hash);
        }
        Ok(self.completed_hash.get().expect("initialized hash"))
    }
}
