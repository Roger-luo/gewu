use gewu_review::{
    FeedbackTarget, Handover, ReviewChunk, ReviewSession, ReviewSpec,
    harness::{InvestigationCheckpoint, WorkItem},
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
        .replace("{{height_guidance}}", gewu_review::CHUNK_HEIGHT_GUIDANCE)
}

pub fn expand(
    item: &WorkItem,
    input: &gewu_review::ReviewInput,
    investigation: Option<&InvestigationCheckpoint>,
    max_chunk_lines: usize,
    max_chunks: usize,
) -> String {
    EXPAND
        .replace("{{item_id}}", &item.id)
        .replace("{{item_title}}", &item.title)
        .replace("{{item_purpose}}", &item.purpose)
        .replace("{{covers}}", &spans(&item.covers, input))
        .replace(
            "{{investigation}}",
            &investigation_evidence(item, investigation),
        )
        .replace("{{max_chunk_lines}}", &max_chunk_lines.to_string())
        .replace("{{max_chunks}}", &max_chunks.to_string())
        .replace("{{height_guidance}}", gewu_review::CHUNK_HEIGHT_GUIDANCE)
        .replace("{{rubric}}", RUBRIC.trim_end())
}

fn investigation_evidence(
    item: &WorkItem,
    investigation: Option<&InvestigationCheckpoint>,
) -> String {
    let Some(investigation) = investigation else {
        return "No investigation checkpoint is available; derive claims from the frozen source."
            .to_owned();
    };
    let owned: std::collections::HashSet<_> = item.change_ids().collect();
    let findings = investigation
        .findings
        .iter()
        .filter(|finding| finding.change_ids.iter().any(|id| owned.contains(id)))
        .map(|finding| {
            let evidence = if finding.evidence.is_empty() {
                String::new()
            } else {
                format!(" — evidence: {}", finding.evidence.join(", "))
            };
            format!("- {}{evidence}", finding.summary)
        })
        .collect::<Vec<_>>();
    if findings.is_empty() {
        "No investigation finding matched this item's change ids; re-check the frozen source."
            .to_owned()
    } else {
        findings.join("\n")
    }
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

/// A dogfood author mistyped one change id and got two useful diagnostics
/// followed by the entire investigation prompt. Schema checkpoints already
/// exist on disk, so keep the correction in the foreground; `status` remains
/// the explicit way to recover the full instructions.
pub fn checkpoint_retry(path: &str, errors: &[String]) -> String {
    format!(
        "# Gewu harness: checkpoint needs correction\n\n{}\n\nCorrect `{path}`, then run \
         `gewu harness next`. Run `gewu harness status` if you need the stage instructions \
         again.\n",
        bullets(errors)
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

#[cfg(test)]
mod tests {
    use gewu_review::{
        Change, Quote, ReviewInput, SourceIdentity,
        harness::{InvestigationCheckpoint, InvestigationFinding, WorkItem},
    };

    use super::{checkpoint_retry, expand, investigate, plan};

    #[test]
    fn author_contract_explains_driver_continuation_and_height_accounting() {
        let investigation = investigate();
        let plan = plan(40, 6);

        assert!(investigation.contains("driver writes it, validates"));
        assert!(investigation.contains("whole evidence base"));
        assert!(investigation.contains("direct CLI session"));
        assert!(investigation.contains("read-only repository inspection"));
        assert!(investigation.contains("trace the exact characters"));
        assert!(investigation.contains("serialized JSON string"));
        assert!(plan.contains("Start with 8 rows of chunk chrome"));
        assert!(plan.contains("mechanically repeated configuration together"));
        assert!(plan.contains("40 × 6 non-blank source rows"));
        assert!(plan.contains("A one-change item is"));
        assert!(plan.contains("exempt so an intentionally summarized"));
        assert!(!plan.contains("{{height_guidance}}"));
    }

    /// A generated overview once erased an internal dependency while its
    /// detailed chunks correctly described it. Keep aggregate claims tied to
    /// the evidence for every member rather than letting the two stages drift.
    #[test]
    fn prompts_reconcile_aggregate_claims_with_component_evidence() {
        let investigation = investigate();
        let input = ReviewInput::new(SourceIdentity::new("base", "head", b"diff"), vec![]);
        let item = WorkItem {
            id: "item".into(),
            title: "Item".into(),
            purpose: "Purpose".into(),
            covers: Vec::<Quote>::new(),
            expanded: false,
        };
        let expansion = expand(&item, &input, None, 40, 6);

        assert!(investigation.contains("check every member"));
        assert!(investigation.contains("internal relationships"));
        assert!(expansion.contains("aggregate claim"));
        assert!(expansion.contains("opening generalization"));
    }

    #[test]
    fn expansion_carries_forward_only_the_items_investigation_evidence() {
        let source = SourceIdentity::new("base", "head", b"diff");
        let change = Change::new("src/lib.rs", (0, 0, 1, 1), "@@ -0,0 +1 @@\n+line\n");
        let input = ReviewInput::new(source.clone(), vec![change.clone()]);
        let item = WorkItem {
            id: "item".into(),
            title: "Item".into(),
            purpose: "Purpose".into(),
            covers: vec![Quote::whole(change.id.clone(), change.body_lines())],
            expanded: false,
        };
        let investigation = InvestigationCheckpoint {
            schema_version: 1,
            source,
            intent: "Intent".into(),
            changes: vec!["Change".into()],
            concepts: vec![],
            findings: vec![
                InvestigationFinding {
                    change_ids: vec![change.id],
                    summary: "The exact source-backed qualification.".into(),
                    evidence: vec!["src/lib.rs".into()],
                },
                InvestigationFinding {
                    change_ids: vec!["another-change".into()],
                    summary: "Unrelated work must not leak in.".into(),
                    evidence: vec![],
                },
            ],
        };

        let prompt = expand(&item, &input, Some(&investigation), 40, 6);

        assert!(prompt.contains("The exact source-backed qualification."));
        assert!(prompt.contains("evidence: src/lib.rs"));
        assert!(!prompt.contains("Unrelated work must not leak in."));
    }

    #[test]
    fn expansion_schema_names_optional_fields_and_markdown_limit() {
        let input = ReviewInput::new(SourceIdentity::new("base", "head", b"diff"), vec![]);
        let item = WorkItem {
            id: "item".into(),
            title: "Item".into(),
            purpose: "Purpose".into(),
            covers: Vec::<Quote>::new(),
            expanded: false,
        };

        let prompt = expand(&item, &input, None, 40, 6);

        assert!(prompt.contains("\"context\""));
        assert!(prompt.contains("\"confidence\""));
        assert!(prompt.contains("\"oversized_reason\""));
        assert!(prompt.contains("only for paths ending in `.md`"));
        assert!(prompt.contains("Measurement is still the authority"));
        assert!(prompt.contains("local hook does not prove central enforcement"));
        assert!(prompt.contains("no options are"));
        assert!(prompt.contains("contradict the earlier investigation overview"));
        assert!(prompt.contains("unchanged boilerplate reviewed as one coherent unit"));
        assert!(prompt.contains("every field of its selected shape is required"));
        assert!(!prompt.contains("optional supporting definition"));
        assert!(prompt.contains("trace the literal characters across"));
        assert!(prompt.contains("serialized JSON"));
        assert!(prompt.contains("derive claims from the frozen source"));
    }

    #[test]
    fn checkpoint_retry_keeps_the_error_above_recoverable_instructions() {
        let retry = checkpoint_retry(
            ".gewu/harness/investigation.json",
            &["unknown change id `short-id`".into()],
        );

        assert!(retry.contains("unknown change id `short-id`"));
        assert!(retry.contains("Correct `.gewu/harness/investigation.json`"));
        assert!(retry.contains("gewu harness status"));
        assert!(!retry.contains("Inspect every change"));
    }
}
