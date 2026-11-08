use gewu_review::{Change, ReviewInput, SourceIdentity};

/// Header lines that describe a change no hunk can express.
const NOTABLE: [&str; 8] = [
    "old mode ",
    "new mode ",
    "rename from ",
    "rename to ",
    "copy from ",
    "copy to ",
    "Binary files ",
    "GIT binary patch",
];

/// Header lines worth showing alongside a notable one, but not on their own.
const SUPPORTING: [&str; 4] = [
    "new file mode ",
    "deleted file mode ",
    "similarity index ",
    "dissimilarity index ",
];

pub(super) struct DiffParser<'a> {
    source: SourceIdentity,
    lines: std::iter::Peekable<std::str::Lines<'a>>,
    section: FileSection,
    max_chunk_lines: usize,
}

impl<'a> DiffParser<'a> {
    pub(super) fn new(source: SourceIdentity, diff: &'a str, max_chunk_lines: usize) -> Self {
        Self {
            source,
            lines: diff.lines().peekable(),
            section: FileSection::default(),
            max_chunk_lines,
        }
    }

    pub(super) fn parse(mut self) -> ReviewInput {
        let mut changes = Vec::new();
        while let Some(line) = self.lines.next() {
            if let Some(header) = line.strip_prefix("diff --git ") {
                self.close_section(&mut changes);
                self.section = FileSection::new(header);
                continue;
            }
            if self.section.note(line) {
                continue;
            }
            if !line.starts_with("@@ ") {
                continue;
            }
            let Some(ranges) = Self::ranges(line) else {
                continue;
            };
            let mut patch = String::from(line);
            patch.push('\n');
            while self
                .lines
                .peek()
                .is_some_and(|next| !next.starts_with("@@ ") && !next.starts_with("diff --git "))
            {
                patch.push_str(self.lines.next().expect("peeked line exists"));
                patch.push('\n');
            }
            self.section.hunks += 1;
            if self.section.hunks == 1 {
                Self::emit_metadata(&mut self.section, &mut changes);
            }
            changes.push(Change::new(self.section.path(), ranges, patch));
        }
        self.close_section(&mut changes);
        // Hunks reach the manifest whole. Deciding where a view begins and ends
        // is the agent's job, because that boundary belongs to the explanation,
        // not to a row counter.
        ReviewInput::sized(self.source, changes, self.max_chunk_lines)
    }

    fn close_section(&mut self, changes: &mut Vec<Change>) {
        Self::emit_metadata(&mut self.section, changes);
    }

    /// Records a change that hunks cannot express: a binary edit, a mode
    /// transition, a rename, or a file whose diff carries no hunk at all.
    fn emit_metadata(section: &mut FileSection, changes: &mut Vec<Change>) {
        if section.emitted || !section.started {
            return;
        }
        section.emitted = true;
        if !section.notable && section.hunks > 0 {
            return;
        }
        let path = section.path();
        if path.is_empty() {
            return;
        }
        let mut patch = if section.notes.is_empty() {
            String::from("changed without a content diff")
        } else {
            section.notes.join("\n")
        };
        patch.push('\n');
        changes.push(Change::new(path, (0, 0, 0, 0), patch));
    }

    fn ranges(header: &str) -> Option<(u32, u32, u32, u32)> {
        let mut fields = header.split_whitespace();
        fields.next()?;
        let old = Self::range(fields.next()?.strip_prefix('-')?)?;
        let new = Self::range(fields.next()?.strip_prefix('+')?)?;
        Some((old.0, old.1, new.0, new.1))
    }

    fn range(value: &str) -> Option<(u32, u32)> {
        let (start, lines) = value.split_once(',').unwrap_or((value, "1"));
        Some((start.parse().ok()?, lines.parse().ok()?))
    }
}

/// The `diff --git` section for a single file.
#[derive(Default)]
struct FileSection {
    started: bool,
    emitted: bool,
    notable: bool,
    hunks: usize,
    old_path: String,
    new_path: String,
    notes: Vec<String>,
}

impl FileSection {
    fn new(header: &str) -> Self {
        let (old_path, new_path) = Self::header_paths(header);
        Self {
            started: true,
            old_path,
            new_path,
            ..Self::default()
        }
    }

    /// The path a reviewer should see. Deletions name no `+++` path, so they
    /// fall back to the removed file rather than inheriting the previous
    /// section's path.
    fn path(&self) -> &str {
        if self.new_path.is_empty() {
            &self.old_path
        } else {
            &self.new_path
        }
    }

    /// Consumes a file-header line, returning whether it belonged to the header.
    fn note(&mut self, line: &str) -> bool {
        if !self.started {
            return false;
        }
        if let Some(value) = line.strip_prefix("--- ") {
            self.old_path = Self::side(value, "a/");
            return true;
        }
        if let Some(value) = line.strip_prefix("+++ ") {
            self.new_path = Self::side(value, "b/");
            return true;
        }
        if line.starts_with("index ") {
            return true;
        }
        if NOTABLE.iter().any(|prefix| line.starts_with(prefix)) {
            self.notable = true;
            self.notes.push(line.to_owned());
            return true;
        }
        if SUPPORTING.iter().any(|prefix| line.starts_with(prefix)) {
            self.notes.push(line.to_owned());
            return true;
        }
        false
    }

    /// Splits `a/<old> b/<new>`, preferring the unambiguous equal-path form
    /// before falling back to the first ` b/` separator.
    fn header_paths(header: &str) -> (String, String) {
        let middle = header.len() / 2;
        if header.len() % 2 == 1 && header.as_bytes()[middle] == b' ' {
            let (left, right) = header.split_at(middle);
            let (left, right) = (Self::side(left, "a/"), Self::side(&right[1..], "b/"));
            if !left.is_empty() && left == right {
                return (left, right);
            }
        }
        match header.find(" b/") {
            Some(index) => (
                Self::side(&header[..index], "a/"),
                Self::side(&header[index + 1..], "b/"),
            ),
            None => (String::new(), String::new()),
        }
    }

    fn side(value: &str, prefix: &str) -> String {
        value.strip_prefix(prefix).unwrap_or_default().to_owned()
    }
}
