use std::fmt::Write as _;

use anyhow::Result;
use clap::{Args, ValueEnum};
use gewu_review::{
    FeedbackTarget, Handover,
    handover::{HandoverItem, Location},
};

use super::SessionArgs;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum Format {
    /// Prose an agent can act on directly.
    #[default]
    Markdown,
    /// The same content, for a tool that wants fields rather than sentences.
    Json,
}

#[derive(Debug, Args)]
pub struct HandoverArgs {
    #[command(flatten)]
    session: SessionArgs,
    #[arg(long, value_enum, default_value_t = Format::Markdown)]
    format: Format,
}

impl HandoverArgs {
    pub(super) fn run(self) -> Result<()> {
        let (review, _, session) = self.session.load()?;
        let handover = Handover::assemble(&review, &session);
        match self.format {
            Format::Json => println!("{}", serde_json::to_string_pretty(&handover)?),
            Format::Markdown => print!("{}", markdown(&handover)),
        }
        Ok(())
    }
}

/// The review as something to act on: what was asked, where, and what closes
/// it. The event log is the record and keeps withdrawn feedback and bare chunk
/// ids; nobody should have to read that to learn what a reviewer wants.
fn markdown(handover: &Handover<'_>) -> String {
    let mut out = format!("# Review handover — `{}`\n\n", handover.review_id);
    let _ = writeln!(
        out,
        "{} of {} chunk(s) reviewed. {} request(s) and {} comment(s) are open; {} already \
         resolved.\n",
        handover.chunks_reviewed,
        handover.chunks_total,
        handover.requests.len(),
        handover.comments.len(),
        handover.resolved
    );
    if let Some(decision) = handover.decision {
        let _ = writeln!(out, "Decision: **{}**.\n", decision.label());
    }
    if handover.is_empty() {
        out.push_str("The reviewer has left nothing open.\n");
        return out;
    }

    section(
        &mut out,
        "Requested changes to the code",
        "Edit the source, then resolve each request by id.",
        handover.requests_for(FeedbackTarget::Code),
    );
    section(
        &mut out,
        "Requested changes to the walkthrough",
        "The code is not at fault here; the explanation of it is. Run `gewu harness next` to be \
         given these chunks to rewrite.",
        handover.requests_for(FeedbackTarget::Explanation),
    );
    section(
        &mut out,
        "Comments",
        "Non-blocking. The reviewer's thinking, not a request.",
        handover.comments.iter(),
    );

    out.push_str("## Closing the loop\n\n");
    out.push_str(
        "Resolve a request only once it is addressed — it is the reviewer's gate, and the harness \
         will not go on while any are open:\n\n```bash\ngewu review resolve --id <feedback-id>\n\
         ```\n\nIf a request is aimed at the wrong thing, say so rather than working around it:\n\n\
         ```bash\ngewu review retarget --id <feedback-id> --target code|explanation\n```\n",
    );
    out
}

fn section<'a>(
    out: &mut String,
    title: &str,
    guidance: &str,
    items: impl Iterator<Item = &'a HandoverItem<'a>>,
) {
    let mut items = items.peekable();
    if items.peek().is_none() {
        return;
    }
    let _ = writeln!(out, "## {title}\n\n{guidance}\n");
    for item in items {
        let _ = writeln!(
            out,
            "### `{}`\n\nChunk `{}` — {}",
            item.id,
            item.chunk_id,
            item.chunk_title
                .unwrap_or("no longer in the review; re-planning dropped it")
        );
        if let Some(summary) = item.chunk_summary {
            let _ = writeln!(out, "\n> {summary}");
        }
        if let Some(location) = &item.location {
            write_location(out, location);
        }
        let quoted = item
            .body
            .lines()
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = writeln!(out, "\nThe reviewer wrote:\n\n{quoted}\n");
    }
}

fn write_location(out: &mut String, location: &Location<'_>) {
    let _ = writeln!(
        out,
        "\n`{}:{}-{}` ({} side)",
        location.path, location.start_line, location.end_line, location.side
    );
    if location.stale {
        out.push_str(
            "\n**The hunk has changed since this was written**, so the lines below are not the \
             ones the reviewer saw.\n",
        );
    }
    if !location.lines.is_empty() {
        let _ = writeln!(out, "\n```diff\n{}\n```", location.lines.join("\n"));
    }
}
