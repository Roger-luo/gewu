use std::io::{self, Write};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print},
    terminal::{Clear, ClearType, size},
};

use gewu_review::{FeedbackKind, FeedbackTarget};

use super::{
    ReviewTui,
    frame::Frame,
    help::HelpScreen,
    render::Screen,
    render_support::{SIDE_PANE_MIN_WIDTH, SOURCE_GUTTER, muted},
    style::{Document, TextStyle},
};

impl ReviewTui {
    pub(super) fn draw(&mut self) -> Result<()> {
        let (width, height) = size()?;
        let frame = self.compose(usize::from(width), usize::from(height));
        execute!(io::stdout(), MoveTo(0, 0), Clear(ClearType::All))?;
        if let Some(pinned) = &frame.pinned {
            execute!(
                io::stdout(),
                MoveTo(0, 0),
                Print(pinned.ansi_window(frame.width, pinned.lines.len(), 0))
            )?;
        }
        execute!(
            io::stdout(),
            MoveTo(0, u16::try_from(frame.body_top).unwrap_or(u16::MAX)),
            Print(frame.body.ansi_window(frame.width, frame.visible_height, 0))
        )?;
        match (&frame.panel, &frame.footer) {
            (Some(panel), _) => self.draw_prompt(
                panel,
                u16::try_from(frame.width).unwrap_or(u16::MAX),
                u16::try_from(frame.body_bottom).unwrap_or(u16::MAX),
            )?,
            (None, Some(footer)) => execute!(
                io::stdout(),
                MoveTo(0, u16::try_from(frame.body_bottom).unwrap_or(u16::MAX)),
                Print(footer.ansi_window(frame.width, footer.lines.len(), 0))
            )?,
            (None, None) => {}
        }
        io::stdout().flush()?;
        Ok(())
    }

    /// Lays out one screen without touching the terminal, so the same frame
    /// the reviewer sees can be rendered headlessly and checked.
    pub(super) fn compose(&mut self, width: usize, height: usize) -> Frame {
        let panel = self.prompt_panel();
        let sidebar = self.sidebar_document(width);
        let content_width = if sidebar.is_some() {
            width.saturating_mul(2) / 3
        } else {
            width
        };
        let content = self.current_document(width).wrapped(content_width);
        let command_rows = self.footer_command_document(width).lines.len();
        // The pinned header depends on the scroll position (its lead row), and
        // the scroll limit depends on the header's height, so settle the two
        // together; otherwise the last page is sized for a header it lacks.
        let layout = |tui: &Self| {
            let pinned = tui
                .pinned_document(width)
                .map(|document| document.wrapped(width));
            let pinned_rows = pinned.as_ref().map_or(0, |document| document.lines.len());
            let footer_rows = if let Some(panel) = &panel {
                panel.lines.len()
            } else {
                let room_without_status = height.saturating_sub(command_rows + pinned_rows);
                command_rows
                    + usize::from(tui.notice.is_some() || content.lines.len() > room_without_status)
            };
            let body_top = pinned_rows.min(height);
            let body_bottom = height.saturating_sub(footer_rows).max(body_top);
            (pinned, body_top, body_bottom)
        };
        // An elision marker is not content: a page holding only "… source
        // continues" gives the reader nothing.
        let last_content = content
            .lines
            .iter()
            .rposition(|line| !is_content_free(line))
            .unwrap_or(0);
        self.last_content_row = Some(last_content);
        let (mut pinned, mut body_top, mut body_bottom) = layout(self);
        for _ in 0..3 {
            // Past the end, but never onto a page of nothing: the last
            // non-blank row bounds how far PageDown may go.
            let full = content
                .lines
                .len()
                .saturating_sub(body_bottom.saturating_sub(body_top));
            // Past the end, but never onto a page of nothing: the last
            // non-blank row bounds how far PageDown may go.
            let max_scroll = if self.paged_past_end {
                last_content.max(full)
            } else {
                full
            };
            if self.scroll <= max_scroll {
                break;
            }
            self.scroll = max_scroll;
            (pinned, body_top, body_bottom) = layout(self);
        }
        let body_height = body_bottom.saturating_sub(body_top);
        let mut visible_height = if self.show_help {
            help_page_height(&content, body_height, self.scroll)
        } else {
            body_height
        };
        visible_height = avoid_orphaned_source_header(&content, visible_height, self.scroll);
        if !self.show_help {
            visible_height = avoid_split_paragraph(&content, visible_height, self.scroll);
        }
        self.shown_rows = Some(visible_height);
        let window = Document {
            lines: content
                .lines
                .iter()
                .skip(self.scroll)
                .take(visible_height)
                .cloned()
                .collect(),
        };
        let mut overflow = Frame::overflowing(&window, content_width);
        let body = match sidebar {
            Some(sidebar) => {
                let right_width = width.saturating_sub(content_width + 3);
                let sidebar = sidebar.wrapped(right_width);
                overflow.extend(Frame::overflowing(&sidebar, right_width));
                Document::columns(window, sidebar, width)
            }
            None => window,
        };
        let footer = panel
            .is_none()
            .then(|| self.footer_document(width, content.lines.len(), visible_height));
        Frame {
            width,
            pinned,
            body_top,
            body,
            body_bottom,
            footer,
            panel,
            scroll: self.scroll,
            visible_height,
            total: content.lines.len(),
            overflow,
        }
    }

    pub(super) fn page_step(&self) -> usize {
        let step = size()
            .map(|(_, height)| page_step_for(usize::from(height)))
            .unwrap_or(10);
        self.bounded_step(step)
    }

    /// Self-review's frame check found a row PageDown never showed: the page
    /// had been shortened to keep a source label with its code, and the next
    /// page started a fixed step further on. Its UI judge then found pages
    /// without a pinned header repeating six rows, because that fixed step was
    /// sized for chunk pages. A page now advances by what it showed.
    /// Whether the page last drawn already shows the last row with content,
    /// so PageDown has nothing new to show.
    pub(super) fn showing_last_content(&self) -> bool {
        matches!(
            (self.last_content_row, self.shown_rows),
            (Some(last), Some(shown)) if self.scroll + shown > last
        )
    }

    pub(super) fn bounded_step(&self, step: usize) -> usize {
        // One row of overlap keeps the reader's place; more repeats the page.
        self.shown_rows
            .map_or(step, |shown| shown.saturating_sub(1).max(1))
    }

    /// Move help by a complete semantic page. Help is prose rather than source,
    /// so stopping before the next heading or bullet is more useful than
    /// filling the last row with half a sentence.
    pub(super) fn page_help(&mut self, forward: bool) {
        let (width, height) = size()
            .map(|(width, height)| (usize::from(width), usize::from(height)))
            .unwrap_or((80, 24));
        let footer_rows = self.footer_command_document(width).lines.len() + 1;
        let body_height = height.saturating_sub(footer_rows);
        let content = HelpScreen::document().wrapped(width);
        if forward {
            self.scroll = self
                .scroll
                .saturating_add(help_page_height(&content, body_height, self.scroll).max(1));
        } else {
            let target = self.scroll.saturating_sub(body_height);
            self.scroll = previous_help_boundary(&content, target);
        }
    }

    pub(super) fn scroll_to_end(&mut self) {
        let width = size().map_or(80, |(width, _)| usize::from(width));
        self.scroll = self
            .current_document(width)
            .wrapped(self.content_width(width))
            .lines
            .len();
    }

    pub(super) fn scroll_to_feedback(&mut self, feedback_id: &str) {
        let width = size().map_or(80, |(width, _)| usize::from(width));
        let Some(index) = self.chunk_index() else {
            return;
        };
        let document = self
            .chunk_document(index, width, Some(feedback_id))
            .wrapped(self.content_width(width));
        self.scroll = document
            .lines
            .iter()
            .position(|line| line_text(line).starts_with("▸ "))
            .unwrap_or_else(|| document.lines.len().saturating_sub(1));
    }

    pub(super) fn scroll_to_selected_feedback(&mut self) {
        let Some(id) = self.selected_feedback().map(|feedback| feedback.id.clone()) else {
            return;
        };
        self.scroll_to_feedback(&id);
    }

    pub(super) fn scroll_to_selected_line(&mut self) {
        let width = size().map_or(80, |(width, _)| usize::from(width));
        let document = self
            .current_document(width)
            .wrapped(self.content_width(width));
        self.scroll = document
            .lines
            .iter()
            .position(|line| line_text(line).contains('▶'))
            .unwrap_or(self.scroll);
    }

    pub(super) fn scroll_to_first_source(&mut self) {
        let width = size().map_or(80, |(width, _)| usize::from(width));
        let document = self
            .current_document(width)
            .wrapped(self.content_width(width));
        self.scroll = document
            .lines
            .iter()
            .position(is_source_header)
            .unwrap_or(self.scroll);
    }

    pub(super) fn scroll_to_context(&mut self) {
        let width = size().map_or(80, |(width, _)| usize::from(width));
        if width >= SIDE_PANE_MIN_WIDTH {
            self.notice = Some("Context is visible in the sidebar".into());
            return;
        }
        let document = self.current_document(width).wrapped(width);
        if let Some(row) = document.lines.iter().position(|line| {
            line_text(line)
                .to_ascii_lowercase()
                .starts_with("context ·")
        }) {
            self.scroll = row;
        }
    }

    pub(super) fn pinned_document(&self, width: usize) -> Option<Document> {
        if self.show_help || self.show_outline {
            return None;
        }
        let index = self.chunk_index()?;
        let mut document = Screen::chunk_header(&self.review, &self.session, index, width);
        if let Some(feedback) = self.selected_feedback() {
            document.newline();
            document.append(Screen::selected_feedback_header(feedback));
        }
        let chunk = &self.review.spec().chunks[index];
        if width < SIDE_PANE_MIN_WIDTH
            && let Some(context) = &chunk.context
        {
            let selected = self
                .selected_feedback()
                .map(|feedback| feedback.id.as_str());
            let main_rows = Screen::chunk_main(
                &self.review,
                &self.session,
                chunk,
                selected,
                self.show_raw_source,
                self.selected_line.as_ref(),
            )
            .wrapped(width)
            .lines
            .len();
            if self.scroll > main_rows {
                document.newline();
                document.push(
                    Screen::context_title(context),
                    TextStyle {
                        foreground: Some(Color::Cyan),
                        bold: true,
                        ..TextStyle::default()
                    },
                );
            }
        }
        if let Some(lead) = self.lead(width) {
            document.newline();
            document.push(lead, muted());
        }
        document.newline();
        Some(document)
    }

    /// The opening of the paragraph that introduces the code at the top of
    /// this page, when that paragraph has scrolled off. On a short terminal a
    /// chunk's prose and its code land on different pages; one pinned row
    /// keeps the reason for reading the code in view.
    fn lead(&self, width: usize) -> Option<String> {
        if self.scroll == 0 || self.show_raw_source || self.selected_feedback().is_some() {
            return None;
        }
        let content = self
            .current_document(width)
            .wrapped(self.content_width(width));
        let rows: Vec<String> = content.lines.iter().map(line_text).collect();
        let source = |row: &String| row.starts_with(SOURCE_GUTTER.trim_end());
        // A heading is not part of the paragraph under it: the judge found
        // "What this does" run into the pinned sentence.
        let heading = |index: usize| {
            let line = &content.lines[index];
            line.width() > 0 && line.spans.iter().all(|span| span.style.bold)
        };
        let prose_at = |index: usize| {
            !rows[index].trim().is_empty() && !source(&rows[index]) && !heading(index)
        };
        let top = self.scroll.min(rows.len().saturating_sub(1));
        let end = (0..self.scroll.min(rows.len()))
            .rev()
            .find(|&index| prose_at(index))?;
        // Code between that paragraph and the top of the page, the blank row
        // after a view included, is what the paragraph introduces. Deciding on
        // the top row alone would make the header height flicker at a view's
        // end and leave the last row out of reach.
        let opens_on_code = (top..rows.len())
            .find(|&index| !rows[index].trim().is_empty())
            .is_some_and(|index| source(&rows[index]));
        if !opens_on_code && !(end + 1..=top).any(|index| source(&rows[index])) {
            return None;
        }
        let start = (0..=end)
            .rev()
            .take_while(|&index| prose_at(index))
            .last()?;
        let paragraph = rows[start..=end]
            .iter()
            .map(|row| row.trim())
            .collect::<Vec<_>>()
            .join(" ");
        // The last sentence sits right above the code and is where prose says
        // what to look for or asks its question; two UI judges found the first
        // sentence pinned while that question scrolled away.
        let trimmed = paragraph.trim_end();
        let boundary_before = |end: usize| {
            [". ", "? ", "! "]
                .iter()
                .filter_map(|boundary| trimmed[..end].rfind(boundary).map(|at| at + boundary.len()))
                .max()
                .unwrap_or(0)
        };
        let mut start = boundary_before(trimmed.len());
        // A short closing question ("Should they change?") needs the
        // sentence before it to say what it is about.
        if trimmed[start..].split_whitespace().count() < 10 && start > 0 {
            start = boundary_before(start.saturating_sub(1));
        }
        let sentence = trimmed[start..].trim();
        // Three rows at most: one cut the question the reader was meant to
        // check, and two lost its end to word-wrapping slack.
        let limit = width.saturating_mul(3).saturating_sub(12);
        let mut lead = format!("↳ {sentence}");
        if lead.chars().count() > limit {
            // Cut at a word, not inside one ("ori…" read as a typo).
            let kept: String = lead.chars().take(limit.saturating_sub(1)).collect();
            let at = kept.rfind(' ').unwrap_or(kept.len());
            lead = kept[..at].trim_end().to_owned() + " …";
        }
        Some(lead)
    }

    pub(super) fn current_document(&self, width: usize) -> Document {
        if self.show_help {
            HelpScreen::document()
        } else if self.show_outline {
            Screen::outline(&self.review, &self.session, self.outline_selection)
        } else if self.stage == 0 {
            Screen::overview(&self.review, &self.session)
        } else if let Some(index) = self.chunk_index() {
            let selected = self
                .selected_feedback()
                .map(|feedback| feedback.id.as_str());
            self.chunk_document(index, width, selected)
        } else {
            Screen::summary(&self.review, &self.session, self.confirm_finish)
        }
    }

    fn chunk_document(&self, index: usize, width: usize, selected: Option<&str>) -> Document {
        if width >= SIDE_PANE_MIN_WIDTH && self.review.spec().chunks[index].context.is_some() {
            Screen::chunk_main(
                &self.review,
                &self.session,
                &self.review.spec().chunks[index],
                selected,
                self.show_raw_source,
                self.selected_line.as_ref(),
            )
        } else {
            Screen::chunk_content(
                &self.review,
                &self.session,
                index,
                width,
                selected,
                self.show_raw_source,
                self.selected_line.as_ref(),
            )
        }
    }

    pub(super) fn sidebar_document(&self, width: usize) -> Option<Document> {
        if width < SIDE_PANE_MIN_WIDTH || self.show_help || self.show_outline {
            return None;
        }
        let index = self.chunk_index()?;
        self.review.spec().chunks[index]
            .context
            .as_ref()
            .map(Screen::context)
    }

    fn content_width(&self, width: usize) -> usize {
        if self.sidebar_document(width).is_some() {
            width.saturating_mul(2) / 3
        } else {
            width
        }
    }

    fn footer_document(&self, width: usize, total_lines: usize, body_height: usize) -> Document {
        let mut document = self.footer_command_document(width);
        let position = if total_lines > body_height {
            let document = self
                .current_document(width)
                .wrapped(self.content_width(width));
            let context = document.lines.iter().position(|line| {
                line_text(line)
                    .to_ascii_lowercase()
                    .starts_with("context ·")
            });
            let source = document.lines.iter().position(is_source_header);
            // Labelled by the middle of the page, not its top: a page that
            // opens on one overlap row of code and then shows a context pane
            // is a context page (found by self-review's UI judge).
            let middle = self.scroll + body_height / 2;
            let section = if context.is_some_and(|row| middle >= row) {
                "Context"
            } else if source.is_some_and(|row| middle >= row) {
                "Source"
            } else {
                "Explanation"
            };
            // Rows after the last content row are never paged to, so the
            // cue must not promise them (self-review's UI judge found
            // "1–18 / 21" on a chunk's only page).
            let owed = self
                .last_content_row
                .map_or(total_lines, |last| (last + 1).min(total_lines));
            format!(
                "{section} · view {}–{} / {owed}",
                self.scroll + 1,
                (self.scroll + body_height).min(owed)
            )
        } else {
            String::new()
        };
        if self.notice.is_none() && position.is_empty() {
            return document;
        }
        document.newline();
        if let Some(notice) = &self.notice {
            document.push(
                notice,
                TextStyle {
                    foreground: Some(if self.confirm_finish {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                    bold: self.confirm_finish,
                    ..TextStyle::default()
                },
            );
            let room = width.saturating_sub(document.lines.last().map_or(0, |line| line.width()));
            if position.len() + 3 <= room {
                document.push(format!(" · {position}"), muted());
            }
        } else {
            document.push(
                position,
                TextStyle {
                    foreground: Some(Color::Grey),
                    ..TextStyle::default()
                },
            );
        }
        document
    }

    pub(super) fn footer_command_document(&self, width: usize) -> Document {
        let narrow = width < 70;
        let compact = width < 120;
        let hints: Vec<(&str, String)> = if self.show_help {
            vec![
                ("↑/↓", "Scroll".into()),
                ("PgUp/PgDn", "Page".into()),
                ("Esc/?", "Close help".into()),
                ("q", "Quit".into()),
            ]
        } else if self.show_outline {
            vec![
                ("Enter", "Open".into()),
                ("↑/↓", "Choose".into()),
                ("Esc/o", "Close".into()),
                ("?", "Help".into()),
                ("q", "Quit".into()),
            ]
        } else if self.stage == 0 {
            vec![
                (
                    "Enter/→",
                    if self.has_saved_progress() {
                        "Resume review".into()
                    } else {
                        "Start review".into()
                    },
                ),
                ("↑/↓", "Scroll".into()),
                ("o", "Outline".into()),
                ("?", "Help".into()),
                ("q", "Quit".into()),
            ]
        } else if self.is_summary() {
            let action = if self.ended() {
                "Done".to_owned()
            } else if self.confirm_finish {
                if self.partial_approval_pending() {
                    "Confirm batch approval".to_owned()
                } else {
                    "Confirm end".to_owned()
                }
            } else if self.generation_pending() {
                format!("Record batch: {}", self.outcome().label())
            } else if compact {
                format!("End: {}", self.outcome().label())
            } else {
                format!("End review as {}", self.outcome().label())
            };
            if self.ended() {
                let mut hints = vec![(if narrow { "↵/q" } else { "Enter/q" }, action)];
                if self.unreviewed_count() > 0 {
                    hints.push(("u", "Unreviewed".into()));
                }
                if self.first_unresolved_feedback().is_some() {
                    hints.push(("g", "Open request".into()));
                }
                if !self.review.spec().chunks.is_empty() {
                    hints.push(("←/p", "Previous".into()));
                }
                hints.push(("?", "Help".into()));
                hints
            } else {
                let mut hints = vec![(if narrow { "↵" } else { "Enter" }, action)];
                if self.confirm_finish {
                    hints.push(("Esc", "Cancel".into()));
                    hints.push((
                        "q",
                        if narrow {
                            "Save+quit".into()
                        } else {
                            "Quit; progress kept".into()
                        },
                    ));
                }
                if !self.confirm_finish {
                    hints.extend([
                        ("u", "Unreviewed".into()),
                        ("g", "Open request".into()),
                        ("←/p", "Previous".into()),
                        ("?", if narrow { "More".into() } else { "Help".into() }),
                        ("q", "Quit".into()),
                    ]);
                }
                hints
            }
        } else if let Some(feedback) = self.selected_feedback() {
            let mut hints = if narrow {
                vec![("↑↓", "Pick".into()), ("Pg", "Scroll".into())]
            } else {
                vec![("↑/↓", "Select".into()), ("PgUp/PgDn", "Scroll".into())]
            };
            if feedback.kind == FeedbackKind::Issue {
                hints.push(("x", resolve_action(feedback.resolved).into()));
            }
            if narrow {
                hints.extend([("Esc", "Done".into()), ("?", "More".into())]);
            } else {
                hints.extend([
                    ("t", retarget_action(feedback.target).into()),
                    ("d", "Delete".into()),
                    ("Esc", "Done".into()),
                    ("?", "Help".into()),
                    ("q", "Quit".into()),
                ]);
            }
            hints
        } else {
            let reviewed = self.chunk_index().is_some_and(|index| {
                self.session
                    .reviewed
                    .contains(&self.review.spec().chunks[index].id)
            });
            let mut hints = if narrow {
                vec![
                    (
                        "↵",
                        if reviewed {
                            "Next".into()
                        } else {
                            "Review+next".into()
                        },
                    ),
                    ("p/n", "Chunks".into()),
                    ("s/z", "Src/ctx".into()),
                    ("q", "Quit".into()),
                    ("?", "More".into()),
                ]
            } else if compact {
                vec![
                    (
                        "Enter",
                        if reviewed {
                            "Next".into()
                        } else {
                            "Review+next".into()
                        },
                    ),
                    ("↑/↓", "Scroll".into()),
                    ("p/n", "Chunks".into()),
                    ("c/!", "Feedback".into()),
                ]
            } else {
                vec![
                    (
                        "Enter",
                        if reviewed {
                            "Next".into()
                        } else {
                            "Review + next".into()
                        },
                    ),
                    ("↑/↓", "Scroll".into()),
                    ("p/n", "Chunks".into()),
                    ("r", review_action(reviewed).into()),
                    ("c", "Comment".into()),
                    ("!", "Request".into()),
                ]
            };
            if !narrow && !self.chunk_feedback().is_empty() {
                hints.push((
                    "f",
                    if compact {
                        format!("Items ({})", self.chunk_feedback().len())
                    } else {
                        format!("Feedback ({})", self.chunk_feedback().len())
                    },
                ));
            }
            if !narrow {
                hints.push(("s", "Jump to source".into()));
                hints.push((
                    "v",
                    if self.show_raw_source {
                        if compact {
                            "Rendered".into()
                        } else {
                            "Rendered view".into()
                        }
                    } else if compact {
                        "Raw".into()
                    } else {
                        "Raw + lines".into()
                    },
                ));
            }
            if !compact {
                hints.push(("o", "Outline".into()));
            }
            if !narrow {
                hints.extend([
                    (
                        "?",
                        if compact {
                            "More".into()
                        } else {
                            "Help".into()
                        },
                    ),
                    ("q", "Quit".into()),
                ]);
            }
            hints
        };
        pack_hints(&hints, width)
    }
}

/// A row that introduces what follows it: a bold heading, or a context
/// pane's title. Self-review found the title ending a page alone, because it
/// is styled as a title rather than bold.
pub(super) fn is_heading(line: &super::style::Line) -> bool {
    (line.width() > 0 && line.spans.iter().all(|span| span.style.bold))
        || line_text(line)
            .to_ascii_lowercase()
            .starts_with("context ·")
}

/// A row that shows the reader nothing: blank, a bare source gutter, or an
/// elision marker.
pub(super) fn is_content_free(line: &super::style::Line) -> bool {
    let text = line_text(line);
    let body = text.strip_prefix(SOURCE_GUTTER).unwrap_or(&text).trim();
    body.is_empty() || body == SOURCE_GUTTER.trim() || body.starts_with('…')
}

pub(super) fn is_source_header(line: &super::style::Line) -> bool {
    let text = line_text(line);
    text.starts_with(SOURCE_GUTTER) && text.contains(" · ")
}

pub(super) fn page_step_for(terminal_height: usize) -> usize {
    terminal_height.saturating_sub(8).max(1)
}

/// A source label is useful only with source beneath it. Leaving a little
/// unused room is less disorienting than putting the label on one page and its
/// first code row on the next.
fn avoid_orphaned_source_header(document: &Document, maximum: usize, offset: usize) -> usize {
    let end = offset.saturating_add(maximum).min(document.lines.len());
    let orphan = (offset..end).find(|&index| {
        let line = &document.lines[index];
        // Nothing but empty rows after a label leaves it as alone as being
        // last: a blank row, a bare gutter, or an elision marker.
        let only_empty_after =
            (index + 1..end).all(|after| is_content_free(&document.lines[after]));
        (is_source_header(line) || is_heading(line)) && only_empty_after
    });
    orphan
        .map(|index| index.saturating_sub(offset))
        .filter(|height| *height > 0)
        .unwrap_or(maximum)
}

/// Rows a page may give up to keep a prose paragraph whole on the next page.
const PARAGRAPH_SLACK: usize = 8;

/// Ends a page before a prose paragraph it would otherwise cut, when that
/// costs few rows. A UI judge found a page stopping at "Commands that read"
/// with the rest of the sentence alone on a near-empty next page. Code rows
/// are left alone: their line numbers already say where the reader is.
fn avoid_split_paragraph(document: &Document, maximum: usize, offset: usize) -> usize {
    let end = offset.saturating_add(maximum);
    // A context pane that would fit whole on the next page starts it, rather
    // than leaving its definition here and its example on a near-empty page.
    let pane = (offset + 1..end.min(document.lines.len())).find(|&index| {
        line_text(&document.lines[index])
            .to_ascii_lowercase()
            .starts_with("context ·")
    });
    // Never at the cost of most of this page: a pane title just below the
    // top would otherwise leave a page of one overlap row (found at 80x24).
    if let Some(start) = pane
        && end < document.lines.len()
        && document.lines.len() - start <= maximum
        && (start - offset) * 3 >= maximum
    {
        return start - offset;
    }
    let prose = |index: usize| {
        document.lines.get(index).is_some_and(|line| {
            let text = line_text(line);
            !text.trim().is_empty() && !text.starts_with(SOURCE_GUTTER.trim_end())
        })
    };
    if end >= document.lines.len() || !prose(end - 1) || !prose(end) {
        return maximum;
    }
    let start = (offset..end).rev().take_while(|&index| prose(index)).last();
    match start {
        Some(start) if start > offset && end - start <= PARAGRAPH_SLACK => start - offset,
        _ => maximum,
    }
}

fn help_page_height(document: &Document, maximum: usize, offset: usize) -> usize {
    let remaining = document.lines.len().saturating_sub(offset);
    if remaining <= maximum {
        return remaining;
    }
    let target = offset.saturating_add(maximum).min(document.lines.len());
    (offset + 1..=target)
        .rev()
        .find(|&index| help_block_starts(document, index))
        .map_or(maximum, |end| end - offset)
}

fn previous_help_boundary(document: &Document, target: usize) -> usize {
    (0..=target.min(document.lines.len().saturating_sub(1)))
        .rev()
        .find(|&index| help_block_starts(document, index))
        .unwrap_or(0)
}

fn help_block_starts(document: &Document, index: usize) -> bool {
    let Some(line) = document.lines.get(index) else {
        return false;
    };
    let text = line
        .spans
        .iter()
        .map(|span| span.text.as_str())
        .collect::<String>();
    text.trim_start().starts_with('•')
        || (line.width() > 0
            && index > 0
            && document
                .lines
                .get(index - 1)
                .is_some_and(|line| line.width() == 0))
}

fn pack_hints(hints: &[(&str, String)], width: usize) -> Document {
    let mut document = Document::new();
    for (key, label) in hints {
        let text_width = key.chars().count() + label.chars().count() + 3;
        let current = document.lines.last().map_or(0, |line| line.width());
        if current > 0 && current + 2 + text_width > width {
            document.newline();
        } else if current > 0 {
            document.push("  ", TextStyle::default());
        }
        document.push(
            format!("[{key}]"),
            TextStyle {
                foreground: Some(Color::White),
                bold: true,
                ..TextStyle::default()
            },
        );
        document.push(
            format!(" {label}"),
            TextStyle {
                foreground: Some(Color::Grey),
                ..TextStyle::default()
            },
        );
    }
    document
}

#[cfg(test)]
mod pagination_tests {
    use super::{avoid_orphaned_source_header, help_block_starts, help_page_height, page_step_for};
    use crate::tui::{
        help::HelpScreen,
        style::{Document, TextStyle},
    };

    fn text(document: &Document, index: usize) -> String {
        document.lines[index]
            .spans
            .iter()
            .map(|span| span.text.as_str())
            .collect()
    }

    #[test]
    fn help_pages_stop_before_splitting_the_next_bullet() {
        let help = HelpScreen::document().wrapped(65);
        let (maximum, height) = (8..=24)
            .find_map(|maximum| {
                let height = help_page_height(&help, maximum, 0);
                (height < maximum).then_some((maximum, height))
            })
            .expect("help contains a semantic page boundary");

        assert!(height < maximum, "the next bullet should move as a unit");
        assert!(help_block_starts(&help, height));
        assert!(text(&help, height).trim_start().starts_with('•'));
        assert!(
            !text(&help, height - 1).trim_start().starts_with('•'),
            "the prior block should have finished before the page break"
        );
    }

    #[test]
    fn source_header_waits_for_a_page_with_source_beneath_it() {
        let mut document = Document::new();
        document.push("explanation", TextStyle::default());
        document.newline();
        document.push(
            "▏ src/lib.rs:1-2 · added code · modified",
            TextStyle::default(),
        );
        document.newline();
        document.push("▏ ", TextStyle::default());
        document.newline();
        document.push("▏    1 source", TextStyle::default());

        assert_eq!(avoid_orphaned_source_header(&document, 3, 0), 1);
        assert_eq!(avoid_orphaned_source_header(&document, 4, 0), 4);
    }

    #[test]
    fn narrow_page_movement_keeps_context_from_the_previous_page() {
        // A 22-row review has about 15 body rows after its pinned header and
        // footer. Moving 14 retains one row of overlap instead of dropping the
        // source label that ended the previous page.
        assert_eq!(page_step_for(22), 14);
    }
}

fn resolve_action(resolved: bool) -> &'static str {
    if resolved { "Reopen" } else { "Resolve" }
}

fn retarget_action(target: FeedbackTarget) -> &'static str {
    match target {
        FeedbackTarget::Code => "Retarget → explanation",
        FeedbackTarget::Explanation => "Retarget → code",
    }
}

pub(super) fn line_text(line: &super::style::Line) -> String {
    line.spans.iter().map(|span| span.text.as_str()).collect()
}

fn review_action(reviewed: bool) -> &'static str {
    if reviewed { "Unreview" } else { "Review" }
}

#[cfg(test)]
mod tests {
    use super::{pack_hints, resolve_action, review_action};

    #[test]
    fn review_action_reflects_current_state() {
        assert_eq!(review_action(false), "Review");
        assert_eq!(review_action(true), "Unreview");
    }

    #[test]
    fn resolve_action_reflects_current_state() {
        assert_eq!(resolve_action(false), "Resolve");
        assert_eq!(resolve_action(true), "Reopen");
    }

    #[test]
    fn narrow_hints_wrap_only_between_complete_actions() {
        let hints = vec![
            ("Enter", "Review + next".to_owned()),
            ("o", "Outline".to_owned()),
            ("?", "Help".to_owned()),
            ("q", "Quit".to_owned()),
        ];

        let footer = pack_hints(&hints, 32);
        let text: String = footer
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();

        assert!(footer.lines.iter().all(|line| line.width() <= 32));
        assert!(text.contains("[?] Help"));
        assert!(text.contains("[q] Quit"));
    }

    /// Reviewer capture found Quit orphaned on a second row at the standard
    /// 110-column width. Keep the compact chunk legend on one row there.
    #[test]
    fn standard_width_chunk_hints_fit_one_row() {
        let hints = vec![
            ("Enter", "Review+next".to_owned()),
            ("↑/↓", "Scroll".to_owned()),
            ("p/n", "Chunks".to_owned()),
            ("c/!", "Feedback".to_owned()),
            ("f", "Items (4)".to_owned()),
            ("v", "Source".to_owned()),
            ("?", "More".to_owned()),
            ("q", "Quit".to_owned()),
        ];

        let footer = pack_hints(&hints, 110);

        assert_eq!(footer.lines.len(), 1, "standard footer wrapped");
        assert!(footer.lines[0].width() <= 110);
    }
}
