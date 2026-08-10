use std::io::{self, Write};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType, size},
};

use gewu_review::FeedbackTarget;

use super::{ReviewTui, help::HelpScreen, render::Screen, style::Document};

impl ReviewTui {
    pub(super) fn draw(&mut self) -> Result<()> {
        let (width, height) = size()?;
        // A prompt takes the foot of the screen, as many rows as it needs; the
        // reading area gives way to it rather than being written over.
        let panel = self.prompt_panel();
        let panel_rows = panel
            .as_ref()
            .and_then(|panel| u16::try_from(panel.lines.len()).ok())
            .unwrap_or(2);
        let top = height.saturating_sub(panel_rows);
        let body_height = usize::from(top);
        let content = self
            .current_document(usize::from(width))
            .wrapped(usize::from(width));
        let max_scroll = content.lines.len().saturating_sub(body_height);
        self.scroll = self.scroll.min(max_scroll);
        let visible = content.ansi_window(usize::from(width), body_height, self.scroll);

        execute!(io::stdout(), MoveTo(0, 0), Clear(ClearType::All))?;
        print!("{visible}");
        match &panel {
            Some(panel) => self.draw_prompt(panel, width, top)?,
            None => {
                let footer = self.footer(content.lines.len(), body_height);
                execute!(
                    io::stdout(),
                    MoveTo(0, top),
                    SetForegroundColor(Color::DarkGrey),
                    Print(footer.0),
                    MoveTo(0, height.saturating_sub(1)),
                    Print(footer.1),
                    ResetColor
                )?;
            }
        }
        io::stdout().flush()?;
        Ok(())
    }

    pub(super) fn page_step(&self) -> usize {
        size()
            .map(|(_, height)| usize::from(height.saturating_sub(4)).max(1))
            .unwrap_or(10)
    }

    pub(super) fn scroll_to_end(&mut self) {
        let width = size().map_or(80, |(width, _)| usize::from(width));
        self.scroll = self.current_document(width).wrapped(width).lines.len();
    }

    fn current_document(&self, width: usize) -> Document {
        if self.show_help {
            HelpScreen::document()
        } else if self.stage == 0 {
            Screen::overview(&self.review)
        } else if let Some(index) = self.chunk_index() {
            let selected = self
                .selected_feedback()
                .map(|feedback| feedback.id.as_str());
            Screen::chunk(&self.review, &self.session, index, width, selected)
        } else {
            Screen::summary(&self.review, &self.session)
        }
    }

    fn footer(&self, total_lines: usize, body_height: usize) -> (String, String) {
        let position = if total_lines <= body_height {
            format!("{total_lines} lines")
        } else {
            format!(
                "lines {}–{} of {total_lines}",
                self.scroll + 1,
                (self.scroll + body_height).min(total_lines)
            )
        };
        if self.show_help {
            return ("[Esc/?] Close help".into(), position);
        }
        if self.stage == 0 {
            return (
                "[Enter/→] Start review    [↑/↓] Scroll    [?] Help    [q] Quit".into(),
                position,
            );
        }
        if self.is_summary() {
            return (
                format!(
                    "[Enter] End the review as \"{}\"  [u] Unreviewed  [g] Open request",
                    self.outcome().label()
                ),
                format!("[←/p] Previous chunk    [?] Help    [q] Quit    {position}"),
            );
        }
        if let Some(feedback) = self.selected_feedback() {
            return (
                format!(
                    "Selected feedback: [x] {}  [t] {}  [d] Delete  [q] Quit",
                    resolve_action(feedback.resolved),
                    retarget_action(feedback.target)
                ),
                format!("[↑/k ↓/j] Select  [Esc] Done    {position}"),
            );
        }
        let reviewed = self.chunk_index().is_some_and(|index| {
            self.session
                .reviewed
                .contains(&self.review.spec().chunks[index].id)
        });
        (
            format!(
                "[Enter] {}  [↑/k ↓/j] Scroll  [←/p →/n] Chunk  [r] {}  [?] Help  [q] Quit",
                advance_action(reviewed, self.chunk_feedback().is_empty()),
                review_action(reviewed)
            ),
            format!(
                "Feedback: [c] Comment  [!] Request change{}    {position}",
                select_action(self.chunk_feedback().len())
            ),
        )
    }
}

/// Named only when there is feedback to select, so the actions that act on a
/// selection are never advertised with nothing to act on.
fn select_action(count: usize) -> String {
    if count == 0 {
        return String::new();
    }
    format!("  [f] Select ({count})")
}

fn resolve_action(resolved: bool) -> &'static str {
    if resolved { "Reopen" } else { "Resolve" }
}

/// The key offers the aim the feedback does not currently have, so the label
/// says what pressing it does rather than what is already true.
fn retarget_action(target: FeedbackTarget) -> &'static str {
    match target {
        FeedbackTarget::Code => "About the explanation",
        FeedbackTarget::Explanation => "About the code",
    }
}

/// What Enter will do from here, so the reviewer is told before they press it
/// rather than after. It only claims a chunk they have said nothing about and
/// have not already settled.
fn advance_action(reviewed: bool, unremarked: bool) -> &'static str {
    if reviewed || !unremarked {
        "Next chunk"
    } else {
        "Reviewed, next"
    }
}

fn review_action(reviewed: bool) -> &'static str {
    if reviewed {
        "Mark unreviewed"
    } else {
        "Mark reviewed"
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_action, review_action, select_action};

    #[test]
    fn review_action_reflects_current_state() {
        assert_eq!(review_action(false), "Mark reviewed");
        assert_eq!(review_action(true), "Mark unreviewed");
    }

    #[test]
    fn offers_selection_only_once_feedback_exists() {
        assert_eq!(select_action(0), "");
        assert_eq!(select_action(2), "  [f] Select (2)");
    }

    #[test]
    fn resolve_action_reflects_current_state() {
        assert_eq!(resolve_action(false), "Resolve");
        assert_eq!(resolve_action(true), "Reopen");
    }
}
