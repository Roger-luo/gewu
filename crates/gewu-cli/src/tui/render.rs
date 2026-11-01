use std::collections::{HashMap, HashSet};

use crossterm::style::Color;
use gewu_review::{
    Change, ChangePresentation, ChunkBlock, ChunkContext, Decision, Feedback, FeedbackTarget,
    LineAnchor, Quote, ReviewChunk, ReviewSession, ValidatedReview,
};

use super::{
    highlight::CodeHighlighter,
    markdown::Markdown,
    render_support::{
        SIDE_PANE_MIN_WIDTH, SOURCE_GUTTER, bullets, document_title, heading, illustration, muted,
        presentation_label, source_path_style, title_style,
    },
    style::{Document, Line, TextStyle},
};

pub struct Screen;

impl Screen {
    #[must_use]
    pub fn overview(review: &ValidatedReview, session: &ReviewSession) -> Document {
        let overview = &review.spec().overview;
        let mut document = document_title("PR OVERVIEW");
        document.append(Markdown::render(&overview.intent));
        if let Some(progress) = generation_progress(review) {
            document.push(
                format!("Generation in progress: {}", generation_status(&progress)),
                TextStyle {
                    foreground: Some(Color::Yellow),
                    bold: true,
                    ..TextStyle::default()
                },
            );
            document.newline();
            document.newline();
        }
        if saved_progress(session, review) {
            document.push(resume_status(session, review), title_style());
            document.newline();
            document.newline();
        }
        heading(&mut document, "What changes");
        document.append(Markdown::render(&bullets(&overview.changes)));
        // The map and the contents are the review's shape; they come before
        // vocabulary so the first screen shows it (a UI judge found the
        // contents pushed alone onto the overview's second page).
        if let Some(value) = &overview.illustration {
            heading(&mut document, "How it fits together");
            document.append(Markdown::render(&illustration(value)));
        }
        if let Some(contents) = Self::contents(review) {
            heading(&mut document, "Walkthrough");
            document.append(Markdown::render(&contents));
        }
        if !overview.concepts.is_empty() {
            heading(&mut document, "New concepts");
            document.append(Markdown::render(&bullets(&overview.concepts)));
        }
        let scope = scope(review);
        document.push(
            scope,
            if generation_progress(review).is_some() {
                TextStyle {
                    foreground: Some(Color::Yellow),
                    bold: true,
                    ..TextStyle::default()
                }
            } else {
                muted()
            },
        );
        document
    }

    #[cfg(test)]
    #[must_use]
    pub fn chunk(
        review: &ValidatedReview,
        session: &ReviewSession,
        index: usize,
        width: usize,
        selected: Option<&str>,
    ) -> Document {
        let mut document = Self::chunk_header(review, session, index, width);
        document.newline();
        document.newline();
        document.append(Self::chunk_content(
            review, session, index, width, selected, false, None,
        ));
        document
    }

    #[must_use]
    pub fn chunk_header(
        review: &ValidatedReview,
        session: &ReviewSession,
        index: usize,
        width: usize,
    ) -> Document {
        let chunk = &review.spec().chunks[index];
        let status = if session.reviewed.contains(&chunk.id) {
            "reviewed"
        } else {
            "unreviewed"
        };
        let mut document = Document::new();
        // The section names which part of the overview's map this is, so a
        // reviewer never has to reconstruct where they are (self-review found
        // chunks reading as isolated notes).
        let status = match &chunk.section {
            Some(section) => format!(
                "CHUNK {}/{} · {section} · {status}",
                index + 1,
                review.spec().chunks.len()
            ),
            None => format!(
                "CHUNK {}/{} · {status}",
                index + 1,
                review.spec().chunks.len()
            ),
        };
        document.push(&status, title_style());
        let sources = Self::chunk_sources(review, chunk);
        if width < 70 && !sources.is_empty() {
            let room = width.saturating_sub(status.chars().count() + 3);
            if room >= 8 {
                document.push(" · ", muted());
                document.push(compact_text(&sources, room), source_path_style());
            }
        }
        document.newline();
        document.append(Markdown::render(&chunk.title).trim_trailing());
        if width >= 70 && !sources.is_empty() {
            document.push(format!(" · {sources}"), source_path_style());
        }
        document
    }

    #[must_use]
    pub fn chunk_content(
        review: &ValidatedReview,
        session: &ReviewSession,
        index: usize,
        width: usize,
        selected: Option<&str>,
        raw_source: bool,
        selected_line: Option<&LineAnchor>,
    ) -> Document {
        let chunk = &review.spec().chunks[index];
        let main = Self::chunk_main(review, session, chunk, selected, raw_source, selected_line);
        let mut document = Document::new();
        match chunk.context.as_ref().map(Self::context) {
            Some(context) if width >= SIDE_PANE_MIN_WIDTH => {
                document.append(Document::columns(main, context, width));
            }
            Some(context) => {
                document.append(main);
                document.newline();
                document.append(context);
            }
            None => document.append(main),
        }
        document
    }

    /// The walkthrough's sections in order, with how many chunks each holds:
    /// a table of contents on the first screen instead of behind a key.
    fn contents(review: &ValidatedReview) -> Option<String> {
        let mut sections: Vec<(&str, usize)> = Vec::new();
        for chunk in &review.spec().chunks {
            let section = chunk.section.as_deref()?;
            match sections.last_mut() {
                Some((last, count)) if *last == section => *count += 1,
                _ => sections.push((section, 1)),
            }
        }
        if sections.is_empty() {
            return None;
        }
        let mut lines: Vec<String> = sections
            .iter()
            .enumerate()
            .map(|(index, (section, count))| {
                let chunks = if *count == 1 { "chunk" } else { "chunks" };
                format!("{}. {section} — {count} {chunks}", index + 1)
            })
            .collect();
        if let Some(remaining) = review.spec().remaining_work_items().filter(|n| *n > 0) {
            lines.push(format!("…and {remaining} more part(s) still to be written"));
        }
        Some(lines.join("\n"))
    }

    #[must_use]
    pub fn outline(review: &ValidatedReview, session: &ReviewSession, selected: usize) -> Document {
        let mut document = document_title("REVIEW OUTLINE");
        for (index, chunk) in review.spec().chunks.iter().enumerate() {
            let marker = if index == selected { "▸" } else { " " };
            let reviewed = if session.reviewed.contains(&chunk.id) {
                "reviewed"
            } else {
                "unreviewed"
            };
            let feedback = session.chunk_feedback(&chunk.id).len();
            let suffix = if feedback == 0 {
                String::new()
            } else {
                format!(" · {feedback} feedback")
            };
            let style = if index == selected {
                title_style()
            } else {
                TextStyle::default()
            };
            document.push(format!("{marker} {:>2}. ", index + 1), style);
            document.append(Markdown::render(&chunk.title).trim_trailing());
            document.push(format!(" · {reviewed}{suffix}"), style);
            document.newline();
        }
        document
    }

    #[must_use]
    pub fn summary(
        review: &ValidatedReview,
        session: &ReviewSession,
        confirm_finish: bool,
    ) -> Document {
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
        let comments = review
            .spec()
            .chunks
            .iter()
            .flat_map(|chunk| session.chunk_feedback(&chunk.id))
            .filter(|feedback| feedback.kind == gewu_review::FeedbackKind::Note)
            .count();
        let outcome = session.outcome(review.spec().chunks.iter().map(|chunk| &chunk.id));
        let mut document = document_title("REVIEW SUMMARY");
        document.append(Markdown::render(&format!(
            "- **Reviewed chunks in this batch:** {reviewed} / {}\n- **Unreviewed chunks in this batch:** {unreviewed}\n\
             - **Open requests:** {open}\n- **Comments:** {comments}",
            review.spec().chunks.len(),
        )));
        let generation = generation_progress(review);
        if let Some(progress) = &generation {
            document.push(
                format!("Source coverage: {}", generation_status(progress)),
                TextStyle {
                    foreground: Some(Color::Yellow),
                    bold: true,
                    ..TextStyle::default()
                },
            );
            document.newline();
            document.newline();
        }
        if unreviewed > 0 && !confirm_finish && session.decision.is_none() {
            document.append(Markdown::render(&format!(
                "**Review incomplete:** {unreviewed} chunk(s) are not marked reviewed. Press `u` to return to the first one."
            )));
        }
        heading(
            &mut document,
            if generation.is_some() {
                "Current batch outcome"
            } else {
                "Outcome"
            },
        );
        // Named before it is recorded, and with the reason, because the
        // reviewer never chose it: it follows from what they wrote, and they
        // are owed the chance to see that Gewu read them correctly.
        document.append(Markdown::render(&format!(
            "**{}** — {}\n\n{}",
            outcome.label(),
            outcome_reason(outcome, open, reviewed),
            match session.decision {
                _ if confirm_finish && generation.is_some() && outcome == Decision::Approve => {
                    let progress = generation.as_ref().expect("generation checked above");
                    let unreviewed = if unreviewed > 0 {
                        format!(" {unreviewed} generated chunk(s) are also unreviewed.")
                    } else {
                        String::new()
                    };
                    format!(
                        "**WARNING:** Approve only this generated batch covering {} of {} change hunks? {}{unreviewed} Press `Enter` to approve this batch or `Esc` to keep reviewing.",
                        progress.covered_hunks,
                        progress.total_hunks,
                        pending_generation_warning(progress),
                    )
                }
                Some(recorded) if recorded == outcome =>
                    if generation.is_some() {
                        "Recorded for the generated batch. Press `Enter` to leave, then run `gewu review open` after generation adds the next batch. Use `u`, `g`, or `p` to revisit this batch; changing feedback updates its recorded outcome.".to_owned()
                    } else {
                        "Recorded. Press `Enter` to leave, or use `u`, `g`, or `p` to revisit work. Changing feedback updates the recorded outcome.".to_owned()
                    },
                Some(recorded) => format!(
                    "Recorded earlier as *{}*; press `Enter` to record it as it now stands.",
                    recorded.label()
                ),
                None if confirm_finish => format!(
                    "**WARNING:** End with {unreviewed} unreviewed chunk(s)? Press `Enter` to confirm or `Esc` to keep reviewing."
                ),
                None if generation.is_some() => "Press `Enter` to record this generated batch; pending work remains and will be reviewed later.".to_owned(),
                None => "Press `Enter` to end the review with it.".to_owned(),
            }
        )));
        document
    }

    pub(super) fn chunk_main(
        review: &ValidatedReview,
        session: &ReviewSession,
        chunk: &ReviewChunk,
        selected: Option<&str>,
        raw_source: bool,
        selected_line: Option<&LineAnchor>,
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
        let mut document = Document::new();
        heading(&mut document, "What this does");
        document.append(Markdown::render(&chunk.summary));
        if let Some(confidence) = &chunk.confidence {
            document.append(Markdown::render(&format!("*Confidence:* {confidence}")));
        }
        let mut shown_lines: HashMap<String, HashSet<usize>> = HashMap::new();
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
                        let earlier_omitted = run.first().is_some_and(|(quote, change)| {
                            (1..quote.start).any(|line| {
                                !shown_lines
                                    .get(&change.id)
                                    .is_some_and(|shown| shown.contains(&line))
                                    && !source_line_is_blank(change, line)
                            })
                        });
                        document.append(Self::source_block(
                            run,
                            *presentation,
                            width,
                            raw_source || selected_line.is_some(),
                            selected_line,
                            earlier_omitted,
                        ));
                        for (quote, _) in run {
                            shown_lines
                                .entry(quote.change_id.clone())
                                .or_default()
                                .extend(quote.start..=quote.end);
                        }
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
                document.append(Self::feedback_item(
                    review,
                    item,
                    selected == Some(&item.id),
                ));
            }
        }
        document
    }

    /// One piece of feedback, marked when the reviewer has it selected so the
    /// actions offered below plainly belong to it.
    fn feedback_item(review: &ValidatedReview, item: &Feedback, selected: bool) -> Document {
        let mut document = Document::new();
        let style = if selected {
            title_style()
        } else {
            TextStyle {
                bold: true,
                ..TextStyle::default()
            }
        };
        document.push(if selected { "▸ " } else { "  " }, style);
        document.push(item.kind.label(), style);
        {
            let target = match (item.target, item.anchor.as_ref()) {
                (FeedbackTarget::Explanation, _) => "about this explanation".to_owned(),
                (FeedbackTarget::Code, Some(anchor)) => {
                    format!("about {}:{}", anchor.path, anchor.start_line)
                }
                (FeedbackTarget::Code, None) => "about the code".to_owned(),
            };
            document.push(format!(" · {target}"), style);
            if item.kind == gewu_review::FeedbackKind::Issue {
                document.push(
                    if item.resolved {
                        " · resolved"
                    } else {
                        " · open"
                    },
                    style,
                );
            }
        }
        document.newline();
        if let Some(anchor) = &item.anchor {
            let excerpt = anchor_excerpt(review, anchor);
            document.push("    ↳ ", source_path_style());
            document.push(excerpt, TextStyle::default());
            document.newline();
        }
        // A pasted body carries its own line breaks, so it is drawn line by
        // line rather than as one run that wrapping would reflow.
        let body: Vec<_> = item.body.lines().collect();
        let visible = if selected || body.len() <= 3 {
            body.len()
        } else {
            2
        };
        for line in body.iter().take(visible) {
            document.push("    ", muted());
            document.push(*line, TextStyle::default());
            document.newline();
        }
        if visible < body.len() {
            document.push("    ", muted());
            document.push(
                format!(
                    "… {} more line(s); select this feedback to expand",
                    body.len() - visible
                ),
                TextStyle {
                    foreground: Some(Color::Grey),
                    italic: true,
                    ..TextStyle::default()
                },
            );
            document.newline();
        }
        document
    }

    pub(super) fn selected_feedback_header(item: &Feedback) -> Document {
        let mut document = Document::new();
        let target = match (item.target, item.anchor.as_ref()) {
            (FeedbackTarget::Explanation, _) => "this explanation".to_owned(),
            (FeedbackTarget::Code, Some(anchor)) => {
                format!("{}:{}", anchor.path, anchor.start_line)
            }
            (FeedbackTarget::Code, None) => "the code".to_owned(),
        };
        document.push(
            format!("SELECTED: {} · {target}", item.kind.label()),
            title_style(),
        );
        document
    }

    /// Frames one run of same-file changes: a labelled source header above the
    /// quoted material, every line of it behind a gutter.
    fn source_block(
        views: &[(&Quote, &Change)],
        presentation: ChangePresentation,
        width: usize,
        raw_source: bool,
        selected_line: Option<&LineAnchor>,
        earlier_omitted: bool,
    ) -> Document {
        let mut document = Document::new();
        document.push(SOURCE_GUTTER, muted());
        document.push(Self::source_label(views), source_path_style());
        let label = if selected_line.is_some() {
            format!("{} · line picker", presentation_label(presentation))
        } else if raw_source {
            "raw source + lines".to_owned()
        } else {
            presentation_label(presentation).to_owned()
        };
        document.push(format!(" · {label}"), muted());
        document.push(format!(" · {}", change_semantics(views)), muted());
        if views.len() > 1 {
            document.push(format!(" · {} views", views.len()), muted());
        }
        document.newline();
        document.push(SOURCE_GUTTER, muted());
        document.newline();
        let mut body = Document::new();
        if earlier_omitted {
            body.push("… earlier hunk lines not shown", muted());
            body.newline();
        }
        for (quote, change) in views {
            if body.lines.last().is_some_and(|line| line.width() > 0) {
                body.newline();
                // Two views are distinct selections, even if their source
                // ranges happen to touch or overlap.
                body.push("… next selected view", muted());
                body.newline();
            }
            body.append(Self::change_body(
                quote,
                change,
                presentation,
                width,
                raw_source,
                selected_line,
            ));
        }
        if views
            .last()
            .is_some_and(|(quote, change)| quote.end < change.body_lines())
        {
            body.newline();
            let next = views
                .last()
                .and_then(|(quote, change)| {
                    (quote.end + 1..=change.body_lines()).find_map(|line| change.new_line_at(line))
                })
                .map_or_else(
                    || "… later hunk lines not shown".to_owned(),
                    |line| format!("… source continues at line {line}"),
                );
            body.push(next, muted());
        }
        document.append(body.gutter(SOURCE_GUTTER, muted()));
        document.newline();
        document
    }

    fn source_label(views: &[(&Quote, &Change)]) -> String {
        let path = &views[0].1.path;
        let ranges = views
            .iter()
            .filter_map(|(quote, change)| {
                let lines: Vec<_> = (quote.start..=quote.end)
                    .filter_map(|body_line| change.new_line_at(body_line))
                    .collect();
                match (lines.first(), lines.last()) {
                    (Some(first), Some(last)) if first != last => Some(format!("{first}-{last}")),
                    (Some(line), _) => Some(line.to_string()),
                    _ => None,
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        if ranges.is_empty() {
            path.clone()
        } else {
            format!("{path}:{ranges}")
        }
    }

    fn change_body(
        quote: &Quote,
        change: &Change,
        presentation: ChangePresentation,
        width: usize,
        raw_source: bool,
        selected_line: Option<&LineAnchor>,
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
        let selected = selected_line
            .filter(|anchor| anchor.path == change.path)
            .map(|anchor| anchor.start_line);
        let presentation = if raw_source {
            match presentation {
                ChangePresentation::Diff => ChangePresentation::Diff,
                _ => ChangePresentation::AddedCode,
            }
        } else {
            presentation
        };
        match presentation {
            ChangePresentation::Diff => numbered(
                CodeHighlighter::global().diff(&change.path, &view),
                &numbers,
                width,
                selected,
            ),
            ChangePresentation::AddedCode => numbered(
                CodeHighlighter::global().source(&change.path, &Self::added_text(&view)),
                &shown_line_numbers(&view, &numbers),
                width,
                selected,
            ),
            ChangePresentation::RenderedMarkdown => rendered_markdown(
                &Self::added_text(&view),
                &shown_line_numbers(&view, &numbers),
                width,
            ),
            ChangePresentation::SummaryOnly => {
                let mut document = Document::new();
                document.push(
                    format!(
                        "{} quoted source line(s) summarized by the walkthrough; press v to inspect",
                        quote.lines()
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

    pub(super) fn context(context: &ChunkContext) -> Document {
        match context {
            ChunkContext::Concept {
                definition,
                example,
                relation,
            } => {
                let mut document = document_title("Context · New concept");
                document.append(Markdown::render(definition));
                heading(&mut document, "Example");
                document.append(context_code(example));
                document.newline();
                document.newline();
                heading(&mut document, "In this change");
                document.append(Markdown::render(relation));
                document
            }
            ChunkContext::Refactor {
                before,
                after,
                reason,
            } => {
                let mut document = document_title("Context · Refactor");
                heading(&mut document, "Before");
                document.append(context_code(before));
                document.newline();
                document.newline();
                heading(&mut document, "After");
                document.append(context_code(after));
                document.newline();
                document.newline();
                heading(&mut document, "Why");
                document.append(Markdown::render(reason));
                document
            }
        }
    }

    pub(super) fn context_title(context: &ChunkContext) -> &'static str {
        match context {
            ChunkContext::Concept { .. } => "CONTEXT · New concept",
            ChunkContext::Refactor { .. } => "CONTEXT · Refactor",
        }
    }

    fn chunk_sources(review: &ValidatedReview, chunk: &ReviewChunk) -> String {
        let by_id: HashMap<_, _> = review
            .input()
            .changes
            .iter()
            .map(|change| (&change.id, change))
            .collect();
        let views: Vec<_> = chunk
            .quotes()
            .filter_map(|quote| by_id.get(&quote.change_id).map(|change| (quote, *change)))
            .collect();
        views
            .chunk_by(|left, right| left.1.path == right.1.path)
            .map(Self::source_label)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

fn compact_text(value: &str, limit: usize) -> String {
    let mut text: String = value.chars().take(limit).collect();
    if value.chars().count() > limit {
        text.pop();
        text.push('…');
    }
    text
}

/// Context examples are code fields, not Markdown documents. Authors
/// sometimes supplied a fenced block anyway; stripping one balanced outer
/// fence keeps the code while preventing that fence from terminating Gewu's
/// own frame and swallowing the following relation prose.
fn context_code(example: &str) -> Document {
    let (language, code) = outer_fenced_code(example).unwrap_or(("text", example));
    CodeHighlighter::global()
        .code(language, code.trim_end_matches('\n'))
        .gutter("│ ", muted())
}

fn outer_fenced_code(value: &str) -> Option<(&str, &str)> {
    let first_end = value.find('\n')?;
    let opener = value[..first_end].trim();
    let language = opener.strip_prefix("```")?.trim();
    let body_with_close = &value[first_end + 1..];
    let close_start = body_with_close.rfind('\n')?;
    (body_with_close[close_start + 1..].trim() == "```").then_some((
        if language.is_empty() {
            "text"
        } else {
            language
        },
        &body_with_close[..close_start],
    ))
}

/// Why the review came out the way it did, in the reviewer's own actions. An
/// outcome nobody chose has to explain itself, or it reads as Gewu deciding.
fn outcome_reason(outcome: Decision, open: usize, reviewed: usize) -> &'static str {
    match outcome {
        Decision::RequestChanges if open == 1 => "one request is open",
        Decision::RequestChanges => "requests are open",
        Decision::Comment => "you left remarks, and none of them ask for a change",
        // Found by self-review's UI judge: with nothing read, "you asked for
        // nothing" read as an endorsement of a review nobody had looked at.
        Decision::Approve if reviewed == 0 => {
            "only because you have not read or remarked on anything yet"
        }
        Decision::Approve => "you asked for nothing and remarked on nothing",
    }
}

pub(super) struct GenerationProgress {
    pub(super) covered_hunks: usize,
    pub(super) total_hunks: usize,
    remaining_work_items: Option<usize>,
}

fn generation_status(progress: &GenerationProgress) -> String {
    format!(
        "{} of {} change hunks prepared{}; more review chunks will follow.",
        progress.covered_hunks,
        progress.total_hunks,
        progress
            .remaining_work_items
            .map_or_else(String::new, |remaining| format!(
                " · {remaining} work item(s) pending"
            ))
    )
}

pub(super) fn generation_progress(review: &ValidatedReview) -> Option<GenerationProgress> {
    let covered: HashSet<_> = review
        .spec()
        .chunks
        .iter()
        .flat_map(ReviewChunk::change_ids)
        .collect();
    let total_hunks = review.input().changes.len();
    let remaining_work_items = review.spec().remaining_work_items();
    (covered.len() < total_hunks || remaining_work_items.is_some_and(|remaining| remaining > 0))
        .then_some(GenerationProgress {
            covered_hunks: covered.len(),
            total_hunks,
            remaining_work_items,
        })
}

pub(super) fn pending_generation_warning(progress: &GenerationProgress) -> String {
    let unprepared = progress.total_hunks.saturating_sub(progress.covered_hunks);
    if unprepared > 0 {
        let subject = if unprepared == 1 {
            "change hunk is"
        } else {
            "change hunks are"
        };
        format!("The remaining {unprepared} {subject} not yet reviewable.")
    } else {
        "Further review chunks are not yet available.".to_owned()
    }
}

fn saved_progress(session: &ReviewSession, review: &ValidatedReview) -> bool {
    session.spec_id.is_some()
        && (!session.visited.is_empty()
            || !session.reviewed.is_empty()
            || !session.feedback.is_empty()
            || session.decision.is_some())
        && !review.spec().chunks.is_empty()
}

fn resume_status(session: &ReviewSession, review: &ValidatedReview) -> String {
    let reviewed = review
        .spec()
        .chunks
        .iter()
        .filter(|chunk| session.reviewed.contains(&chunk.id))
        .count();
    let feedback = review
        .spec()
        .chunks
        .iter()
        .flat_map(|chunk| session.chunk_feedback(&chunk.id))
        .count();
    let resume = session
        .last_visited
        .as_ref()
        .and_then(|id| {
            review
                .spec()
                .chunks
                .iter()
                .position(|chunk| &chunk.id == id)
        })
        .map_or_else(
            || "the first unreviewed chunk".to_owned(),
            |index| format!("chunk {}", index + 1),
        );
    format!(
        "Resuming saved progress: {reviewed}/{} reviewed · {feedback} feedback item(s) · continue at {resume}",
        review.spec().chunks.len()
    )
}

fn anchor_excerpt(review: &ValidatedReview, anchor: &LineAnchor) -> String {
    review
        .input()
        .changes
        .iter()
        .find(|change| change.id == anchor.change_id)
        .and_then(|change| {
            change
                .new_side(anchor.start_line, anchor.end_line)
                .first()
                .copied()
        })
        .map(|line| line.get(1..).unwrap_or(line).trim_end())
        .filter(|line| !line.trim().is_empty())
        .map_or_else(|| "(blank line)".into(), ToOwned::to_owned)
}

fn source_line_is_blank(change: &Change, line: usize) -> bool {
    change
        .body(line, line)
        .first()
        .is_none_or(|text| text.get(1..).unwrap_or_default().trim().is_empty())
}

fn change_semantics(views: &[(&Quote, &Change)]) -> &'static str {
    if views.iter().all(|(_, change)| change.old_lines == 0) {
        "new file / added"
    } else if views.iter().all(|(_, change)| change.new_lines == 0) {
        "deleted"
    } else {
        "modified"
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

/// Renders a changed Markdown document without losing the source map. Each
/// source row keeps its file line number, and fenced blocks retain visible
/// boundaries so commands remain distinguishable without color.
fn rendered_markdown(
    source: &str,
    source_numbers: &[Option<u32>],
    number_width: usize,
) -> Document {
    let mut document = Document { lines: Vec::new() };
    let mut rendered_numbers = Vec::new();
    let mut fence: Option<String> = None;

    for (index, source_line) in source.split('\n').enumerate() {
        let number = source_numbers.get(index).copied().flatten();
        let trimmed = source_line.trim_start();
        let rendered = if let Some(marker) = trimmed.strip_prefix("```") {
            let mut boundary = Document::new();
            if fence.is_some() {
                boundary.push("└─ end code block", muted());
                fence = None;
            } else {
                let language = marker.trim();
                boundary.push(
                    if language.is_empty() {
                        "┌─ code block".to_owned()
                    } else {
                        format!("┌─ {language} code block")
                    },
                    muted(),
                );
                fence = Some(language.to_owned());
            }
            boundary
        } else if let Some(language) = fence.as_deref() {
            CodeHighlighter::global()
                .code(language, source_line)
                .gutter("│ ", muted())
        } else if source_line.trim().is_empty() {
            Document::new()
        } else {
            Markdown::render(source_line).trim_trailing()
        };

        let mut lines = rendered.lines;
        if lines.is_empty() {
            lines.push(Line::default());
        }
        for (rendered_index, line) in lines.into_iter().enumerate() {
            document.lines.push(line);
            rendered_numbers.push((rendered_index == 0).then_some(number).flatten());
        }
    }

    numbered(document, &rendered_numbers, number_width, None)
}

/// Prefixes each line with its file line number, right-aligned so the code
/// edge stays straight.
fn numbered(
    document: Document,
    numbers: &[Option<u32>],
    width: usize,
    selected: Option<u32>,
) -> Document {
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
                let is_selected =
                    selected.is_some() && numbers.get(index).copied().flatten() == selected;
                numbered.push(
                    if is_selected { "▶ " } else { "  " },
                    if is_selected { title_style() } else { muted() },
                );
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
        Change, ChangePresentation, ChunkBlock, ChunkContext, ChunkKind, Decision, Feedback,
        FeedbackKind, FeedbackTarget, Overview, Quote, ReviewChunk, ReviewEvent, ReviewInput,
        ReviewSession, ReviewSpec, SourceIdentity, ValidatedReview,
    };

    use super::{super::style::Line, SOURCE_GUTTER, Screen};

    #[test]
    fn an_untouched_review_does_not_read_as_endorsed() {
        let untouched = super::outcome_reason(Decision::Approve, 0, 0);
        assert!(untouched.contains("not read"), "{untouched}");
        let read = super::outcome_reason(Decision::Approve, 0, 3);
        assert!(read.contains("asked for nothing"), "{read}");
    }

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
        assert!(
            headers[0].starts_with("▏ docs/design.md:1,2 · document · new file / added · 2 views")
        );
        assert!(headers[1].starts_with("▏ README.md:3 · document · new file / added"));
    }

    #[test]
    fn rendered_markdown_header_keeps_its_source_location() {
        let review = review(
            &[("AGENTS.md", "A documented rule")],
            ChangePresentation::RenderedMarkdown,
        );

        let rendered = lines(&Screen::chunk(
            &review,
            &ReviewSession::default(),
            0,
            80,
            None,
        ));

        assert!(
            rendered
                .iter()
                .any(|line| line.starts_with("▏ AGENTS.md:1 · document")),
            "{rendered:#?}"
        );
    }

    #[test]
    fn rendered_markdown_keeps_line_numbers_and_code_boundaries() {
        let change = Change::new(
            "AGENTS.md",
            (0, 0, 13, 4),
            "@@ -0,0 +13,4 @@\n+## Tooling\n+```bash\n+mise install\n+```\n",
        );
        let quote = Quote::whole(change.id.clone(), change.body_lines());

        let rendered = lines(&Screen::change_body(
            &quote,
            &change,
            ChangePresentation::RenderedMarkdown,
            2,
            false,
            None,
        ));

        assert!(
            rendered.iter().any(|line| line.contains("13 Tooling")),
            "{rendered:#?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("14 ┌─ bash code block")),
            "{rendered:#?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("15 │ mise install")),
            "{rendered:#?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("16 └─ end code block")),
            "{rendered:#?}"
        );
    }

    #[test]
    fn chunk_titles_render_inline_code_instead_of_backticks() {
        let review = review_with_title(
            &[("a.rs", "one")],
            ChangePresentation::AddedCode,
            "The `Runtime` type",
        );

        let document = Screen::chunk(&review, &ReviewSession::default(), 0, 80, None);
        let rendered = lines(&document).join("\n");

        assert!(!rendered.contains('`'), "{rendered}");
        assert!(
            document
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| {
                    span.text == "Runtime"
                        && span.style.foreground == Some(crossterm::style::Color::Yellow)
                })
        );
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
                .any(|line| line.contains("1 quoted source line(s) summarized")),
            "{:#?}",
            lines(&document)
        );
    }

    #[test]
    fn source_views_only_mark_meaningful_omitted_leading_lines() {
        let change = Change::new(
            "src/lib.rs",
            (0, 0, 1, 3),
            "@@ -0,0 +1,3 @@\n+first\n+second\n+third\n",
        );
        let quote = Quote {
            change_id: change.id.clone(),
            start: 2,
            end: 3,
        };
        let views = [(&quote, &change)];

        let marked = lines(&Screen::source_block(
            &views,
            ChangePresentation::AddedCode,
            1,
            false,
            None,
            true,
        ))
        .join("\n");
        let continuous = lines(&Screen::source_block(
            &views,
            ChangePresentation::AddedCode,
            1,
            false,
            None,
            false,
        ))
        .join("\n");

        assert!(marked.contains("earlier hunk lines not shown"), "{marked}");
        assert!(
            !continuous.contains("earlier hunk lines not shown"),
            "{continuous}"
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
            section: None,
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
                section: None,
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

        let rendered = lines(&Screen::summary(&review, &session, false)).join("\n");

        assert!(
            rendered.contains("Reviewed chunks in this batch: 1 / 1"),
            "{rendered}"
        );
        assert!(
            rendered.contains("Unreviewed chunks in this batch: 0"),
            "{rendered}"
        );
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

        let rendered = lines(&Screen::summary(&review, &session, false)).join("\n");

        assert!(rendered.contains("Open requests: 1"), "{rendered}");
        assert!(rendered.contains("request changes"), "{rendered}");
        assert!(rendered.contains("one request is open"), "{rendered}");
    }

    #[test]
    fn summary_counts_comments_and_calls_out_unreviewed_work() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(feedback("note", "A note.")));

        let rendered = lines(&Screen::summary(&review, &session, false)).join("\n");

        assert!(rendered.contains("Comments: 1"), "{rendered}");
        assert!(rendered.contains("Review incomplete"), "{rendered}");
        assert!(rendered.contains("Press u"), "{rendered}");
    }

    #[test]
    fn requests_always_name_their_target_while_comments_have_no_open_state() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(feedback("note", "A note.")));
        let mut issue = feedback("issue", "A request.");
        issue.kind = FeedbackKind::Issue;
        session.apply(&ReviewEvent::feedback(issue));

        let rendered = lines(&Screen::chunk(&review, &session, 0, 80, None)).join("\n");

        assert!(
            rendered.contains("Comment · about the code\n"),
            "{rendered}"
        );
        assert!(!rendered.contains("Comment · open"), "{rendered}");
        assert!(
            rendered.contains("Request change · about the code · open"),
            "{rendered}"
        );
    }

    #[test]
    fn numbers_added_code_from_its_place_in_the_new_file() {
        let change = Change::new(
            "a.rs",
            (0, 0, 98, 3),
            "@@ -0,0 +98,3 @@\n+one\n+two\n+three\n",
        );

        let quote = Quote::whole(change.id.clone(), change.body_lines());
        let document = Screen::change_body(
            &quote,
            &change,
            ChangePresentation::AddedCode,
            3,
            false,
            None,
        );

        assert_eq!(
            lines(&document),
            vec!["   98 one", "   99 two", "  100 three"]
        );
    }

    #[test]
    fn numbered_source_keeps_trailing_blank_lines_inside_the_labelled_range() {
        let change = Change::new(
            "Cargo.toml",
            (0, 0, 1, 2),
            "@@ -0,0 +1,2 @@\n+[workspace]\n+\n",
        );
        let quote = Quote::whole(change.id.clone(), change.body_lines());

        let rendered = lines(&Screen::change_body(
            &quote,
            &change,
            ChangePresentation::AddedCode,
            1,
            false,
            None,
        ));

        assert_eq!(rendered.len(), 2, "{rendered:#?}");
        assert!(rendered[1].starts_with("  2 "), "{rendered:#?}");
    }

    #[test]
    fn numbers_only_the_new_side_of_a_diff() {
        let change = Change::new(
            "a.rs",
            (7, 2, 7, 2),
            "@@ -7,2 +7,2 @@\n context\n-removed\n+added\n",
        );

        let quote = Quote::whole(change.id.clone(), change.body_lines());
        let document =
            Screen::change_body(&quote, &change, ChangePresentation::Diff, 1, false, None);

        // The hunk header and the removed line name no new-side line.
        // The gutter numbers say where the view sits, so no hunk header is drawn.
        assert_eq!(
            lines(&document),
            vec!["  7  context", "    -removed", "  8 +added"]
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
            section: None,
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
    fn later_blocks_do_not_claim_already_shown_source_was_omitted() {
        let source = SourceIdentity::new("base", "head", b"diff");
        let change = Change::new(
            "a.rs",
            (0, 0, 1, 3),
            "@@ -0,0 +1,3 @@\n+first\n+second\n+third\n",
        );
        let quote = |start, end| Quote {
            change_id: change.id.clone(),
            start,
            end,
        };
        let chunk = ReviewChunk {
            id: "chunk".into(),
            title: "Chunk".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Other,
            content: vec![
                ChunkBlock::Changes {
                    quotes: vec![quote(1, 1)],
                    presentation: ChangePresentation::AddedCode,
                },
                ChunkBlock::Narrative {
                    markdown: "Then continue.".into(),
                },
                ChunkBlock::Changes {
                    quotes: vec![quote(2, 3)],
                    presentation: ChangePresentation::AddedCode,
                },
            ],
            context: None,
            confidence: None,
            oversized_reason: None,
            section: None,
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
        ))
        .join("\n");

        assert!(
            !rendered.contains("earlier hunk lines not shown"),
            "{rendered}"
        );
    }

    #[test]
    fn marks_the_feedback_the_reviewer_has_selected() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(feedback("first", "First.")));
        session.apply(&ReviewEvent::feedback(feedback("second", "Second.")));

        let rendered = lines(&Screen::chunk(&review, &session, 0, 80, Some("second")));

        assert!(
            rendered
                .iter()
                .any(|line| line == "  Comment · about the code")
        );
        assert!(
            rendered
                .iter()
                .any(|line| line == "▸ Comment · about the code")
        );
        // Each body sits under the item it belongs to.
        let selected = rendered
            .iter()
            .position(|line| line == "▸ Comment · about the code")
            .expect("selected item");
        assert_eq!(rendered[selected + 1], "    Second.");

        let pinned = lines(&Screen::selected_feedback_header(
            session
                .chunk_feedback("chunk")
                .into_iter()
                .find(|feedback| feedback.id == "second")
                .expect("selected feedback"),
        ));
        assert_eq!(pinned, vec!["SELECTED: Comment · the code"]);
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

    /// A partially generated review used to say "approve" and "3/3" without
    /// disclosing that most planned work had not been written yet.
    #[test]
    fn summary_scopes_an_outcome_to_the_generated_batch() {
        let review = partial_review();
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::reviewed(String::from("chunk"), true));

        let rendered = lines(&Screen::summary(&review, &session, false)).join("\n");

        assert!(rendered.contains("Source coverage"), "{rendered}");
        assert!(rendered.contains("1 work item(s) pending"), "{rendered}");
        assert!(rendered.contains("Current batch outcome"), "{rendered}");
        assert!(
            rendered.contains("record this generated batch"),
            "{rendered}"
        );
    }

    #[test]
    fn partial_approval_confirmation_names_its_exact_source_scope() {
        let review = partial_review();
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::reviewed(String::from("chunk"), true));

        let rendered = lines(&Screen::summary(&review, &session, true)).join("\n");

        assert!(
            rendered.contains("Approve only this generated batch"),
            "{rendered}"
        );
        assert!(rendered.contains("1 of 2 change hunks"), "{rendered}");
        assert!(rendered.contains("not yet reviewable"), "{rendered}");

        session.apply(&ReviewEvent::feedback(feedback("withdrawn", "A note.")));
        session.apply(&ReviewEvent::decision(Decision::Comment));
        session.apply(&ReviewEvent::deleted("withdrawn"));
        let stale_decision = lines(&Screen::summary(&review, &session, true)).join("\n");
        assert!(
            stale_decision.contains("Approve only this generated batch"),
            "{stale_decision}"
        );
    }

    #[test]
    fn overview_discloses_pending_generation_before_the_change_list() {
        let review = partial_review();
        let rendered = lines(&Screen::overview(&review, &ReviewSession::default()));
        let generation = rendered
            .iter()
            .position(|line| line.contains("Generation in progress"))
            .expect("generation banner");
        let changes = rendered
            .iter()
            .position(|line| line == "What changes")
            .expect("change list");

        assert!(generation < changes, "{rendered:#?}");
    }

    /// A reviewer found no big picture before the first chunk: the overview
    /// now draws the map and lists the walkthrough's sections, and each
    /// chunk's header names its section.
    #[test]
    fn overview_maps_the_change_and_lists_the_sections() {
        let review = review(
            &[("a.rs", "one"), ("b.rs", "two")],
            ChangePresentation::AddedCode,
        );
        let mut spec = review.spec().clone();
        spec.overview.illustration = Some(gewu_review::Illustration::Diagram {
            text: "parser ──▶ manifest".into(),
        });
        let mut second = spec.chunks[0].clone();
        second.id = "second".into();
        spec.chunks[0].section = Some("Parsing the diff".into());
        second.section = Some("Building the manifest".into());
        spec.chunks.push(second);
        let review =
            ValidatedReview::validate_partial(review.input().clone(), spec).expect("valid");

        let rendered = lines(&Screen::overview(&review, &ReviewSession::default())).join("\n");
        assert!(rendered.contains("How it fits together"), "{rendered}");
        assert!(rendered.contains("parser ──▶ manifest"), "{rendered}");
        assert!(
            rendered.contains("1. Parsing the diff — 1 chunk"),
            "{rendered}"
        );
        assert!(
            rendered.contains("2. Building the manifest — 1 chunk"),
            "{rendered}"
        );
        let header = lines(&Screen::chunk_header(
            &review,
            &ReviewSession::default(),
            1,
            120,
        ));
        assert!(header[0].contains("Building the manifest"), "{header:?}");
    }

    #[test]
    fn recorded_summary_replaces_the_stale_unreviewed_instruction() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::decision(Decision::Approve));

        let rendered = lines(&Screen::summary(&review, &session, false)).join("\n");

        assert!(!rendered.contains("Press u to return"), "{rendered}");
        assert!(rendered.contains("use u, g, or p"), "{rendered}");
    }

    #[test]
    fn overview_orients_a_resumed_reviewer() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::started(review.spec().session_key()));
        session.apply(&ReviewEvent::visited(String::from("chunk")));
        session.apply(&ReviewEvent::reviewed(String::from("chunk"), true));

        let rendered = lines(&Screen::overview(&review, &session)).join("\n");

        assert!(rendered.contains("Resuming saved progress"), "{rendered}");
        assert!(rendered.contains("1/1 reviewed"), "{rendered}");
        assert!(rendered.contains("continue at chunk 1"), "{rendered}");
    }

    #[test]
    fn raw_source_marks_the_line_picker_selection() {
        let change = Change::new(
            "docs/guide.md",
            (0, 0, 10, 2),
            "@@ -0,0 +10,2 @@\n+# Guide\n+Details\n",
        );
        let quote = Quote::whole(change.id.clone(), change.body_lines());
        let anchor = gewu_review::LineAnchor {
            change_id: change.id.clone(),
            path: change.path.clone(),
            side: "new".into(),
            start_line: 11,
            end_line: 11,
            patch_sha256: change.patch_sha256(),
        };

        let rendered = lines(&Screen::change_body(
            &quote,
            &change,
            ChangePresentation::RenderedMarkdown,
            2,
            true,
            Some(&anchor),
        ));

        assert!(
            rendered.iter().any(|line| line.contains("▶ 11 Details")),
            "{rendered:#?}"
        );
    }

    #[test]
    fn anchored_feedback_repeats_the_source_excerpt() {
        let review = review(&[("a.rs", "named line")], ChangePresentation::AddedCode);
        let change = &review.input().changes[0];
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(Feedback {
            id: "anchored".into(),
            chunk_id: "chunk".into(),
            kind: FeedbackKind::Issue,
            target: FeedbackTarget::Code,
            body: "Change this.".into(),
            anchor: Some(gewu_review::LineAnchor {
                change_id: change.id.clone(),
                path: change.path.clone(),
                side: "new".into(),
                start_line: 1,
                end_line: 1,
                patch_sha256: change.patch_sha256(),
            }),
            resolved: false,
        }));

        let rendered = lines(&Screen::chunk(&review, &session, 0, 80, None)).join("\n");

        assert!(rendered.contains("about a.rs:1"), "{rendered}");
        assert!(rendered.contains("↳ named line"), "{rendered}");
    }

    #[test]
    fn long_feedback_is_compact_until_selected() {
        let review = review(&[("a.rs", "one")], ChangePresentation::AddedCode);
        let mut session = ReviewSession::default();
        session.apply(&ReviewEvent::feedback(feedback(
            "long",
            "one\ntwo\nthree\nfour\nfive",
        )));

        let compact = lines(&Screen::chunk(&review, &session, 0, 65, None)).join("\n");
        let expanded = lines(&Screen::chunk(&review, &session, 0, 65, Some("long"))).join("\n");

        assert!(compact.contains("3 more line(s)"), "{compact}");
        assert!(!compact.contains("    five"), "{compact}");
        assert!(expanded.contains("    five"), "{expanded}");
    }

    #[test]
    fn context_examples_preserve_code_lines() {
        let context = ChunkContext::Concept {
            definition: "A facade re-exports public APIs.".into(),
            example: "pub use first as one;\npub use second as two;".into(),
            relation: "This crate is the facade.".into(),
        };

        let rendered = lines(&Screen::context(&context));

        assert!(
            rendered
                .iter()
                .any(|line| line == "│ pub use first as one;")
        );
        assert!(
            rendered
                .iter()
                .any(|line| line == "│ pub use second as two;")
        );
        assert!(
            !rendered
                .iter()
                .any(|line| { line.contains("pub use first") && line.contains("pub use second") })
        );
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
        review_with_title(units, presentation, "Chunk")
    }

    fn review_with_title(
        units: &[(&str, &str)],
        presentation: ChangePresentation,
        title: &str,
    ) -> ValidatedReview {
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
            title: title.into(),
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
            section: None,
        };
        let spec = ReviewSpec::new(source.clone(), overview(), vec![chunk]);
        ValidatedReview::validate(ReviewInput::new(source, changes), spec).expect("valid review")
    }

    fn partial_review() -> ValidatedReview {
        let source = SourceIdentity::new("base", "head", b"diff");
        let first = Change::new("a.rs", (0, 0, 1, 1), "@@ -0,0 +1,1 @@\n+one\n");
        let second = Change::new("b.rs", (0, 0, 1, 1), "@@ -0,0 +1,1 @@\n+two\n");
        let chunk = ReviewChunk {
            id: "chunk".into(),
            title: "First batch".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Other,
            content: vec![ChunkBlock::Changes {
                quotes: vec![Quote::whole(first.id.clone(), first.body_lines())],
                presentation: ChangePresentation::AddedCode,
            }],
            context: None,
            confidence: None,
            oversized_reason: None,
            section: None,
        };
        let spec = ReviewSpec::new(source.clone(), overview(), vec![chunk]).with_remaining_work(1);
        ValidatedReview::validate_partial(ReviewInput::new(source, vec![first, second]), spec)
            .expect("valid partial review")
    }
    #[test]
    fn context_examples_accept_an_outer_fence_without_leaking_markdown() {
        let context = ChunkContext::Concept {
            definition: "A workspace shares package fields.".into(),
            example: "```toml\n[package]\nversion.workspace = true\n```".into(),
            relation: "This manifest opts into the shared field.".into(),
        };

        let rendered = lines(&Screen::context(&context));

        assert!(
            !rendered.iter().any(|line| line.contains("```")),
            "{rendered:#?}"
        );
        assert!(rendered.iter().any(|line| line == "In this change"));
        assert!(
            rendered
                .iter()
                .any(|line| line == "This manifest opts into the shared field."),
            "{rendered:#?}"
        );
    }
}
