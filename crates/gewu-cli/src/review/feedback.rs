//! Acting on feedback without a terminal.
//!
//! A reviewer resolves feedback on the screen where they wrote it. An agent
//! cannot, and an unresolved request holds the harness shut, so the two actions
//! a reviewer takes on their own feedback — declaring it addressed, and saying
//! what it was really about — are also commands.

use anyhow::{Result, bail};
use clap::{Args, ValueEnum};
use gewu_review::{Feedback, FeedbackTarget, ReviewEvent, ReviewSession};

use super::SessionArgs;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Target {
    /// The code under review.
    Code,
    /// The walkthrough explaining it.
    Explanation,
}

impl From<Target> for FeedbackTarget {
    fn from(target: Target) -> Self {
        match target {
            Target::Code => Self::Code,
            Target::Explanation => Self::Explanation,
        }
    }
}

#[derive(Debug, Args)]
pub struct ResolveArgs {
    #[command(flatten)]
    session: SessionArgs,
    /// Feedback to mark addressed. Repeat for more than one.
    #[arg(long = "id", required_unless_present = "all")]
    ids: Vec<String>,
    /// Resolve every open request. Say what was done; do not use this to clear
    /// a gate you have not satisfied.
    #[arg(long, conflicts_with = "ids")]
    all: bool,
    /// Undo a resolution, putting the request back in front of whoever is next.
    #[arg(long)]
    reopen: bool,
}

impl ResolveArgs {
    pub(super) fn run(self) -> Result<()> {
        let (_, log, session) = self.session.load()?;
        let resolved = !self.reopen;
        let ids = if self.all {
            session
                .unresolved_issues()
                .map(|feedback| feedback.id.clone())
                .collect()
        } else {
            self.ids.clone()
        };
        for id in &ids {
            let feedback = known(&session, id)?;
            if feedback.resolved == resolved {
                println!("{id} is already {}", state(resolved));
                continue;
            }
            log.append(&ReviewEvent::resolved(id, resolved))?;
            println!("{id} {}", state(resolved));
        }
        if ids.is_empty() {
            println!("nothing to resolve");
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct RetargetArgs {
    #[command(flatten)]
    session: SessionArgs,
    /// Feedback to re-aim. Repeat for more than one.
    #[arg(long = "id", required = true)]
    ids: Vec<String>,
    /// What the feedback is really about.
    #[arg(long, value_enum)]
    target: Target,
}

impl RetargetArgs {
    pub(super) fn run(self) -> Result<()> {
        let (_, log, session) = self.session.load()?;
        let target = FeedbackTarget::from(self.target);
        for id in &self.ids {
            let feedback = known(&session, id)?;
            if feedback.target == target {
                println!("{id} is already about the {}", target.label());
                continue;
            }
            log.append(&ReviewEvent::retargeted(id, target))?;
            println!("{id} is about the {}", target.label());
        }
        Ok(())
    }
}

/// Feedback the review actually holds. Acting on an id that is not there would
/// otherwise append an event nothing replays, and silently change nothing.
fn known<'a>(session: &'a ReviewSession, id: &str) -> Result<&'a Feedback> {
    match session.feedback.get(id) {
        Some(feedback) => Ok(feedback),
        None => bail!("this review has no feedback `{id}`"),
    }
}

fn state(resolved: bool) -> &'static str {
    if resolved { "resolved" } else { "reopened" }
}
