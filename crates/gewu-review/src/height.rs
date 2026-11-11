use std::collections::HashMap;

use unicode_width::UnicodeWidthChar;

use crate::{Change, ChangePresentation, ChunkBlock, ChunkContext, Quote, ReviewChunk};

/// Rows a chunk always costs around its summary: the position line and its
/// blank, the title and its blank, the "What this does" heading, and the blank
/// under the summary. The summary itself is measured, not assumed — validation
/// permits 500 characters, which is seven rows at the reference width.
const CHUNK_CHROME: usize = 8;

/// Rows a quoted block costs: its source header, the blank line under it, and
/// the blank line that closes it. Charged once per file run, because the
/// renderer starts a new header per file.
const BLOCK_CHROME: usize = 3;

/// Rows a confidence note costs beyond its own text: the blank line that
/// closes it. The note itself wraps like any other prose, so it is measured.
const CONFIDENCE_CHROME: usize = 1;

/// Rows a context pane costs before its fields: its own "Context" heading and
/// the blank under it.
const CONTEXT_CHROME: usize = 2;

/// The terminal this estimate assumes. Output wraps, so height depends on
/// width; 80 columns is the narrow end of what a reviewer is likely to use,
/// which keeps the estimate honest for the case that would otherwise page.
const REFERENCE_WIDTH: usize = 80;

/// The public approximation authors use before the harness measures their
/// actual draft. Keeping the prose beside the implementation makes prompt and
/// validator drift visible in review.
pub const CHUNK_HEIGHT_GUIDANCE: &str = "Start with 8 rows of chunk chrome, plus any title rows beyond its first when wrapped at 80 columns. Add the summary wrapped at 80 columns. Add each narrative block wrapped at 80 columns plus 1 closing blank row. For each consecutive run of quotes from one file, add 3 rows for its source header and surrounding blanks, plus any extra rows needed to wrap that header. Add 1 explicit elision row between separate views and at either edge where the view omits hunk lines. Then add every displayed source row, including wrapping after the line-number gutter; rendered Markdown is charged by its rendered rows plus 1 closing blank row, and a summary-only artifact costs 1 row. Add a confidence note at its wrapped height plus 1 blank row. Explainer blocks and a context pane are measured separately and are not charged to the chunk budget.";

/// The gutter is as wide as the largest line number it has to print, so a file
/// past a thousand lines costs a column more than a short one.
fn quote_indent(change: &Change) -> usize {
    let last = change.new_start.saturating_add(change.new_lines);
    3 + last.to_string().len()
}

/// Approximate terminal height of the review work a chunk asks for: what the
/// reviewer must read and form a judgement about.
///
/// A `context` pane is not counted. It answers a question the reviewer may not
/// even have — what a type introduced by this change is — and reading it is not
/// reviewing. Charging the budget for it made panes unaffordable exactly where
/// they help most, on a chunk dense enough to need one. [`pane_height`] measures
/// it separately; the two together still predict what the screen draws.
///
/// This lives in the domain so checkpoint validation can hold a chunk to a
/// viewport budget, but the renderer is the authority; treat the result as an
/// estimate that errs slightly high rather than an exact row count.
#[must_use]
pub fn chunk_height(chunk: &ReviewChunk, changes: &HashMap<&String, &Change>) -> usize {
    HeightBreakdown::of(chunk, changes).total()
}

/// Where a chunk's rows go, so an author over budget knows what to cut: prose,
/// source lines, or the extra rows long source lines take when they wrap.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HeightBreakdown {
    /// Headings, blanks, source headers, and elision markers: the sum of
    /// `fixed`, `frames`, `elisions`, and `closings`.
    pub chrome: usize,
    /// The 8 rows every chunk costs, plus wrapped title rows.
    pub fixed: usize,
    /// Source headers and the blanks around each run of views of one file.
    pub frames: usize,
    /// "… lines not shown" markers between and around views.
    pub elisions: usize,
    /// The blank row after each narrative block and confidence note.
    pub closings: usize,
    /// Summary, narrative, and confidence text as wrapped.
    pub prose: usize,
    /// One row per quoted source line.
    pub source: usize,
    /// Rows added by source lines too wide for the reference width.
    pub wrapping: usize,
}

impl HeightBreakdown {
    #[must_use]
    pub fn total(&self) -> usize {
        self.chrome + self.prose + self.source + self.wrapping
    }

    #[must_use]
    pub fn of(chunk: &ReviewChunk, changes: &HashMap<&String, &Change>) -> Self {
        let mut parts = Self {
            fixed: CHUNK_CHROME + wrapped_rows(&chunk.title, REFERENCE_WIDTH).saturating_sub(1),
            prose: prose_height(&chunk.summary, REFERENCE_WIDTH),
            ..Self::default()
        };
        if let Some(confidence) = &chunk.confidence {
            parts.closings += CONFIDENCE_CHROME;
            parts.prose += prose_height(&format!("*Confidence:* {confidence}"), REFERENCE_WIDTH);
        }
        // The renderer sizes one gutter for the whole chunk, so every view
        // loses the same columns to it.
        let indent = chunk
            .quotes()
            .filter_map(|quote| changes.get(&quote.change_id))
            .map(|change| quote_indent(change))
            .max()
            .unwrap_or(4);
        for block in &chunk.content {
            match block {
                ChunkBlock::Narrative { markdown } => {
                    parts.prose += prose_height(markdown, REFERENCE_WIDTH);
                    parts.closings += 1;
                }
                // Measured separately (`explainer_height`), like a context
                // pane: a stepper beside the quote it explains must fit.
                ChunkBlock::Explainer(_) => {}
                ChunkBlock::Changes {
                    quotes,
                    presentation,
                } => {
                    let resolved: Vec<_> = quotes
                        .iter()
                        .filter_map(|quote| {
                            changes.get(&quote.change_id).map(|change| (quote, *change))
                        })
                        .collect();
                    for (quote, change) in &resolved {
                        let rows = quoted_height(quote, change, *presentation, indent);
                        let lines = match presentation {
                            ChangePresentation::Diff | ChangePresentation::AddedCode => {
                                quote.lines()
                            }
                            _ => rows,
                        };
                        parts.source += lines.min(rows);
                        parts.wrapping += rows.saturating_sub(lines);
                    }
                    // Elision markers make omitted material and separate views
                    // explicit. Each file run can carry one at either edge, and
                    // every later view carries one before it.
                    let elisions =
                        resolved
                            .chunk_by(|left, right| left.1.path == right.1.path)
                            .map(|run| {
                                usize::from(run[0].0.start > 1)
                                    + run.len().saturating_sub(1)
                                    + usize::from(run.last().is_some_and(|(quote, change)| {
                                        quote.end < change.body_lines()
                                    }))
                            })
                            .sum::<usize>();
                    let files: Vec<_> = resolved.iter().map(|(_, change)| *change).collect();
                    parts.frames += BLOCK_CHROME * file_runs(&files).max(1)
                        + header_rows(&resolved, *presentation);
                    parts.elisions += elisions;
                }
            }
        }
        parts.chrome = parts.fixed + parts.frames + parts.elisions + parts.closings;
        parts
    }
}

/// Rows the chunk's explainers take, reported beside the budget rather than
/// charged to it. Found by dogfooding: charged, a stepper could never sit
/// beside the quote it explains.
#[must_use]
pub fn explainer_height(chunk: &ReviewChunk) -> usize {
    chunk
        .content
        .iter()
        .filter_map(|block| match block {
            ChunkBlock::Explainer(explainer) => Some(explainer),
            _ => None,
        })
        .map(|explainer| {
            1 + explainer.rows()
                + explainer
                    .prose()
                    .iter()
                    .map(|prose| prose_height(prose, REFERENCE_WIDTH))
                    .max()
                    .unwrap_or(0)
        })
        .sum()
}

/// The rows a context pane's own text takes, without the headings and blanks
/// every pane has. Found by dogfooding: a refactor pane's fixed rows alone
/// came near half the budget, so a warning on the whole pane never cleared.
#[must_use]
pub fn pane_text_height(chunk: &ReviewChunk) -> usize {
    let Some(context) = &chunk.context else {
        return 0;
    };
    let fields = match context {
        ChunkContext::Concept {
            definition,
            example,
            relation,
        } => [definition, example, relation],
        ChunkContext::Refactor {
            before,
            after,
            reason,
        } => [before, after, reason],
    };
    fields
        .iter()
        .map(|field| prose_height(field, REFERENCE_WIDTH))
        .sum()
}

/// What a chunk's `context` pane costs, measured apart from the review work it
/// sits beside. Outside the budget, but not invisible: at the reference width
/// the pane is drawn below the chunk rather than next to it, so a reviewer still
/// pays for a long one in scrolling.
#[must_use]
pub fn pane_height(chunk: &ReviewChunk) -> usize {
    chunk
        .context
        .as_ref()
        .map_or(0, |context| CONTEXT_CHROME + context_height(context))
}

/// A context pane renders as headed sections of prose, and at the reference
/// width it sits below the chunk rather than beside it.
fn context_height(context: &ChunkContext) -> usize {
    let fields = match context {
        ChunkContext::Concept {
            definition,
            example,
            relation,
        } => [definition, example, relation],
        ChunkContext::Refactor {
            before,
            after,
            reason,
        } => [before, after, reason],
    };
    // A refactor pane carries one heading more than a concept pane.
    let extra = match context {
        ChunkContext::Refactor { .. } => 2,
        ChunkContext::Concept { .. } => 0,
    };
    // Each field is drawn under its own heading: the heading row, the blank
    // under it, and the blank that closes the field.
    extra
        + fields
            .iter()
            .map(|field| prose_height(field, REFERENCE_WIDTH) + 3)
            .sum::<usize>()
}

/// Rows the source headers cost beyond the one row `BLOCK_CHROME` already
/// allows for each. A header names the path and the presentation, and a long
/// path wraps.
fn header_rows(quotes: &[(&Quote, &Change)], presentation: ChangePresentation) -> usize {
    let mut rows = 0;
    for run in quotes.chunk_by(|left, right| left.1.path == right.1.path) {
        let path = &run[0].1.path;
        let ranges = run
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
        let location = if ranges.is_empty() {
            path.clone()
        } else {
            format!("{path}:{ranges}")
        };
        let presentation = match presentation {
            ChangePresentation::Diff => "diff",
            ChangePresentation::AddedCode => "added code",
            ChangePresentation::RenderedMarkdown => "document",
            ChangePresentation::SummaryOnly => "not shown",
        };
        let views = if run.len() > 1 {
            format!(" · {} views", run.len())
        } else {
            String::new()
        };
        rows += wrapped_rows(
            &format!("  {location} · {presentation}{views}"),
            REFERENCE_WIDTH,
        ) - 1;
    }
    rows
}

/// The renderer emits one header per run of same-file changes, so a block
/// spanning three files costs three headers, not one.
fn file_runs(changes: &[&Change]) -> usize {
    changes
        .windows(2)
        .filter(|pair| pair[0].path != pair[1].path)
        .count()
        + usize::from(!changes.is_empty())
}

/// Rendered Markdown wraps to the terminal and puts a blank line after every
/// heading, so prose occupies more rows than it has source lines. A blank
/// source line is drawn as one row, not two, and a fence marker is not drawn
/// at all.
fn prose_height(markdown: &str, width: usize) -> usize {
    let mut rows = 0;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            continue;
        }
        if trimmed.is_empty() {
            rows += 1;
            continue;
        }
        rows += wrapped_rows(line, width);
        if trimmed.starts_with('#') {
            rows += 1;
        }
    }
    rows.max(1)
}

fn wrapped_rows(text: &str, width: usize) -> usize {
    let indent: usize = text
        .chars()
        .take_while(|character| character.is_whitespace())
        .map(|character| character.width().unwrap_or(0))
        .sum();
    let width = width.max(1);
    let continuation = width.saturating_sub(indent).max(1);
    let mut rows = 1;
    let mut used = 0;
    let mut word = 0;
    for character in text.chars() {
        let size = character.width().unwrap_or(0);
        let limit = if rows == 1 { width } else { continuation };
        if character == ' ' {
            used += word + size;
            word = 0;
            if used > limit {
                rows += 1;
                used = 0;
            }
            continue;
        }
        word += size;
        if used + word > limit {
            if used > 0 {
                rows += 1;
                used = 0;
            }
            // A word longer than the line is cut rather than lost.
            while word > continuation {
                rows += 1;
                word -= continuation;
            }
        }
    }
    if used + word > if rows == 1 { width } else { continuation } {
        rows += 1;
    }
    rows
}

/// The added lines without their diff markers, which is what the renderer draws
/// for Markdown and therefore what that estimate must measure.
fn added_text(body: &str) -> String {
    body.lines()
        .filter_map(|line| line.strip_prefix('+').or_else(|| line.strip_prefix(' ')))
        .collect::<Vec<_>>()
        .join("\n")
}

fn quoted_height(
    quote: &Quote,
    change: &Change,
    presentation: ChangePresentation,
    indent: usize,
) -> usize {
    let width = REFERENCE_WIDTH.saturating_sub(indent).max(1);
    let body = change.body(quote.start, quote.end);
    let rows = |strip_marker: bool| {
        body.iter()
            .map(|line| {
                let drawn = if strip_marker {
                    line.get(1..).unwrap_or(line)
                } else {
                    line
                };
                wrapped_rows(drawn, width)
            })
            .sum()
    };
    match presentation {
        // Generated artifacts collapse to a single withheld-lines notice.
        ChangePresentation::SummaryOnly => 1,
        // A diff view keeps its markers.
        ChangePresentation::Diff => rows(false),
        // Markers are dropped and only the source is shown.
        ChangePresentation::AddedCode => rows(true),
        // Markdown is rendered, so it inflates the way prose does. A unit cut
        // mid-block still ends with the blank row that closes the block, even
        // though its source does not carry one.
        ChangePresentation::RenderedMarkdown => {
            prose_height(&added_text(&body.join("\n")), width) + 1
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{Change, ChangePresentation, ChunkBlock, ChunkKind, Quote, ReviewChunk};

    use super::{BLOCK_CHROME, CHUNK_CHROME, CHUNK_HEIGHT_GUIDANCE, chunk_height};

    /// The breakdown must add up to what validation charges, or an author
    /// trimming by it would cut the wrong thing.
    #[test]
    fn the_breakdown_sums_to_the_height_and_names_wrapping() {
        let long = format!("@@ -0,0 +1,2 @@\n+{}\n+short\n", "x".repeat(150));
        let change = change("a.rs", &long, 2);
        let chunk = chunk(vec![ChunkBlock::Changes {
            quotes: vec![Quote::whole(change.id.clone(), 2)],
            presentation: ChangePresentation::AddedCode,
        }]);
        let known = HashMap::from([(&change.id, &change)]);
        let parts = super::HeightBreakdown::of(&chunk, &known);
        assert_eq!(parts.total(), chunk_height(&chunk, &known));
        assert_eq!(parts.source, 2);
        assert!(parts.wrapping >= 1, "{parts:?}");
    }

    /// Prompt drift once made authors undercount every rendered document.
    #[test]
    fn public_guidance_names_the_rendered_markdown_closing_row() {
        assert!(CHUNK_HEIGHT_GUIDANCE.contains("rendered rows plus 1 closing blank row"));
    }

    /// The fixtures' one-line summary, which is measured rather than assumed.
    const SUMMARY: usize = 1;

    fn change(path: &str, patch: &str, lines: u32) -> Change {
        Change::new(path, (0, 0, 1, lines), patch)
    }

    fn chunk(content: Vec<ChunkBlock>) -> ReviewChunk {
        ReviewChunk {
            id: "chunk".into(),
            title: "Title".into(),
            summary: "Summary.".into(),
            kind: ChunkKind::Other,
            content,
            context: None,
            confidence: None,
            oversized_reason: None,
            section: None,
        }
    }

    #[test]
    fn counts_block_chrome_and_quoted_lines() {
        let change = change("a.rs", "@@ -0,0 +1,3 @@\n+one\n+two\n+three\n", 3);
        let chunk = chunk(vec![ChunkBlock::Changes {
            quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
            presentation: ChangePresentation::AddedCode,
        }]);

        let height = chunk_height(&chunk, &HashMap::from([(&change.id, &change)]));

        assert_eq!(height, CHUNK_CHROME + SUMMARY + BLOCK_CHROME + 3);
    }

    #[test]
    fn charges_a_header_for_every_file_a_block_spans() {
        let first = change("a.rs", "@@ -0,0 +1,1 @@\n+one\n", 1);
        let second = change("b.rs", "@@ -0,0 +1,1 @@\n+two\n", 1);
        let chunk = chunk(vec![ChunkBlock::Changes {
            quotes: vec![
                Quote::whole(first.id.clone(), 1),
                Quote::whole(second.id.clone(), 1),
            ],
            presentation: ChangePresentation::AddedCode,
        }]);
        let changes = HashMap::from([(&first.id, &first), (&second.id, &second)]);

        let height = chunk_height(&chunk, &changes);

        // Two file runs and two quoted lines. Each run's closing blank already
        // separates it from the next header.
        assert_eq!(height, CHUNK_CHROME + SUMMARY + BLOCK_CHROME * 2 + 2);
    }

    #[test]
    fn charges_for_the_summary_it_will_actually_draw() {
        let brief = chunk(vec![ChunkBlock::Narrative {
            markdown: "One line.".into(),
        }]);
        let mut wordy = brief.clone();
        wordy.summary = "word ".repeat(100);

        let grew = chunk_height(&wordy, &HashMap::new()) - chunk_height(&brief, &HashMap::new());

        // 500 characters is seven rows at the reference width, not one.
        assert!(grew >= 5, "a long summary only cost {grew} extra rows");
    }

    #[test]
    fn charges_for_a_title_that_wraps() {
        let brief = chunk(vec![ChunkBlock::Narrative {
            markdown: "One line.".into(),
        }]);
        let mut long = brief.clone();
        long.title = "title ".repeat(40);

        let grew = chunk_height(&long, &HashMap::new()) - chunk_height(&brief, &HashMap::new());

        assert!(grew >= 2, "a long title only cost {grew} extra rows");
    }

    #[test]
    fn charges_for_a_confidence_note_by_its_length() {
        let brief = chunk(vec![ChunkBlock::Narrative {
            markdown: "One line.".into(),
        }]);
        let bare = chunk_height(&brief, &HashMap::new());
        let mut short = brief.clone();
        short.confidence = Some("Unsure about the motivation.".into());
        let mut wordy = brief.clone();
        wordy.confidence = Some("Unsure about the motivation. ".repeat(12));

        let short_height = chunk_height(&short, &HashMap::new());
        let wordy_height = chunk_height(&wordy, &HashMap::new());

        assert_eq!(short_height, bare + 2, "one row of note plus its blank");
        // A note that wraps costs what it draws, not a flat allowance.
        assert!(wordy_height >= short_height + 3, "{wordy_height}");
    }

    #[test]
    fn a_withheld_artifact_costs_one_row() {
        let body: String = (0..500).map(|index| format!("+{index}\n")).collect();
        let change = change("Cargo.lock", &format!("@@ -0,0 +1,500 @@\n{body}"), 500);
        let chunk = chunk(vec![ChunkBlock::Changes {
            quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
            presentation: ChangePresentation::SummaryOnly,
        }]);

        let height = chunk_height(&chunk, &HashMap::from([(&change.id, &change)]));

        assert_eq!(height, CHUNK_CHROME + SUMMARY + BLOCK_CHROME + 1);
    }

    #[test]
    fn a_line_wider_than_the_terminal_costs_more_than_one_row() {
        let long = "+".to_owned() + &"x".repeat(200);
        let change = change("a.rs", &format!("@@ -0,0 +1,1 @@\n{long}\n"), 1);
        let chunk = chunk(vec![ChunkBlock::Changes {
            quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
            presentation: ChangePresentation::AddedCode,
        }]);

        let height = chunk_height(&chunk, &HashMap::from([(&change.id, &change)]));

        assert!(
            height > CHUNK_CHROME + SUMMARY + BLOCK_CHROME + 1,
            "{height}"
        );
    }
}
