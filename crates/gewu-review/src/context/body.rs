/// Borrowed index of the same normalized body lines used by `Change::body`.
/// Build once per accessed change; page reads allocate only the output window.
pub(super) struct BodyIndex<'a> {
    lines: Vec<&'a str>,
    starts: Vec<usize>,
}

impl<'a> BodyIndex<'a> {
    pub fn new(patch: &'a str) -> Self {
        let mut lines: Vec<_> = patch
            .lines()
            .skip(usize::from(patch.starts_with("@@ ")))
            .collect();
        if lines.is_empty() {
            lines.push("");
        }
        let mut offset = 0;
        let starts = lines
            .iter()
            .map(|line| {
                let start = offset;
                offset += line.len() + 1;
                start
            })
            .collect();
        Self { lines, starts }
    }

    pub fn range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        Some((
            *self.starts.get(start.checked_sub(1)?)?,
            self.starts.get(end.checked_sub(1)?)? + self.lines.get(end - 1)?.len(),
        ))
    }

    pub(super) fn lines_from(&self, start: usize) -> impl Iterator<Item = (usize, &str)> {
        self.lines.iter().copied().enumerate().skip(start)
    }

    /// A separator belongs to the preceding line at its length in bytes.
    pub fn location(&self, offset: usize) -> (usize, usize) {
        let line = self
            .starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        (line, offset - self.starts[line])
    }

    pub fn is_char_boundary(&self, offset: usize) -> bool {
        let (line, byte) = self.location(offset);
        self.lines[line].is_char_boundary(byte)
    }

    pub fn window(&self, start: usize, end: usize, max_bytes: usize) -> String {
        let mut output = String::new();
        let (mut line, mut byte) = self.location(start);
        let target = (end - start).min(max_bytes);
        while output.len() < target {
            let text = &self.lines[line][byte..];
            let mut take = text.len().min(target - output.len());
            while !text.is_char_boundary(take) {
                take -= 1;
            }
            output.push_str(&text[..take]);
            if take < text.len() || output.len() == target {
                break;
            }
            output.push('\n');
            line += 1;
            byte = 0;
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::BodyIndex;

    #[test]
    fn normalizes_only_as_the_existing_quote_body_does() {
        let body = BodyIndex::new("@@ header\r\n+a\r\n+λ\r\n\r\n");
        let (start, end) = body.range(1, 3).unwrap();
        assert_eq!(body.window(start, end, 100), "+a\n+λ\n");
        assert_eq!(body.location(2), (0, 2));
        assert_eq!(body.location(3), (1, 0));
        assert!(!body.is_char_boundary(5));
        assert_eq!(body.window(3, end, 2), "+");
    }

    #[test]
    fn huge_line_is_borrowed_and_random_access_copies_only_the_window() {
        let patch = format!("+{}", "a".repeat(4 * 1024 * 1024));
        let body = BodyIndex::new(&patch);
        assert_eq!(body.lines[0].as_ptr(), patch.as_ptr());
        assert_eq!(body.lines.len(), 1);
        for offset in [0, 1_000_000, 4_000_000] {
            let window = body.window(offset, patch.len(), 1024);
            assert_eq!(window.len(), 1024);
            assert_eq!(window, patch[offset..offset + 1024]);
        }
    }
}
