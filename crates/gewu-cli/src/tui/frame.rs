//! One composed screen, and a headless walk through every screen a reviewer
//! can page to, so the terminal UI can be checked without a terminal.

use serde_json::{Value, json};

use super::{
    ReviewTui,
    style::{Document, Line},
    view::{is_content_free, is_heading, is_source_header, line_text, page_step_for},
};

/// What `draw` prints, laid out for one terminal size.
pub(super) struct Frame {
    pub(super) width: usize,
    pub(super) pinned: Option<Document>,
    pub(super) body_top: usize,
    /// The rows of the body that are visible, beside any context column.
    pub(super) body: Document,
    pub(super) body_bottom: usize,
    pub(super) footer: Option<Document>,
    pub(super) panel: Option<Document>,
    pub(super) scroll: usize,
    pub(super) visible_height: usize,
    /// Rows the current document wraps to, visible or not.
    pub(super) total: usize,
    /// Display widths of rows that `ansi_window` will clip, hiding their end.
    pub(super) overflow: Vec<usize>,
}

impl Frame {
    pub(super) fn overflowing(document: &Document, width: usize) -> Vec<usize> {
        document
            .lines
            .iter()
            .map(Line::width)
            .filter(|row| *row > width)
            .collect()
    }

    /// The screen as plain text, one string per terminal row.
    fn screen(&self) -> Vec<String> {
        let clip = |line: &Line| line_text(&line.clipped(self.width));
        let mut rows: Vec<String> = Vec::new();
        rows.extend(self.pinned.iter().flat_map(|d| d.lines.iter().map(clip)));
        rows.extend(self.body.lines.iter().take(self.visible_height).map(clip));
        rows.resize(self.body_bottom, String::new());
        rows.extend(self.footer.iter().flat_map(|d| d.lines.iter().map(clip)));
        rows
    }

    /// The page leaves a source label or heading as its last row, with what
    /// it introduces pushed to the next page.
    fn orphaned(&self) -> bool {
        let shown = self.visible_height.min(self.body.lines.len());
        let more = self.scroll + shown < self.total;
        // The last row with anything on it: a heading followed only by blank
        // rows is as orphaned as one on the last row.
        more && self.body.lines[..shown]
            .iter()
            .rev()
            .find(|line| !is_content_free(line))
            .is_some_and(|last| is_source_header(last) || is_heading(last))
    }

    fn cued(&self) -> bool {
        self.footer
            .iter()
            .flat_map(|footer| &footer.lines)
            .any(|line| line_text(line).contains(" · view "))
    }
}

impl ReviewTui {
    /// Every page of the overview, each chunk, and the summary at one size,
    /// reached by the same PageDown step the reviewer has, with the defects a
    /// machine can see: clipped rows, rows PageDown skips, scrolling without a
    /// position cue, and pages ending on an orphaned label.
    pub fn frames(&mut self, width: usize, height: usize) -> Value {
        let chunks: Vec<_> = self
            .review
            .spec()
            .chunks
            .iter()
            .map(|c| c.id.clone())
            .collect();
        let names = std::iter::once("overview".to_owned())
            .chain(chunks)
            .chain(std::iter::once("summary".to_owned()));
        let stages: Vec<_> = names
            .enumerate()
            .map(|(stage, name)| {
                self.stage = stage;
                self.leave_chunk();
                self.stage_frames(&name, width, height)
            })
            .collect();
        let passes = stages.iter().all(|stage| stage["passes"] == true);
        json!({ "columns": width, "rows": height, "passes": passes, "stages": stages })
    }

    fn stage_frames(&mut self, name: &str, width: usize, height: usize) -> Value {
        let (mut pages, mut seen) = (Vec::new(), Vec::new());
        let (mut clipped, mut uncued, mut orphaned) = (Vec::new(), Vec::new(), Vec::new());
        let mut stubs = Vec::new();
        let mut total;
        loop {
            let frame = self.compose(width, height);
            let page = pages.len();
            total = frame.total;
            seen.push((frame.scroll, frame.scroll + frame.visible_height));
            if !frame.overflow.is_empty() {
                clipped.push(json!({ "page": page, "widths": frame.overflow }));
            }
            if frame.total > frame.visible_height && !frame.cued() {
                uncued.push(page);
            }
            if frame.orphaned() {
                orphaned.push(page);
            }
            // Rows the previous page already showed: a near-duplicate page
            // is one that repeats most of the last.
            let repeated = seen
                .iter()
                .rev()
                .nth(1)
                .map_or(0, |&(_, end): &(usize, usize)| {
                    end.saturating_sub(frame.scroll)
                });
            // A page after the first that shows almost nothing new: the reader
            // turns it for a row or two (a UI judge found pages of one row).
            let fresh = frame
                .visible_height
                .min(frame.total.saturating_sub(frame.scroll))
                .saturating_sub(repeated);
            if page > 0 && fresh <= 2 && frame.scroll + frame.visible_height < frame.total {
                stubs.push(page);
            }
            pages.push(json!({
                "repeated_rows": repeated,
                "first_row": frame.scroll + 1,
                "last_row": frame.scroll + frame.visible_height.min(frame.total),
                "screen": frame.screen(),
            }));
            let before = frame.scroll;
            let at_end = self.showing_last_content();
            self.paged_past_end = true;
            self.scroll = before.saturating_add(self.bounded_step(page_step_for(height)));
            if frame.scroll + frame.visible_height >= frame.total || at_end || pages.len() > 500 {
                break;
            }
            if self.compose(width, height).scroll == before {
                break;
            }
        }
        // Rows after the last content row (a closing elision marker, blank
        // padding) are deliberately not paged to, so they are not owed.
        let owed = self
            .last_content_row
            .map_or(total, |last| (last + 1).min(total));
        let unreached = Self::unreached(&seen, owed);
        let passes =
            clipped.is_empty() && unreached.is_empty() && uncued.is_empty() && stubs.is_empty();
        json!({
            "stage": name, "total_rows": total, "passes": passes, "pages": pages,
            "clipped": clipped, "unreached": unreached, "uncued_pages": uncued,
            "orphaned_pages": orphaned, "stub_pages": stubs,
        })
    }

    /// Row ranges, 1-based and inclusive, that no visited page showed.
    fn unreached(seen: &[(usize, usize)], total: usize) -> Vec<[usize; 2]> {
        let mut shown = vec![false; total];
        for &(start, end) in seen {
            for row in shown.iter_mut().take(end.min(total)).skip(start) {
                *row = true;
            }
        }
        let mut ranges: Vec<[usize; 2]> = Vec::new();
        for (index, _) in shown.iter().enumerate().filter(|(_, shown)| !**shown) {
            match ranges.last_mut() {
                Some(range) if range[1] == index => range[1] = index + 1,
                _ => ranges.push([index + 1, index + 1]),
            }
        }
        ranges
    }
}

#[cfg(test)]
mod tests {
    use gewu_review::{
        Change, ChangePresentation, ChunkBlock, ChunkKind, Overview, Quote, ReviewChunk,
        ReviewInput, ReviewSpec, SourceIdentity, ValidatedReview,
    };

    use super::{Document, Frame, ReviewTui};
    use crate::tui::style::TextStyle;

    /// One chunk far taller than any terminal, as `oversized_reason` allows.
    fn oversized() -> ValidatedReview {
        let source = SourceIdentity::new("base", "head", b"diff");
        let body: String = (1..=120)
            .map(|line| format!("+let value_{line} = {line};\n"))
            .collect();
        let change = Change::new("a.rs", (0, 0, 1, 120), format!("@@ -0,0 +1,120 @@\n{body}"));
        let chunk = ReviewChunk {
            id: "whole-test".into(),
            title: "One indivisible test".into(),
            summary: "Declares one hundred and twenty values.".into(),
            kind: ChunkKind::Test,
            content: vec![ChunkBlock::Changes {
                quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
                presentation: ChangePresentation::AddedCode,
            }],
            context: None,
            confidence: None,
            oversized_reason: Some("one test function read as a unit".into()),
            section: None,
        };
        let overview = Overview {
            intent: "Intent.".into(),
            changes: vec![],
            illustration: None,
            concepts: vec![],
        };
        let spec = ReviewSpec::new(source.clone(), overview, vec![chunk]);
        ValidatedReview::validate(ReviewInput::new(source, vec![change]), spec).expect("valid")
    }

    #[test]
    fn paging_an_oversized_chunk_reaches_every_row_with_a_position_cue() {
        let directory = tempfile::tempdir().expect("session directory");
        let mut tui =
            ReviewTui::new(oversized(), directory.path().join("events.jsonl")).expect("tui");
        for (width, height) in [(80, 24), (120, 40)] {
            let report = tui.frames(width, height);
            let chunk = &report["stages"][1];
            assert_eq!(chunk["stage"], "whole-test");
            assert!(
                chunk["pages"].as_array().expect("pages").len() > 2,
                "{chunk}"
            );
            assert_eq!(chunk["unreached"], serde_json::json!([]), "{chunk}");
            assert_eq!(chunk["uncued_pages"], serde_json::json!([]), "{chunk}");
            assert_eq!(report["passes"], true);
        }
    }

    /// A page shortened to keep a source label with its code must not let
    /// PageDown skip the label. Found by self-review at 80x24; the fixture is
    /// the chunk it was found in, because the geometry is easy to miss.
    #[test]
    fn paging_never_skips_a_label_held_back_from_the_previous_page() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/held-back-label.json")).expect("fixture");
        let input = serde_json::from_value(fixture["input"].clone()).expect("input");
        let spec = serde_json::from_value(fixture["spec"].clone()).expect("spec");
        let review = ValidatedReview::validate_partial(input, spec).expect("valid");
        let directory = tempfile::tempdir().expect("session directory");
        let mut tui = ReviewTui::new(review, directory.path().join("events.jsonl")).expect("tui");
        let report = tui.frames(80, 24);
        let stage = &report["stages"][1];
        assert_eq!(stage["unreached"], serde_json::json!([]), "{stage}");
    }

    /// The UI judge found prose on one page and the code it introduces on the
    /// next at 80x24. The page that opens on code pins the paragraph's first
    /// sentence, and the extra header row must not put the last row out of
    /// reach at any size.
    #[test]
    fn a_page_that_opens_on_code_pins_the_sentence_that_introduces_it() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/held-back-label.json")).expect("fixture");
        let input = serde_json::from_value(fixture["input"].clone()).expect("input");
        let spec = serde_json::from_value(fixture["spec"].clone()).expect("spec");
        let review = ValidatedReview::validate_partial(input, spec).expect("valid");
        let directory = tempfile::tempdir().expect("session directory");
        let mut tui = ReviewTui::new(review, directory.path().join("events.jsonl")).expect("tui");
        let report = tui.frames(80, 24);
        let pages = report["stages"][1]["pages"].as_array().expect("pages");
        let first = pages[0]["screen"].to_string();
        let second = pages[1]["screen"].to_string();
        assert!(
            !first.contains("↳ "),
            "the first page shows the prose itself"
        );
        // The paragraph's closing question, not its opening instruction.
        assert!(
            second.contains("↳ Once the final item is appended"),
            "{second}"
        );
        assert!(second.contains("leaving the last batch"), "{second}");
        // Whole, and its wrapped rows indented under the text after `↳`.
        assert!(
            second.contains("\"  unread?\""),
            "the question is whole and indented: {second}"
        );
        assert!(!second.contains("What this does"), "{second}");
        for (width, height) in [(80, 20), (80, 24), (100, 30), (120, 40)] {
            let report = tui.frames(width, height);
            assert_eq!(report["passes"], true, "{width}x{height}");
        }
    }

    /// The last PageDown used to snap back to a full page, repeating all but
    /// a row or two of the one before.
    #[test]
    fn the_last_page_does_not_repeat_the_one_before() {
        let directory = tempfile::tempdir().expect("session directory");
        let mut tui =
            ReviewTui::new(oversized(), directory.path().join("events.jsonl")).expect("tui");
        for (width, height) in [(80, 24), (120, 40)] {
            let report = tui.frames(width, height);
            for stage in report["stages"].as_array().expect("stages") {
                for page in &stage["pages"].as_array().expect("pages")[1..] {
                    assert!(page["repeated_rows"].as_u64() <= Some(1), "{page}");
                }
            }
            assert_eq!(report["passes"], true);
        }
    }

    /// Found by self-review's UI judge: a context pane's title ended a page
    /// alone, followed by one blank row, and the cue promised rows PageDown
    /// never shows.
    #[test]
    fn a_context_title_never_ends_a_page_and_the_cue_counts_only_content() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/context-heading.json")).expect("fixture");
        let input = serde_json::from_value(fixture["input"].clone()).expect("input");
        let spec = serde_json::from_value(fixture["spec"].clone()).expect("spec");
        let review = ValidatedReview::validate_partial(input, spec).expect("valid");
        let directory = tempfile::tempdir().expect("session directory");
        let mut tui = ReviewTui::new(review, directory.path().join("events.jsonl")).expect("tui");
        for (width, height) in [(80, 20), (80, 24), (120, 40)] {
            let report = tui.frames(width, height);
            let stage = &report["stages"][1];
            assert_eq!(
                stage["orphaned_pages"],
                serde_json::json!([]),
                "{width}x{height}"
            );
            for page in stage["pages"].as_array().expect("pages") {
                let rows: Vec<_> = page["screen"]
                    .as_array()
                    .expect("screen")
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect();
                let cue = rows.last().expect("cue");
                let last_page = page["last_row"]
                    == stage["pages"]
                        .as_array()
                        .expect("pages")
                        .last()
                        .expect("page")["last_row"];
                if last_page && cue.contains(" · view ") {
                    let (shown, total) = cue.rsplit_once(" / ").expect("cue total");
                    assert!(
                        shown.ends_with(total),
                        "the last page reaches the cue's total: {cue}"
                    );
                }
            }
        }
    }

    /// Found by self-review's UI judge: a context pane's definition on one
    /// page and its example on an otherwise empty next page.
    #[test]
    fn a_context_pane_that_fits_on_the_next_page_is_not_split() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/context-heading.json")).expect("fixture");
        let input = serde_json::from_value(fixture["input"].clone()).expect("input");
        let spec = serde_json::from_value(fixture["spec"].clone()).expect("spec");
        let review = ValidatedReview::validate_partial(input, spec).expect("valid");
        let directory = tempfile::tempdir().expect("session directory");
        let mut tui = ReviewTui::new(review, directory.path().join("events.jsonl")).expect("tui");
        let report = tui.frames(120, 40);
        let stage = &report["stages"][1];
        let pages: Vec<String> = stage["pages"]
            .as_array()
            .expect("pages")
            .iter()
            .map(|page| page["screen"].to_string())
            .collect();
        let titled = pages
            .iter()
            .position(|page| page.contains("Context · New concept"));
        if let Some(first) = titled {
            assert!(
                pages[first].contains("Example"),
                "the pane's title and its example share a page: {}",
                pages[first]
            );
        }
        assert_eq!(report["passes"], true);
    }

    #[test]
    fn unreached_rows_are_reported_as_ranges() {
        assert_eq!(
            ReviewTui::unreached(&[(0, 3), (5, 8)], 10),
            [[4, 5], [9, 10]]
        );
        assert!(ReviewTui::unreached(&[(0, 4), (2, 10)], 10).is_empty());
    }

    #[test]
    fn rows_wider_than_the_screen_are_overflow() {
        let mut document = Document::new();
        document.push("x".repeat(90), TextStyle::default());
        assert_eq!(Frame::overflowing(&document, 80), [90]);
        assert!(Frame::overflowing(&document.wrapped(80), 80).is_empty());
    }
}
