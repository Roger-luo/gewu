use std::{fs, path::Path};

use gewu_review::{
    ReviewInput,
    harness::{HarnessStage, HarnessState, InvestigationCheckpoint},
    research::{GroundedOverview, ResearchLedger, ResearchRun},
};

use super::artifacts;
use crate::{Error, Result};

pub fn prompt() -> String {
    gewu_prompts::Prompt::Research.text().into_owned()
}

pub fn validate_checkpoint(
    root: &Path,
    input: &ReviewInput,
    checkpoint: &InvestigationCheckpoint,
    required: bool,
) -> Result<()> {
    let path = root.join("research.json");
    if !required && !path.exists() {
        return Ok(());
    }
    let ledger: ResearchLedger = artifacts::read(path)?;
    let run = ResearchRun::resume(input, ledger)?;
    let overview: GroundedOverview = artifacts::read(root.join("research-overview.json"))?;
    if run.assemble(&overview)? != *checkpoint {
        return Err(Error::message(
            "investigation differs from accepted research and grounded overview",
        ));
    }
    Ok(())
}

pub fn require_investigate(root: &Path, input: &ReviewInput) -> Result<()> {
    let path = root.join("state.json");
    if path.exists() {
        let state: HarnessState = artifacts::read(path)?;
        if state.stage != HarnessStage::Investigate || state.source != input.source {
            return Err(Error::message(
                "research can only write to the current harness investigation stage",
            ));
        }
    }
    Ok(())
}

pub fn attach(root: &Path) -> Result<()> {
    let path = root.join("state.json");
    if path.exists() {
        let mut state: HarnessState = artifacts::read(&path)?;
        state.research_required = true;
        artifacts::write(&path, &state)?;
    }
    Ok(())
}

/// Retain evidence from the old source while making room for a new ledger.
/// The harness's research-required flag deliberately survives this operation.
pub fn archive(root: &Path, revision: u32, input: &ReviewInput) -> Result<()> {
    let destination = root.join("research-archive").join(revision.to_string());
    if root.join("research.json").exists() {
        let snapshot = destination.join("review-input.json");
        if snapshot.exists() {
            let existing: ReviewInput = artifacts::read(&snapshot)?;
            if existing != *input {
                return Err(Error::message(
                    "research archive source conflicts with snapshot",
                ));
            }
        } else {
            artifacts::write(snapshot, input)?;
        }
    }
    for name in ["research.json", "research-overview.json"] {
        let source = root.join(name);
        if source.exists() {
            fs::create_dir_all(&destination)?;
            let target = destination.join(name);
            if target.exists() {
                if fs::read(&target)? != fs::read(&source)? {
                    return Err(Error::message(
                        "research archive conflicts with existing evidence",
                    ));
                }
                fs::remove_file(&source)?;
            } else {
                fs::rename(&source, &target)?;
            }
        }
    }
    Ok(())
}
