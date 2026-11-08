/// Text a reviewer is writing, and where their cursor sits in it.
///
/// The cursor is a byte offset that always lands on a character boundary, so
/// every move steps over whole characters and slicing around it is safe.
#[derive(Debug, Default)]
pub(crate) struct Entry {
    text: String,
    cursor: usize,
}

impl Entry {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// The text before the cursor, the character under it, and the text after
    /// that character — what a renderer needs to draw the cursor in place.
    pub(crate) fn split(&self) -> (&str, Option<char>, &str) {
        let under = self.text[self.cursor..].chars().next();
        let after = self.cursor + under.map_or(0, char::len_utf8);
        (&self.text[..self.cursor], under, &self.text[after..])
    }

    pub(crate) fn insert(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    pub(crate) fn insert_char(&mut self, character: char) {
        self.text.insert(self.cursor, character);
        self.cursor += character.len_utf8();
    }

    pub(crate) fn backspace(&mut self) {
        let start = self.previous();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub(crate) fn delete(&mut self) {
        let end = self.next();
        self.text.replace_range(self.cursor..end, "");
    }

    pub(crate) fn delete_word_left(&mut self) {
        let start = self.word_left();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub(crate) fn delete_to_line_start(&mut self) {
        let start = self.line_start();
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub(crate) fn delete_to_line_end(&mut self) {
        let end = self.line_end();
        self.text.replace_range(self.cursor..end, "");
    }

    pub(crate) fn move_left(&mut self) {
        self.cursor = self.previous();
    }

    pub(crate) fn move_right(&mut self) {
        self.cursor = self.next();
    }

    pub(crate) fn move_word_left(&mut self) {
        self.cursor = self.word_left();
    }

    pub(crate) fn move_word_right(&mut self) {
        self.cursor = self.word_right();
    }

    pub(crate) fn move_line_start(&mut self) {
        self.cursor = self.line_start();
    }

    pub(crate) fn move_line_end(&mut self) {
        self.cursor = self.line_end();
    }

    /// Moves to the line above, holding the column where it can. A shorter line
    /// puts the cursor at its end, which is where the next character would go.
    pub(crate) fn move_up(&mut self) {
        let start = self.line_start();
        if start == 0 {
            self.cursor = 0;
            return;
        }
        let column = self.text[start..self.cursor].chars().count();
        let previous = self.text[..start.saturating_sub(1)]
            .rfind('\n')
            .map_or(0, |at| at + 1);
        self.cursor = self.column_in(previous, start.saturating_sub(1), column);
    }

    pub(crate) fn move_down(&mut self) {
        let end = self.line_end();
        if end == self.text.len() {
            self.cursor = self.text.len();
            return;
        }
        let column = self.text[self.line_start()..self.cursor].chars().count();
        let next = end + 1;
        let following = self.text[next..]
            .find('\n')
            .map_or(self.text.len(), |at| next + at);
        self.cursor = self.column_in(next, following, column);
    }

    /// The offset `column` characters into a line, or its end if the line is
    /// shorter than that.
    fn column_in(&self, start: usize, end: usize, column: usize) -> usize {
        self.text[start..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(offset, _)| start + offset)
    }

    fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |at| at + 1)
    }

    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |at| self.cursor + at)
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .chars()
            .next_back()
            .map_or(0, |character| self.cursor - character.len_utf8())
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |character| self.cursor + character.len_utf8())
    }

    /// The start of the word before the cursor: whitespace is stepped over
    /// first, so a cursor sitting after a space still reaches the word.
    fn word_left(&self) -> usize {
        let head = &self.text[..self.cursor];
        let trimmed = head.trim_end_matches(|character: char| character.is_whitespace());
        trimmed.rfind(char::is_whitespace).map_or(0, |at| {
            at + self.text[at..].chars().next().map_or(1, char::len_utf8)
        })
    }

    fn word_right(&self) -> usize {
        let tail = &self.text[self.cursor..];
        let skipped = tail.len() - tail.trim_start_matches(char::is_whitespace).len();
        let rest = &tail[skipped..];
        self.cursor + skipped + rest.find(char::is_whitespace).unwrap_or(rest.len())
    }
}

impl From<&str> for Entry {
    fn from(text: &str) -> Self {
        Self {
            cursor: text.len(),
            text: text.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Entry;

    /// Writes the entry with the cursor shown as `|`, so a test reads as what
    /// the reviewer would see.
    fn shown(entry: &Entry) -> String {
        let (before, under, after) = entry.split();
        let under: String = under.map(String::from).unwrap_or_default();
        format!("{before}|{under}{after}")
    }

    fn entry(text: &str) -> Entry {
        Entry::from(text)
    }

    #[test]
    fn a_new_entry_takes_the_cursor_to_the_end() {
        assert_eq!(shown(&entry("written")), "written|");
    }

    #[test]
    fn steps_over_characters_in_both_directions() {
        let mut entry = entry("ab");
        entry.move_left();
        assert_eq!(shown(&entry), "a|b");
        entry.move_left();
        assert_eq!(shown(&entry), "|ab");
        // The start holds; there is nowhere further left to go.
        entry.move_left();
        assert_eq!(shown(&entry), "|ab");
        entry.move_right();
        entry.move_right();
        entry.move_right();
        assert_eq!(shown(&entry), "ab|");
    }

    #[test]
    fn steps_over_whole_characters_not_bytes() {
        let mut entry = entry("héllo");
        for _ in 0..4 {
            entry.move_left();
        }
        assert_eq!(shown(&entry), "h|éllo");
        entry.backspace();
        assert_eq!(shown(&entry), "|éllo");
    }

    #[test]
    fn writes_and_erases_at_the_cursor_rather_than_the_end() {
        let mut entry = entry("ac");
        entry.move_left();
        entry.insert_char('b');
        assert_eq!(shown(&entry), "ab|c");
        entry.delete();
        assert_eq!(shown(&entry), "ab|");
        entry.backspace();
        assert_eq!(shown(&entry), "a|");
    }

    #[test]
    fn moves_between_lines_holding_the_column() {
        let mut entry = entry("first\nsecond");
        entry.move_line_start();
        entry.move_right();
        entry.move_right();
        assert_eq!(shown(&entry), "first\nse|cond");
        entry.move_up();
        assert_eq!(shown(&entry), "fi|rst\nsecond");
        entry.move_down();
        assert_eq!(shown(&entry), "first\nse|cond");
    }

    #[test]
    fn a_shorter_line_above_takes_the_cursor_to_its_end() {
        let mut entry = entry("ab\nlonger");
        entry.move_line_start();
        for _ in 0..5 {
            entry.move_right();
        }
        entry.move_up();
        assert_eq!(shown(&entry), "ab|\nlonger");
    }

    #[test]
    fn leaving_the_first_line_upward_goes_to_the_start() {
        let mut entry = entry("only");
        entry.move_left();
        entry.move_up();
        assert_eq!(shown(&entry), "|only");
    }

    #[test]
    fn leaving_the_last_line_downward_goes_to_the_end() {
        let mut entry = entry("only");
        entry.move_line_start();
        entry.move_down();
        assert_eq!(shown(&entry), "only|");
    }

    #[test]
    fn home_and_end_stay_within_the_line_the_cursor_is_on() {
        let mut entry = entry("first\nsecond");
        entry.move_line_start();
        assert_eq!(shown(&entry), "first\n|second");
        entry.move_line_end();
        assert_eq!(shown(&entry), "first\nsecond|");
        entry.move_up();
        entry.move_line_start();
        assert_eq!(shown(&entry), "|first\nsecond");
        entry.move_line_end();
        assert_eq!(shown(&entry), "first|\nsecond");
    }

    #[test]
    fn steps_over_words() {
        let mut entry = entry("one two three");
        entry.move_word_left();
        assert_eq!(shown(&entry), "one two |three");
        entry.move_word_left();
        assert_eq!(shown(&entry), "one |two three");
        entry.move_word_right();
        assert_eq!(shown(&entry), "one two| three");
        entry.move_word_right();
        assert_eq!(shown(&entry), "one two three|");
    }

    #[test]
    fn erases_the_word_before_the_cursor() {
        let mut written = entry("one two");
        written.delete_word_left();
        assert_eq!(shown(&written), "one |");
    }

    #[test]
    fn erases_to_the_start_and_the_end_of_the_line_the_cursor_is_on() {
        let mut written = entry("first\nsecond");
        written.move_left();
        written.delete_to_line_start();
        assert_eq!(shown(&written), "first\n|d");

        let mut written = entry("first\nsecond");
        written.move_line_start();
        written.delete_to_line_end();
        assert_eq!(shown(&written), "first\n|");
    }

    #[test]
    fn a_paste_lands_at_the_cursor() {
        let mut entry = entry("ab");
        entry.move_left();
        entry.insert("one\ntwo");
        assert_eq!(shown(&entry), "aone\ntwo|b");
    }
}
