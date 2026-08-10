use std::collections::{HashMap, HashSet};

use gewu_review::{
    Change, ChangePresentation, ChunkBlock, ChunkContext, Decision, Feedback, FeedbackTarget,
    Quote, ReviewChunk, ReviewSession, ValidatedReview,
};

use super::{
    highlight::CodeHighlighter,
    markdown::Markdown,
    render_support::{
        SOURCE_GUTTER, bullets, document_title, heading, illustration, muted, presentation_label,
        source_path_style, title_style,
    },
    style::{Document, Line, TextStyle},
};

pub struct Screen;

impl Screen {
    #[must_use]
    pub fn overview(review: &ValidatedReview) -> Document {
        let overview = &review.spec().overview;
        let mut document = document_title("PR OVERVIEW");
        document.append(Markdown::render(&overview.intent));
        heading(&mut document, "What changes");
        document.append(Markdown::render(&bullets(&overview.changes)));
        if !overview.concepts.is_empty() {
            heading(&mut document, "New concepts");
            document.append(Markdown::render(&bullets(&overview.concepts)));
        }
        if let Some(value) = &overview.illustration {
            heading(&mut document, "Illustration");
            document.append(Markdown::render(&illustration(value)));
        }
        document.push(scope(review), muted());
        document
    }

    #[must_use]
    pub fn chunk(
        review: &ValidatedReview,
        session: &ReviewSession,
        index: usize,
        width: usize,
        selected: Option<&str>,
    ) -> Document {
        let chunk = &review.spec().chunks[index];
        let status = if session.reviewed.contains(&chunk.id) {
            "reviewed"
        } else {
            "unreviewed"
        };
        let mut document = Document::new();
        document.push(
            format!(
                "CHUNK {} OF {} · {status}",
                index + 1,
                review.spec().chunks.len()
            ),
            title_style(),
        );
        document.newline();
        document.newline();
        let main = Self::chunk_main(review, session, chunk, selected);
        match chunk.context.as_ref().map(Self::context) {
            Some(context) if width >= 100 => {
                document.append(Document::columns(main, context, width));
            }
            Some(context) => {
                document.append(main);
                heading(&mut document, "Context");
                document.append(context);
            }
            None => document.append(main),
        }
        document
    }

    #[must_use]
    pub fn summary(review: &ValidatedReview, session: &ReviewSession) -> Document {
        // Counted over the chunks the review still holds: re-planning can drop
        // a reviewed chunk, and progress must never read as more than the whole.
        let reviewed = review
            .spec()
            .chunks
            .iter()
            .filter(|chunk| session.reviewed.contains(&chunk.id))
            .count();
        let unreviewed = review.spec().chunks.len().saturating_sub(reviewed);
        let open = session
            .live_unresolved_issues(review.spec().chunks.iter().map(|chunk| &chunk.id))
            .len();
        let outcome = session.outcome(review.spec().chunks.iter().map(|chunk| &chunk.id));
        let mut document = document_title("REVIEW SUMMARY");
        document.append(Markdown::render(&format!(
            "- **Reviewed:** {reviewed} / {}\n- **Unreviewed:** {unreviewed}\n\
             - **Open requests:** {open}",
            review.spec().chunks.len(),
        )));
        heading(&mut document, "Outcome");
        // Named before it is recorded, and with the reason, because the
        // reviewer never chose it: it follows from what they wrote, and they
        // are owed the chance to see that Gewu read them correctly.
        document.append(Markdown::render(&format!(
            "**{}** — {}\n\n{}",
            outcome.label(),
            outcome_reason(outcome, open),
            match session.decision {
                Some(recorded) if recorded == outcome =>
                    "Recorded. Press `Enter` to leave the review.".to_owned(),
                Some(recorded) => format!(
                    "Recorded earlier as *{}*; press `Enter` to record it as it now stands.",
                    recorded.label()
                ),
                None => "Press `Enter` to end the review with it.".to_owned(),
            }
        )));
        document
    }

    fn chunk_main(
        review: &ValidatedReview,
        session: &ReviewSession,
        chunk: &ReviewChunk,
        selected: Option<&str>,
    ) -> Document {
        let by_id: HashMap<_, _> = review
            .input()
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();
        // One gutter width for the whole chunk, so the code edge does not step
        // between one view and the next.
        let width = chunk
            .quotes()
            .filter_map(|quote| {
                let change = by_id.get(&quote.change_id)?;
                diff_line_numbers(&change.patch)
                    .get(quote.start..=quote.end.min(change.body_lines()))
                    .and_then(|numbers| numbers.iter().flatten().max().copied())
            })
            .max()
            .map_or(1, |line| line.to_string().len());
        let mut document = document_title(&chunk.title);
        heading(&mut document, "What this does");
        document.append(Markdown::render(&chunk.summary));
        if let Some(confidence) = &chunk.confidence {
            document.append(Markdown::render(&format!("*Confidence:* {confidence}")));
        }
        for block in &chunk.content {
            match block {
                ChunkBlock::Narrative { markdown } => {
                    document.append(Markdown::render(markdown));
                }
                ChunkBlock::Changes {
                    quotes,
                    presentation,
                } => {
                    let views: Vec<_> = quotes
                        .iter()
                        .filter_map(|quote| {
                            by_id.get(&quote.change_id).map(|change| (quote, *change))
                        })
                        .collect();
                    for run in views.chunk_by(|left, right| left.1.path == right.1.path) {
                        document.append(Self::source_block(run, *presentation, width));
                    }
                    // Prose that follows a view needs the same air as the
                    // rendered Markdown above it already leaves.
                    document.newline();
                }
            }
        }
        let feedback = session.chunk_feedback(&chunk.id);
        if !feedback.is_empty() {
            heading(&mut document, "Feedback");
            for item in feedback {
                document.append(Self::feedback_item(item, selected == Some(&item.id)));
            }
        }
        document
    }

    /// One piece of feedback, marked when the reviewer has it selected so the
    /// actions offered below plainly belong to it.
    fn feedback_item(item: &Feedback, selected: bool) -> Document {
        let mut document = Document::new();
        let style = if selected { title_style() } else { muted() };
        document.push(if selected { "▸ " } else { "  " }, style);
        document.push(item.kind.label(), style);
        // Only the unusual aim is named. Feedback is about the code unless the
        // reviewer said otherwise, and labelling every item "code" would price
        // the common case to pay for the rare one.
        if item.target == FeedbackTarget::Explanation {
            document.push(" · about this explanation", style);
        }
        document.push(
            if item.resolved {
                " · resolved"
            } else {
                " · open"
            },
            style,
        );
        if let Some(anchor) = &item.anchor {
            document.push(format!(" · {}:{}", anchor.path, anchor.start_line), muted());
        }
        document.newline();
        // A pasted body carries its own line breaks, so it is drawn line by
        // line rather than as one run that wrapping would reflow.
        for line in item.body.lines() {
            document.push("    ", muted());
            document.push(line, TextStyle::default());
            document.newline();
        }
        document
    }

    /// Frames one run of same-file changes: a labelled source header above the
    /// quoted material, every line of it behind a gutter.
    fn source_block(
        views: &[(&Quote, &Change)],
        presentation: ChangePresentation,
        width: usize,
    ) -> Document {
        let mut document = Document::new();
        document.push(SOURCE_GUTTER, muted());
        document.push(&views[0].1.path, source_path_style());
        document.push(format!(" · {}", presentation_label(presentation)), muted());
        if views.len() > 1 {
            document.push(format!(" · {} views", views.len()), muted());
        }
        document.newline();
        document.push(SOURCE_GUTTER, muted());
        document.newline();
        let mut body = Document::new();
        for (quote, change) in views {
            if body.lines.last().is_some_and(|line| line.width() > 0) {
                body.newline();
                // Two views of the same file are not contiguous code; say so.
                body.newline();
            }
            body.append(Self::change_body(quote, change, presentation, width));
        }
        document.append(body.trim_trailing().gutter(SOURCE_GUTTER, muted()));
        document.newline();
        document
    }

    fn change_body(
        quote: &Quote,
        change: &Change,
        presentation: ChangePresentation,
        width: usize,
    ) -> Document {
        // Line numbers are counted from the hunk's start, then the view is cut
        // out, so a quote of lines 40-60 is numbered as it sits in the file.
        let all = diff_line_numbers(&change.patch);
        let view = change.body(quote.start, quote.end).join("\n");
        // Numbers for this view only; a short view must not be indented for
        // line numbers it never prints.
        let numbers = all
            .get(quote.start..=quote.end.min(change.body_lines()))
            .unwrap_or(&[])
            .to_vec();
        match presentation {
            ChangePresentation::Diff => numbered(
                CodeHighlighter::global().diff(&change.path, &view),
                &numbers,
                width,
            ),
            ChangePresentation::AddedCode => numbered(
                CodeHighlighter::global().source(&change.path, &Self::added_text(&view)),
                &shown_line_numbers(&view, &numbers),
                width,
            ),
            ChangePresentation::RenderedMarkdown => Markdown::render(&Self::added_text(&view)),
            ChangePresentation::SummaryOnly => {
                let mut document = Document::new();
                document.push(
                    format!(
                        "{} lines withheld as a generated artifact",
                        change.new_lines
                    ),
                    TextStyle {
                        italic: true,
                        ..muted()
                    },
                );
                document
            }
        }
    }

    /// Strips the diff markers from a view. Metadata changes carry no markers
    /// at all, so they are shown as written rather than filtered to nothing.
    fn added_text(view: &str) -> String {
        if !view
            .lines()
            .any(|line| line.starts_with('+') || line.starts_with(' '))
        {
            return view.trim_end().to_owned();
        }
        view.lines()
            .filter_map(|line| line.strip_prefix('+').or_else(|| line.strip_prefix(' ')))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn context(context: &ChunkContext) -> Document {
        match context {
            ChunkContext::Concept {
                definition,
                example,
                relation,
            } => Markdown::render(&format!(
                "# New concept\n\n{definition}\n\n## Example\n\n{example}\n\n## Here\n\n{relation}"
            )),
            ChunkContext::Refactor {
                before,
                after,
                reason,
            } => Markdown::render(&format!(
                "# Refactor\n\n## Before\n\n{before}\n\n## After\n\n{after}\n\n## Why\n\n{reason}"
            )),
        }
    }
}

/// Why the review came out the way it did, in the reviewer's own actions. An
/// outcome nobody chose has to explain itself, or it reads as Gewu deciding.
fn outcome_reason(outcome: Decision, open: usize) -> &'static str {
    match outcome {
        Decision::RequestChanges if open == 1 => "one request is open",
        Decision::RequestChanges => "requests are open",
        Decision::Comment => "you left remarks, and none of them ask for a change",
        Decision::Approve => "you asked for nothing and remarked on nothing",
    }
}

/// States how much of the change set the review covers so far. Work is
/// expanded one item at a time, so a review is usually still growing.
fn scope(review: &ValidatedReview) -> String {
    let total = review.input().changes.len();
    let covered: HashSet<_> = review
        .spec()
        .chunks
        .iter()
        .flat_map(ReviewChunk::change_ids)
        .collect();
    let chunks = review.spec().chunks.len();
    if covered.len() >= total {
        return format!("Scope: {total} change hunks · {chunks} review chunks");
    }
    format!(
        "Scope: {} of {total} change hunks · {chunks} review chunks · more follow as you review",
        covered.len()
    )
}

/// New-side file line for each line the diff renderer will draw, so a reviewer
/// can name a line without counting.
fn diff_line_numbers(patch: &str) -> Vec<Option<u32>> {
    let mut numbers = Vec::new();
    let mut next = 0;
    for line in patch.lines() {
        if line.starts_with("@@ ") {
            next = hunk_start(line).unwrap_or(next);
            numbers.push(None);
            continue;
        }
        match line.chars().next() {
            // Removed lines exist only on the old side.
            Some('-') => numbers.push(None),
            Some('+' | ' ') => {
                numbers.push(Some(next));
                next = next.saturating_add(1);
            }
            // Metadata and "\ No newline" markers name no line.
            _ => numbers.push(None),
        }
    }
    numbers
}

/// New-side numbers for the lines `added_text` will actually draw, which skips
/// removals.
fn shown_line_numbers(view: &str, numbers: &[Option<u32>]) -> Vec<Option<u32>> {
    view.lines()
        .zip(numbers.iter().copied().chain(std::iter::repeat(None)))
        .filter(|(line, _)| !line.starts_with('-'))
        .map(|(_, number)| number)
        .collect()
}

fn hunk_start(header: &str) -> Option<u32> {
    header
        .split_whitespace()
        .nth(2)?
        .strip_prefix('+')?
        .split(',')
        .next()?
        .parse()
        .ok()
}

/// Prefixes each line with its file line number, right-aligned so the code
/// edge stays straight.
fn numbered(document: Document, numbers: &[Option<u32>], width: usize) -> Document {
    Document {
        lines: document
            .lines
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                let label = match numbers.get(index).copied().flatten() {
                    Some(number) => format!("{number:>width$} "),
                    None => " ".repeat(width + 1),
                };
                let mut numbered = Line::default();
                numbered.push(label, muted());
                numbered.spans.extend(line.spans);
                numbered
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use gewu_review::{
        Change, ChangePresentation, ChunkBlock, ChunkContext, ChunkKind, Feedback, FeedbackKind,
        FeedbackTarget, Overview, Quote, ReviewChunk, ReviewEvent, ReviewInput, ReviewSession,
        ReviewSpec, SourceIdentity, ValidatedReview,
    };

    use super::{super::style::Line, SOURCE_GUTTER, Screen};

    #[test]
    fn extracts_added_markdown_without_diff_markers() {
        let patch = "@@ -0,0 +1,3 @@\n+## Flow\n+\n+`input` → `output`\n";

        assert_eq!(Screen::added_text(patch), "## Flow\n\n`input` → `output`");
    }

    #[test]
    fn frames_every_quoted_line_behind_the_source_gutter() {
        let review = review(
            &[("docs/design.md", "# Title"), ("docs/design.md", "Body")],
            ChangePresentation::RenderedMarkdown,
        );

        let document = Screen::chunk(&review, &ReviewSession::default(), 0, 80, None);

        let quoted: Vec<_> = lines(&document)
            .into_iter()
            .skip_while(|line| !line.starts_with(SOURCE_GUTTER))
            .filter(|line| !line.is_empty())
            .collect();
        assert!(!quoted.is_empty());
        assert!(
            quoted.iter().all(|line| line.starts_with(SOURCE_GUTTER)),
            "{quoted:#?}"
        );
    }

    #[test]
    fn gives_each_file_one_header_naming_its_presentation() {
        let review = review(
            &[
                ("docs/design.md", "# One"),
                ("docs/design.md", "# Two"),
                ("README.md", "# Three"),
            ],
            ChangePresentation::RenderedMarkdown,
        );

        let document = Screen::chunk(&review, &ReviewSession::default(), 0, 80, None);

        let headers: Vec<_> = lines(&document)
            .into_iter()
            .filter(|line| line.contains(" · document"))
            .collect();
        assert_eq!(headers.len(), 2, "{headers:#?}");
        assert!(headers[0].starts_with("▏ docs/design.md · document · 2 views"));
        assert!(headers[1].starts_with("▏ README.md · document"));
    }

    #[test]
    fn states_what_a_withheld_artifact_contains() {
        let review = review(
            &[("Cargo.lock", "dependency")],
            ChangePresentation::SummaryOnly,
        );

        let document = Screen::chunk(&review, &ReviewSession::default(), 0, 80, None);

        assert!(
            lines(&document)
                .iter()
                .any(|line| line.contains("1 lines withheld as a generated artifact")),
            "{:#?}",
            lines(&document)
        );
    }

    /// A `context` pane is outside the review budget, because reading it is not
    /// reviewing. It is still drawn, though, and at this width below the chunk
    /// rather than beside it — so the two measures together must still predict
    /// the screen, or leaving the pane out of the budget would quietly make the
    /// estimate wrong instead of making panes affordable.
    #[test]
    fn the_budget_and_the_pane_together_still_predict_the_screen() {
        let source = SourceIdentity::new("base", "head", b"diff");
        let body: String = (1..=8)
            .map(|line| format!("+    let value_{line} = {line};\n"))
            .collect();
        let change = Change::new("a.rs", (0, 0, 1, 8), format!("@@ -0,0 +1,8 @@\n{body}"));
        let chunk = ReviewChunk {
            id: "chunk".into(),
            title: "A chunk".into(),
            summary: "One sentence of summary.".into(),
            kind: ChunkKind::Concept,
            content: vec![ChunkBlock::Changes {
                quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
                presentation: ChangePresentation::AddedCode,
            }],
            context: Some(ChunkContext::Concept {
                definition: "A `Quote` names a span of one change's body.".into(),
                example: "`Quote { change_id, start: 1, end: 24 }`.".into(),
                relation: "Every view in a chunk is made of them.".into(),
            }),
            confidence: None,
            oversized_reason: None,
        };
        let input = ReviewInput::new(source.clone(), vec![change.clone()]);
        let spec = ReviewSpec::new(source, overview(), vec![chunk.clone()]);
        let review = ValidatedReview::validate(input, spec).expect("valid review");
        let known = review
            .input()
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();

        let work = gewu_review::chunk_height(&chunk, &known);
        let pane = gewu_review::pane_height(&chunk);
        let drawn = Screen::chunk(&review, &ReviewSession::default(), 0, 80, None)
            .wrapped(80)
            .lines
            .len();

        assert!(pane > 0, "the fixture carries a pane");
        assert!(
            work < drawn,
            "the budget charged {work} rows for a chunk that draws {drawn} with its pane"
        );
        assert!(
            work + pane >= drawn,
            "budget {work} plus pane {pane} promised less than the {drawn} rows drawn"
        );
    }

    /// The estimator lives in the domain and the renderer in the CLI, so
    /// nothing but a test like this keeps them honest. It has drifted three
    /// times: a header that wrapped, a confidence note that wrapped, and a
    /// blank row the renderer emitted between views.
    #[test]
    fn the_budget_never_promises_less_than_the_screen_draws() {
        let source = SourceIdentity::new("base", "head", b"diff");
        let change = |path: &str, lines: usize| {
            let body: String = (1..=lines)
                .map(|line| format!("+    let value_{line} = {line};\n"))
                .collect();
            Change::new(
                path,
                (0, 0, 1, lines as u32),
                format!("@@ -0,0 +1,{lines} @@\n{body}"),
            )
        };
        let first = change("a.rs", 12);
        let second = change("b.rs", 8);
        let quote = |change: &Change, start, end| Quote {
            change_id: change.id.clone(),
            start,
            end,
        };
        let block = |quotes| ChunkBlock::Changes {
            quotes,
            presentation: ChangePresentation::AddedCode,
        };
        let prose = |text: &str| ChunkBlock::Narrative {
            markdown: text.to_owned(),
        };
        let shapes = vec![
            ("one view", vec![block(vec![quote(&first, 1, 12)])]),
            (
                "prose on both sides of a view",
                vec![
                    prose("Look at this:"),
                    block(vec![quote(&first, 1, 6)]),
                    prose("And then at this."),
                ],
            ),
            (
                "two blocks, overlapping views",
                vec![
                    block(vec![quote(&first, 1, 8)]),
                    prose("The middle again, in its new company."),
                    block(vec![quote(&first, 5, 12)]),
                ],
            ),
            (
                "one block spanning two files",
                vec![block(vec![quote(&first, 1, 12), quote(&second, 1, 8)])],
            ),
        ];

        for (shape, content) in shapes {
            let chunk = ReviewChunk {
                id: "chunk".into(),
                title: "A chunk".into(),
                summary: "One sentence of summary.".into(),
                kind: ChunkKind::Other,
                content,
                context: None,
                confidence: Some("Unsure whether this was deliberate.".into()),
                oversized_reason: None,
            };
            let input = ReviewInput::new(source.clone(), vec![first.clone(), second.clone()]);
            let spec = ReviewSpec::new(source.clone(), overview(), vec![chunk.clone()]);
            let review = ValidatedReview::validate_partial(input, spec).expect("valid review");
            let known = review
                .input()
                .changes
                .iter()
                .map(|change| (&change.id, change))
                .collect();

            let estimate = gewu_review::chunk_height(&chunk, &known);
            let drawn = Screen::chunk(&review, &ReviewSession::default(), 0, 80, None)
                .wrapped(80)
                .lines
                .len();

            assert!(
                estimate >= drawn,
                "{shape}: budget promised {estimate} rows, screen drew {drawn}"
            );
            assert!(
                estimate <= drawn + 4,
                "{shape}: budget charged {estimate} rows for {drawn} drawn"
            );
        }
    }

    #[test]
    fn progress_never_exceeds_the_chunks_the_review_holds() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::reviewed(String::from("chunk"), true));
        // A chunk re-planning has since dropped, still marked reviewed.
        session.apply(&ReviewEvent::reviewed(String::from("chunk-removed"), true));

        let rendered = lines(&Screen::summary(&review, &session)).join("\n");

        assert!(rendered.contains("Reviewed: 1 / 1"), "{rendered}");
        assert!(rendered.contains("Unreviewed: 0"), "{rendered}");
        // The outcome reads as prose, and says what it followed from: the
        // reviewer never picked it, so it has to account for itself.
        assert!(rendered.contains("approve"), "{rendered}");
        assert!(
            rendered.contains("remarked on nothing"),
            "the outcome does not say why: {rendered}"
        );
        assert!(
            rendered.contains("Press Enter to end the review"),
            "{rendered}"
        );
    }

    #[test]
    fn the_summary_names_the_outcome_an_open_request_forces() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        let mut issue = feedback("issue", "Rework this.");
        issue.kind = FeedbackKind::Issue;
        session.apply(&ReviewEvent::feedback(issue));

        let rendered = lines(&Screen::summary(&review, &session)).join("\n");

        assert!(rendered.contains("Open requests: 1"), "{rendered}");
        assert!(rendered.contains("request changes"), "{rendered}");
        assert!(rendered.contains("one request is open"), "{rendered}");
    }

    #[test]
    fn numbers_added_code_from_its_place_in_the_new_file() {
        let change = Change::new(
            "a.rs",
            (0, 0, 98, 3),
            "@@ -0,0 +98,3 @@\n+one\n+two\n+three\n",
        );

        let quote = Quote::whole(change.id.clone(), change.body_lines());
        let document = Screen::change_body(&quote, &change, ChangePresentation::AddedCode, 3);

        assert_eq!(lines(&document), vec![" 98 one", " 99 two", "100 three"]);
    }

    #[test]
    fn numbers_only_the_new_side_of_a_diff() {
        let change = Change::new(
            "a.rs",
            (7, 2, 7, 2),
            "@@ -7,2 +7,2 @@\n context\n-removed\n+added\n",
        );

        let quote = Quote::whole(change.id.clone(), change.body_lines());
        let document = Screen::change_body(&quote, &change, ChangePresentation::Diff, 1);

        // The hunk header and the removed line name no new-side line.
        // The gutter numbers say where the view sits, so no hunk header is drawn.
        assert_eq!(
            lines(&document),
            vec!["7  context", "  -removed", "8 +added"]
        );
    }

    #[test]
    fn interleaves_prose_between_quotes() {
        let source = SourceIdentity::new("base", "head", b"diff");
        let change = Change::new("a.rs", (0, 0, 1, 1), "@@ -0,0 +1,1 @@\n+quoted\n");
        let chunk = ReviewChunk {
            id: "chunk".into(),
            title: "Chunk".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Concept,
            content: vec![
                ChunkBlock::Narrative {
                    markdown: "Look at this:".into(),
                },
                ChunkBlock::Changes {
                    quotes: vec![Quote::whole(change.id.clone(), 1)],
                    presentation: ChangePresentation::AddedCode,
                },
                ChunkBlock::Narrative {
                    markdown: "And that is all.".into(),
                },
            ],
            context: None,
            confidence: None,
            oversized_reason: None,
        };
        let spec = ReviewSpec::new(source.clone(), overview(), vec![chunk]);
        let review = ValidatedReview::validate(ReviewInput::new(source, vec![change]), spec)
            .expect("valid review");

        let rendered = lines(&Screen::chunk(
            &review,
            &ReviewSession::default(),
            0,
            80,
            None,
        ));

        let prose = rendered
            .iter()
            .position(|line| line == "Look at this:")
            .expect("leading prose");
        let quote = rendered
            .iter()
            .position(|line| line.contains("quoted"))
            .expect("quote");
        let closing = rendered
            .iter()
            .position(|line| line == "And that is all.")
            .expect("trailing prose");
        assert!(prose < quote && quote < closing, "{rendered:#?}");
    }

    #[test]
    fn marks_the_feedback_the_reviewer_has_selected() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(feedback("first", "First.")));
        session.apply(&ReviewEvent::feedback(feedback("second", "Second.")));

        let rendered = lines(&Screen::chunk(&review, &session, 0, 80, Some("second")));

        assert!(rendered.iter().any(|line| line == "  Comment · open"));
        assert!(rendered.iter().any(|line| line == "▸ Comment · open"));
        // Each body sits under the item it belongs to.
        let selected = rendered
            .iter()
            .position(|line| line == "▸ Comment · open")
            .expect("selected item");
        assert_eq!(rendered[selected + 1], "    Second.");
    }

    #[test]
    fn draws_a_pasted_body_on_the_lines_it_was_written_with() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(feedback("note", "First.\nSecond.")));

        let rendered = lines(&Screen::chunk(&review, &session, 0, 80, None));

        assert!(rendered.iter().any(|line| line == "    First."));
        assert!(rendered.iter().any(|line| line == "    Second."));
    }

    #[test]
    fn drops_feedback_a_reviewer_deleted() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(feedback("note", "Mistake.")));
        session.apply(&ReviewEvent::deleted("note"));

        let rendered = lines(&Screen::chunk(&review, &session, 0, 80, None));

        assert!(!rendered.iter().any(|line| line.contains("Mistake.")));
        assert!(!rendered.iter().any(|line| line == "Feedback"));
    }

    fn feedback(id: &str, body: &str) -> Feedback {
        Feedback {
            id: id.into(),
            chunk_id: "chunk".into(),
            kind: FeedbackKind::Note,
            target: FeedbackTarget::Code,
            body: body.into(),
            anchor: None,
            resolved: false,
        }
    }

    fn lines(document: &super::Document) -> Vec<String> {
        document.lines.iter().map(text).collect()
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|span| span.text.as_str()).collect()
    }

    fn overview() -> Overview {
        Overview {
            intent: "Intent.".into(),
            changes: vec![],
            illustration: None,
            concepts: vec![],
        }
    }

    fn review(units: &[(&str, &str)], presentation: ChangePresentation) -> ValidatedReview {
        let source = SourceIdentity::new("base", "head", b"diff");
        let changes: Vec<_> = units
            .iter()
            .enumerate()
            .map(|(index, (path, body))| {
                let line = index as u32 + 1;
                Change::new(
                    *path,
                    (0, 0, line, 1),
                    format!("@@ -0,0 +{line},1 @@\n+{body}\n"),
                )
            })
            .collect();
        let chunk = ReviewChunk {
            id: "chunk".into(),
            title: "Chunk".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Concept,
            content: vec![ChunkBlock::Changes {
                quotes: changes
                    .iter()
                    .map(|change| Quote::whole(change.id.clone(), change.body_lines()))
                    .collect(),
                presentation,
            }],
            context: None,
            confidence: None,
            oversized_reason: None,
        };
        let spec = ReviewSpec::new(source.clone(), overview(), vec![chunk]);
        ValidatedReview::validate(ReviewInput::new(source, changes), spec).expect("valid review")
    }
}
