use std::io;

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::Color,
    terminal::size,
};

use super::{
    ReviewTui,
    entry::Entry,
    render_support::muted,
    style::{Document, TextStyle},
};

/// What the reviewer is being asked at the foot of the screen: a question that
/// one key answers, or a piece of writing they are composing.
pub(super) struct Prompt {
    title: String,
    hint: String,
    /// The text being written, when this prompt takes text at all.
    input: Option<Entry>,
}

const ENTRY_HINT: &str = "[Enter] save   [Shift+Enter] new line   [←/→/↑/↓] move   \
                          [PgUp/PgDn] scroll the chunk   [Esc] cancel";
const INPUT_GUTTER: &str = "│ ";
const CURSOR: &str = "█";

impl Prompt {
    fn question(title: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            hint: hint.into(),
            input: None,
        }
    }

    fn entry(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            hint: ENTRY_HINT.into(),
            input: Some(Entry::default()),
        }
    }

    fn entry_mut(&mut self) -> Option<&mut Entry> {
        self.input.as_mut()
    }

    /// Draws the panel: a rule opening it, the title and its keys over as many
    /// lines as they need, then — when text is being written — a second rule
    /// marking where the writing begins and the text itself below it.
    ///
    /// `rows` bounds the whole panel. Writing may outgrow it, so the part kept
    /// on screen is the part the cursor is in.
    pub(super) fn document(&self, width: usize, rows: usize) -> Document {
        let mut document = Document::new();
        document.push(rule(width), muted());
        document.newline();
        document.push(
            &self.title,
            TextStyle {
                bold: true,
                ..TextStyle::default()
            },
        );
        document.newline();
        document.push(&self.hint, muted());
        let mut document = document.wrapped(width);
        if self.input.is_none() {
            return clamped(document, rows);
        }
        document.newline();
        document.push(rule(width), muted());
        let input = self
            .input_region(width.saturating_sub(INPUT_GUTTER.chars().count()))
            .gutter(INPUT_GUTTER, muted());
        let room = rows.saturating_sub(document.lines.len()).max(1);
        let first = first_visible(&input, room);
        document
            .lines
            .extend(input.lines.into_iter().skip(first).take(room));
        clamped(document, rows)
    }

    /// The text as written: every line break the reviewer made is a line here,
    /// and the cursor marks the character it sits on — or a block of its own at
    /// the end of a line, where there is no character to mark.
    fn input_region(&self, width: usize) -> Document {
        let Some(entry) = &self.input else {
            return Document::new();
        };
        let (before, under, after) = entry.split();
        let mut document = Document::new();
        document.push(before, TextStyle::default());
        match under {
            Some('\n') | None => document.push(CURSOR, cursor()),
            Some(character) => document.push(character.to_string(), cursor()),
        }
        if under == Some('\n') {
            document.newline();
        }
        document.push(after, TextStyle::default());
        document.wrapped(width)
    }
}

/// The editing keys terminals have carried since readline, so muscle memory
/// works here too.
fn control(entry: &mut Entry, character: char) {
    match character {
        'a' => entry.move_line_start(),
        'e' => entry.move_line_end(),
        'b' => entry.move_left(),
        'f' => entry.move_right(),
        'd' => entry.delete(),
        'h' => entry.backspace(),
        'k' => entry.delete_to_line_end(),
        'u' => entry.delete_to_line_start(),
        'w' => entry.delete_word_left(),
        _ => {}
    }
}

/// Reversed, so the character the cursor is on stays readable under it.
fn cursor() -> TextStyle {
    TextStyle {
        foreground: Some(Color::Black),
        background: Some(Color::Grey),
        ..TextStyle::default()
    }
}

/// The first line of writing to show. Long writing outgrows the panel, and the
/// line that must survive that is the one being typed on.
fn first_visible(input: &Document, room: usize) -> usize {
    if input.lines.len() <= room {
        return 0;
    }
    let at = input
        .lines
        .iter()
        .position(|line| {
            line.spans
                .iter()
                .any(|span| span.style.background.is_some())
        })
        .unwrap_or(input.lines.len().saturating_sub(1));
    at.saturating_sub(room - 1)
        .min(input.lines.len().saturating_sub(room))
}

fn rule(width: usize) -> String {
    "─".repeat(width)
}

/// Holds the panel to the rows it was given. A narrow terminal can wrap the
/// title past its share of the screen, and the writing has to survive that:
/// the foot of the panel is what the reviewer is working in.
fn clamped(mut document: Document, rows: usize) -> Document {
    let over = document.lines.len().saturating_sub(rows.max(1));
    document.lines.drain(..over);
    document
}

impl ReviewTui {
    /// Reads a body of feedback while the chunk stays scrollable, so a
    /// reviewer can look back at what they are commenting on. Enter saves and
    /// Shift+Enter adds a line, which needs a terminal that tells the two
    /// apart; pasting works everywhere.
    pub(super) fn write(&mut self, title: &str) -> Result<Option<String>> {
        self.prompt = Some(Prompt::entry(title));
        let written = self.read_entry();
        self.prompt = None;
        written
    }

    fn read_entry(&mut self) -> Result<Option<String>> {
        loop {
            self.draw()?;
            let event = event::read()?;
            let Some(entry) = self.prompt.as_mut().and_then(Prompt::entry_mut) else {
                return Ok(None);
            };
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Release => {}
                Event::Key(key) => {
                    let word = key
                        .modifiers
                        .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL);
                    match key.code {
                        KeyCode::Esc => return Ok(None),
                        KeyCode::Enter
                            if key
                                .modifiers
                                .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) =>
                        {
                            entry.insert_char('\n');
                        }
                        KeyCode::Enter => return Ok(Some(entry.text().trim().to_owned())),
                        KeyCode::Left if word => entry.move_word_left(),
                        KeyCode::Right if word => entry.move_word_right(),
                        KeyCode::Left => entry.move_left(),
                        KeyCode::Right => entry.move_right(),
                        KeyCode::Up => entry.move_up(),
                        KeyCode::Down => entry.move_down(),
                        KeyCode::Home => entry.move_line_start(),
                        KeyCode::End => entry.move_line_end(),
                        KeyCode::Backspace if word => entry.delete_word_left(),
                        KeyCode::Backspace => entry.backspace(),
                        KeyCode::Delete => entry.delete(),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(None);
                        }
                        KeyCode::Char(character)
                            if key.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            control(entry, character);
                        }
                        KeyCode::Char(character) => entry.insert_char(character),
                        // Paging still belongs to the chunk: a reviewer has to
                        // be able to look back at what they are commenting on.
                        code => self.scroll_by(code),
                    }
                }
                // A paste arrives whole, so its line breaks stay in the comment
                // instead of ending it early.
                Event::Paste(text) => {
                    entry.insert(&text.replace("\r\n", "\n").replace('\r', "\n"));
                }
                _ => {}
            }
        }
    }

    /// Puts one question at the foot of the screen and waits for a key, leaving
    /// the chunk itself on screen and unchanged.
    pub(super) fn ask(&mut self, title: &str, hint: &str) -> Result<KeyCode> {
        self.prompt = Some(Prompt::question(title, hint));
        let code = loop {
            self.draw()?;
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Release {
                break key.code;
            }
        };
        self.prompt = None;
        Ok(code)
    }

    pub(super) fn confirm(&mut self, title: &str) -> Result<bool> {
        loop {
            match self.ask(title, "[y] delete   [n/Esc] keep   [↑/↓] scroll")? {
                KeyCode::Char('y') => return Ok(true),
                KeyCode::Esc | KeyCode::Char('n') => return Ok(false),
                code => self.scroll_by(code),
            }
        }
    }

    /// Prints the panel across the foot of the screen, under whatever the
    /// reviewer is reading.
    pub(super) fn draw_prompt(&self, panel: &Document, width: u16, top: u16) -> Result<()> {
        execute!(io::stdout(), MoveTo(0, top))?;
        print!(
            "{}",
            panel.ansi_window(usize::from(width), panel.lines.len(), 0)
        );
        Ok(())
    }

    pub(super) fn prompt_panel(&self) -> Option<Document> {
        let (width, height) = size().ok()?;
        let prompt = self.prompt.as_ref()?;
        // Never more than half the screen: the code being commented on has to
        // stay in view while the comment is written.
        Some(prompt.document(usize::from(width), usize::from(height / 2).max(1)))
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, Prompt};

    fn lines(document: &super::Document) -> Vec<String> {
        document
            .lines
            .iter()
            .map(|line| line.spans.iter().map(|span| span.text.as_str()).collect())
            .collect()
    }

    /// A prompt part-way through being written, with the cursor at the end.
    fn written(title: &str, text: &str) -> Prompt {
        let mut prompt = Prompt::entry(title);
        prompt.input = Some(Entry::from(text));
        prompt
    }

    #[test]
    fn draws_a_question_without_an_input_region() {
        let prompt = Prompt::question("Request change", "[Enter] whole chunk");

        let drawn = lines(&prompt.document(30, 10));

        assert_eq!(drawn[0], "─".repeat(30));
        assert_eq!(drawn[1], "Request change");
        assert_eq!(drawn[2], "[Enter] whole chunk");
        assert_eq!(drawn.len(), 3);
    }

    #[test]
    fn gives_every_line_of_a_comment_its_own_row() {
        let prompt = written("Comment", "first\nsecond");

        let drawn = lines(&prompt.document(40, 10));

        assert!(drawn.contains(&"│ first".to_owned()), "{drawn:#?}");
        assert!(drawn.contains(&"│ second█".to_owned()), "{drawn:#?}");
    }

    #[test]
    fn marks_the_character_the_cursor_sits_on() {
        let mut prompt = written("Comment", "first\nsecond");
        let entry = prompt.input.as_mut().expect("entry");
        entry.move_up();
        entry.move_line_start();

        let drawn = lines(&prompt.document(40, 10));

        // The cursor is a mark on the line, not an extra character in it.
        assert!(drawn.contains(&"│ first".to_owned()), "{drawn:#?}");
        assert!(drawn.contains(&"│ second".to_owned()), "{drawn:#?}");
        let marked = prompt
            .document(40, 10)
            .lines
            .iter()
            .flat_map(|line| line.spans.clone())
            .find(|span| span.style.background.is_some())
            .expect("a marked cell");
        assert_eq!(marked.text, "f");
    }

    #[test]
    fn keeps_the_line_being_typed_on_in_view() {
        let mut prompt = written("Comment", "one\ntwo\nthree\nfour\nfive");
        let entry = prompt.input.as_mut().expect("entry");
        for _ in 0..4 {
            entry.move_up();
        }

        let drawn = lines(&prompt.document(40, 6));

        assert_eq!(drawn.len(), 6);
        // The cursor went back to the first line, so that is the line shown —
        // sitting at its end, where the block marks it.
        assert_eq!(drawn[5], "│ one█");
    }

    #[test]
    fn opens_the_writing_region_with_a_rule_of_its_own() {
        let prompt = Prompt::entry("Comment");

        let drawn = lines(&prompt.document(80, 10));

        // One rule opens the panel, a second says where the writing begins.
        let rules: Vec<_> = drawn
            .iter()
            .enumerate()
            .filter(|(_, line)| line.starts_with('─'))
            .map(|(index, _)| index)
            .collect();
        assert_eq!(rules.len(), 2, "{drawn:#?}");
        assert_eq!(rules[0], 0, "{drawn:#?}");
        assert_eq!(drawn[1], "Comment");
        // The writing region opens directly under the second rule.
        assert_eq!(drawn[rules[1] + 1], "│ █", "{drawn:#?}");
        assert_eq!(drawn.len(), rules[1] + 2, "{drawn:#?}");
    }

    #[test]
    fn keeps_the_writing_on_screen_when_the_title_wraps_past_its_room() {
        let prompt = written("A title long enough to wrap several times over", "written");

        let drawn = lines(&prompt.document(20, 4));

        assert_eq!(drawn.len(), 4);
        assert_eq!(drawn[3], "│ written█");
    }

    #[test]
    fn keeps_the_end_of_a_long_comment_on_screen() {
        let prompt = written("Comment", "one\ntwo\nthree\nfour\nfive");

        let drawn = lines(&prompt.document(40, 6));

        assert_eq!(drawn.len(), 6);
        assert_eq!(drawn[5], "│ five█");
    }
}
