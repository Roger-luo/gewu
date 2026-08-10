use gewu_review::{
    FeedbackTarget, Handover, ReviewChunk, ReviewSession, ReviewSpec, harness::WorkItem,
};

const INVESTIGATE: &str = include_str!("../../../../harness/prompts/investigate.md");
const PLAN: &str = include_str!("../../../../harness/prompts/plan.md");
const EXPAND: &str = include_str!("../../../../harness/prompts/expand.md");
const AWAITING: &str = include_str!("../../../../harness/prompts/awaiting.md");
const REPLAN: &str = include_str!("../../../../harness/prompts/replan.md");
/// The self-check both writing stages face. Expanding and rewriting produce the
/// same artifact and fail the reviewer in the same ways, so they are held to one
/// list rather than two that drift apart.
const RUBRIC: &str = include_str!("../../../../harness/prompts/rubric.md");
const HANDOVER: &str = include_str!("../../../../harness/prompts/handover.md");
const REVISE: &str = include_str!("../../../../harness/prompts/revise.md");
const COMPLETE: &str = include_str!("../../../../harness/prompts/complete.md");

pub fn investigate() -> String {
    INVESTIGATE.to_owned()
}

pub fn plan(max_chunk_lines: usize, max_chunks: usize) -> String {
    PLAN.replace("{{max_chunk_lines}}", &max_chunk_lines.to_string())
        .replace("{{max_chunks}}", &max_chunks.to_string())
}

pub fn expand(
    item: &WorkItem,
    input: &gewu_review::ReviewInput,
    max_chunk_lines: usize,
    max_chunks: usize,
) -> String {
    EXPAND
        .replace("{{item_id}}", &item.id)
        .replace("{{item_title}}", &item.title)
        .replace("{{item_purpose}}", &item.purpose)
        .replace("{{covers}}", &spans(&item.covers, input))
        .replace("{{max_chunk_lines}}", &max_chunk_lines.to_string())
        .replace("{{max_chunks}}", &max_chunks.to_string())
        .replace("{{rubric}}", RUBRIC.trim_end())
}

/// Printed after an expansion lands: the human reviews before anything more is
/// generated, which is the point of expanding one item at a time.
pub fn awaiting(spec: &ReviewSpec, remaining: usize) -> String {
    AWAITING
        .replace("{{review_id}}", &spec.session_key())
        .replace("{{chunks}}", &spec.chunks.len().to_string())
        .replace("{{remaining}}", &remaining.to_string())
}

pub fn waiting(unreviewed: usize) -> String {
    format!(
        "# Gewu harness: waiting for the reviewer\n\n{unreviewed} chunk(s) are still unreviewed. \
         The reviewer decides when to continue.\n"
    )
}

/// The reviewer's requests about the code, as instructions to act on.
pub fn handover(handover: &Handover<'_>, feedback_ids: &[String]) -> String {
    let requests = handover
        .requests_for(FeedbackTarget::Code)
        .filter(|item| feedback_ids.iter().any(|id| id == item.id))
        .map(|item| {
            let mut block = format!(
                "## `{}`\n\nOn chunk `{}` — {}\n",
                item.id,
                item.chunk_id,
                item.chunk_title
                    .unwrap_or("a chunk no longer in the review")
            );
            if let Some(location) = &item.location {
                block.push_str(&format!(
                    "\n`{}:{}-{}`{}\n",
                    location.path,
                    location.start_line,
                    location.end_line,
                    if location.stale {
                        " — the hunk has changed since this was written"
                    } else {
                        ""
                    }
                ));
                if !location.stale && !location.lines.is_empty() {
                    block.push_str(&format!("\n```diff\n{}\n```\n", location.lines.join("\n")));
                }
            }
            format!("{block}\n{}\n", quoted(item.body))
        })
        .collect::<Vec<_>>()
        .join("\n");
    HANDOVER
        .replace("{{count}}", &feedback_ids.len().to_string())
        .replace("{{requests}}", &requests)
}

/// The chunks to rewrite, alongside what the reviewer said about each.
pub fn revise(chunks: &[&ReviewChunk], session: &ReviewSession, max_chunk_lines: usize) -> String {
    let requests = chunks
        .iter()
        .map(|chunk| {
            let said = session
                .chunk_feedback(&chunk.id)
                .into_iter()
                .filter(|feedback| {
                    !feedback.resolved && feedback.target == FeedbackTarget::Explanation
                })
                .map(|feedback| format!("`{}`:\n\n{}", feedback.id, quoted(&feedback.body)))
                .collect::<Vec<_>>()
                .join("\n\n");
            format!("## `{}` — {}\n\n{said}\n", chunk.id, chunk.title)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let current = chunks
        .iter()
        .map(|chunk| {
            let json = serde_json::to_string_pretty(chunk)
                .unwrap_or_else(|_| format!("could not render `{}`", chunk.id));
            format!("```json\n{json}\n```")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    REVISE
        .replace("{{count}}", &chunks.len().to_string())
        .replace("{{requests}}", &requests)
        .replace("{{chunks}}", &current)
        .replace("{{max_chunk_lines}}", &max_chunk_lines.to_string())
        .replace("{{rubric}}", RUBRIC.trim_end())
}

pub fn revised(chunk_ids: &[String]) -> String {
    format!(
        "# Gewu harness: rewrite accepted\n\nThe review now shows the rewritten chunk(s):\n\n{}\n\n\
         Resolve the request(s) they answer, if you have not already:\n\n```bash\ngewu review \
         resolve --id <feedback-id>\n```\n\nThen run `gewu harness next`.\n",
        bullets(chunk_ids)
    )
}

/// A reviewer's own words, kept as theirs.
fn quoted(body: &str) -> String {
    body.lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn replan(stale_chunks: &[String], remainder: usize) -> String {
    let stale = if stale_chunks.is_empty() {
        "No expanded chunk was invalidated.".to_owned()
    } else {
        format!(
            "These chunks quoted code that has changed and were removed:\n\n{}",
            bullets(stale_chunks)
        )
    };
    REPLAN
        .replace("{{stale}}", &stale)
        .replace("{{remainder}}", &remainder.to_string())
}

pub fn retry(stage_prompt: &str, errors: &[String]) -> String {
    format!(
        "# Gewu harness: checkpoint needs correction\n\n{}\n\n{}",
        bullets(errors),
        stage_prompt
    )
}

/// What each chunk costs, so a rewrite can see where the rows went instead of
/// guessing which sentence to cut.
pub fn measured(chunks: &[gewu_review::ReviewChunk], input: &gewu_review::ReviewInput) -> String {
    let known: std::collections::HashMap<_, _> = input
        .changes
        .iter()
        .map(|change| (&change.id, change))
        .collect();
    let rows = chunks
        .iter()
        .map(|chunk| {
            let height = gewu_review::chunk_height(chunk, &known);
            // The pane is reported, not charged: it is context beside the work,
            // and an agent that saw only one number would cut the reminder a
            // reviewer asked for to buy rows it never needed.
            let pane = gewu_review::pane_height(chunk);
            let beside = if pane > 0 {
                format!(", beside a `context` pane of about {pane} more")
            } else {
                String::new()
            };
            format!(
                "- `{}` — about {height} of {} rows{beside}",
                chunk.id, input.max_chunk_lines
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Measured heights:\n\n{rows}\n")
}

pub fn complete(review_id: &str) -> String {
    COMPLETE.replace("{{review_id}}", review_id)
}

/// The spans an item owns, as the agent must name them back.
fn spans(covers: &[gewu_review::Quote], input: &gewu_review::ReviewInput) -> String {
    covers
        .iter()
        .map(|quote| {
            let path = input
                .changes
                .iter()
                .find(|change| change.id == quote.change_id)
                .map_or("?", |change| change.path.as_str());
            format!(
                "- `{}` — {} lines {}-{}",
                quote.change_id, path, quote.start, quote.end
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn bullets(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}
