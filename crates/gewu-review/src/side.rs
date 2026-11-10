use serde::{Deserialize, Serialize};

use crate::Change;

/// Which file a line number counts in: `New` is the file after the change,
/// `Old` the file before it. A removed line has only an old-side number, so a
/// block of pure deletions can be commented on only through `Old`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    #[default]
    New,
    Old,
}

impl Side {
    /// The name stored in anchors, matching the `side` field of a GitHub
    /// review comment's counterpart: `new` is `RIGHT`, `old` is `LEFT`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Old => "old",
        }
    }
}

impl Change {
    /// The file line one 1-based hunk-body line represents on `side`. Lines
    /// absent from that side have no number.
    #[must_use]
    pub fn line_at(&self, body_line: usize, side: Side) -> Option<u32> {
        let (mut next, own, other) = match side {
            Side::New => (self.new_start, '+', '-'),
            Side::Old => (self.old_start, '-', '+'),
        };
        for (index, text) in self
            .patch
            .lines()
            .skip(usize::from(self.patch.starts_with("@@ ")))
            .enumerate()
        {
            let current = index + 1;
            if text.starts_with(other) {
                if current == body_line {
                    return None;
                }
                continue;
            }
            let line = text.starts_with([own, ' ']).then_some(next);
            if current == body_line {
                return line;
            }
            if line.is_some() {
                next = next.saturating_add(1);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::Side;
    use crate::Change;

    #[test]
    fn maps_removed_lines_to_the_old_side_only() {
        let change = Change::new(
            "src/lib.rs",
            (10, 2, 20, 3),
            "@@ -10,2 +20,3 @@\n context\n-removed\n+added\n+last\n",
        );

        assert_eq!(change.line_at(1, Side::Old), Some(10));
        assert_eq!(change.line_at(2, Side::Old), Some(11));
        assert_eq!(change.line_at(3, Side::Old), None);
        assert_eq!(change.line_at(4, Side::Old), None);
        assert_eq!(change.line_at(2, Side::New), None);
        assert_eq!(change.line_at(3, Side::New), Some(21));
        assert_eq!(change.line_at(4, Side::New), Some(22));
    }

    #[test]
    fn side_names_match_the_anchor_contract() {
        assert_eq!(Side::New.as_str(), "new");
        assert_eq!(Side::Old.as_str(), "old");
        assert_eq!(Side::default(), Side::New);
    }
}
