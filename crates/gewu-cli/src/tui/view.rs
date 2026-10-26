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
    help::HelpScreen,
    render::Screen,
    render_support::{SOURCE_GUTTER, muted},
    style::{Document, TextStyle},
};

impl ReviewTui {
    pub(super) fn draw(&mut self) -> Result<()> {
        let (width, height) = size()?;
        let (width, height) = (usize::from(width), usize::from(height));
        let panel = self.prompt_panel();
        let sidebar = self.sidebar_document(width);
        let content_width = if sidebar.is_some() {
            width.saturating_mul(2) / 3
        } else {
            width
        };
        let pinned = self
            .pinned_document(width)
            .map(|document| document.wrapped(width));
        let pinned_rows = pinned.as_ref().map_or(0, |document| document.lines.len());

        let content = self.current_document(width).wrapped(content_width);
        let command_rows = self.footer_command_document(width).lines.len();
        let footer_rows = if let Some(panel) = &panel {
            panel.lines.len()
        } else {
            let room_without_status = height.saturating_sub(command_rows + pinned_rows);
            command_rows
                + usize::from(self.notice.is_some() || content.lines.len() > room_without_status)
        };
        let body_top = pinned_rows.min(height);
        let body_bottom = height.saturating_sub(footer_rows).max(body_top);
        let body_height = body_bottom.saturating_sub(body_top);
        let max_scroll = content.lines.len().saturating_sub(body_height);
        self.scroll = self.scroll.min(max_scroll);
        let mut visible_height = if self.show_help {
            help_page_height(&content, body_height, self.scroll)
        } else {
            body_height
        };
        visible_height = avoid_orphaned_source_header(&content, visible_height, self.scroll);
        let visible = match sidebar {
            Some(sidebar) => {
                let left = Document {
                    lines: content
                        .lines
                        .iter()
                        .skip(self.scroll)
                        .take(visible_height)
                        .cloned()
                        .collect(),
                };
                let columns = Document::columns(left, sidebar, width);
                columns.ansi_window(width, visible_height, 0)
            }
            None => content.ansi_window(width, visible_height, self.scroll),
        };

        execute!(io::stdout(), MoveTo(0, 0), Clear(ClearType::All))?;
        if let Some(pinned) = &pinned {
            execute!(
                io::stdout(),
                MoveTo(0, 0),
                Print(pinned.ansi_window(width, pinned.lines.len(), 0))
            )?;
        }
        execute!(
            io::stdout(),
            MoveTo(0, u16::try_from(body_top).unwrap_or(u16::MAX)),
            Print(visible)
        )?;
        match &panel {
            Some(panel) => self.draw_prompt(
                panel,
                u16::try_from(width).unwrap_or(u16::MAX),
                u16::try_from(body_bottom).unwrap_or(u16::MAX),
            )?,
            None => {
                let footer = self.footer_document(width, content.lines.len(), visible_height);
                execute!(
                    io::stdout(),
                    MoveTo(0, u16::try_from(body_bottom).unwrap_or(u16::MAX)),
                    Print(footer.ansi_window(width, footer.lines.len(), 0))
                )?;
            }
        }
        io::stdout().flush()?;
        Ok(())
    }

    pub(super) fn page_step(&self) -> usize {
        size()
            .map(|(_, height)| page_step_for(usize::from(height)))
            .unwrap_or(10)
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
        if width >= 100 {
            self.notice = Some("Context is visible in the sidebar".into());
            return;
        }
        let document = self.current_document(width).wrapped(width);
        if let Some(row) = document
            .lines
            .iter()
            .position(|line| line_text(line).starts_with("Context ·"))
        {
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
        if width < 100
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
        document.newline();
        Some(document)
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
        if width >= 100 && self.review.spec().chunks[index].context.is_some() {
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
        if width < 100 || self.show_help || self.show_outline {
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
            let context = document
                .lines
                .iter()
                .position(|line| line_text(line).starts_with("Context ·"));
            let source = document.lines.iter().position(is_source_header);
            let section = if context.is_some_and(|row| self.scroll >= row) {
                "Context"
            } else if source.is_some_and(|row| self.scroll >= row) {
                "Source"
            } else {
                "Explanation"
            };
            format!(
                "{section} · view {}–{} / {total_lines}",
                self.scroll + 1,
                (self.scroll + body_height).min(total_lines)
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

fn is_source_header(line: &super::style::Line) -> bool {
    let text = line_text(line);
    text.starts_with(SOURCE_GUTTER) && text.contains(" · ")
}

fn page_step_for(terminal_height: usize) -> usize {
    terminal_height.saturating_sub(8).max(1)
}

/// A source label is useful only with source beneath it. Leaving a little
/// unused room is less disorienting than putting the label on one page and its
/// first code row on the next.
fn avoid_orphaned_source_header(document: &Document, maximum: usize, offset: usize) -> usize {
    let end = offset.saturating_add(maximum).min(document.lines.len());
    let orphan = (offset..end).find(|&index| {
        let line = &document.lines[index];
        let heading = line.width() > 0 && line.spans.iter().all(|span| span.style.bold);
        (is_source_header(line) && index + 2 >= end) || (heading && index + 1 >= end)
    });
    orphan
        .map(|index| index.saturating_sub(offset))
        .filter(|height| *height > 0)
        .unwrap_or(maximum)
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

fn line_text(line: &super::style::Line) -> String {
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
