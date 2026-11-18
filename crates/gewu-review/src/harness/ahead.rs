//! How far an agent may write ahead of the reviewer.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{HarnessState, WorkList};

/// Batches the agent may have written beyond the one the reviewer is
/// reading. Agents write faster than people read, so letting the agent run
/// ahead means the next batch is always ready; the cost is work a request
/// may later send back, because the plan was written before the reviewer
/// said anything about the code.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteAhead {
    /// Write the whole plan without waiting for the reviewer. Completing the
    /// review still waits for every chunk to be read.
    Unlimited,
    /// Wait once a chunk older than the latest this many batches is unread.
    /// `0` holds the agent level with the reviewer.
    Batches(u32),
}

impl Default for WriteAhead {
    /// Level with the reviewer, as runs started before writing ahead existed.
    fn default() -> Self {
        Self::Batches(0)
    }
}

impl WriteAhead {
    /// The most batches a reviewer may set: beyond it, `Unlimited` says the
    /// same thing plainly.
    pub const MAX_BATCHES: u32 = 20;

    /// Whether `unread` chunks leave the agent free to write another batch.
    #[must_use]
    pub fn allows<'a>(self, work: &WorkList, mut unread: impl Iterator<Item = &'a str>) -> bool {
        match self {
            Self::Unlimited => true,
            Self::Batches(count) => {
                let recent = work.latest_batches(count);
                unread.all(|chunk| recent.contains(chunk))
            }
        }
    }
}

impl WorkList {
    /// The chunks of the last `count` batches written: the last `count`
    /// expanded items'.
    #[must_use]
    pub fn latest_batches(&self, count: u32) -> HashSet<&str> {
        self.items
            .iter()
            .rev()
            .filter(|item| item.expanded)
            .take(count as usize)
            .flat_map(|item| item.chunks.iter().map(String::as_str))
            .collect()
    }
}

impl HarnessState {
    /// How far this run's agent may write ahead. Runs that recorded only the
    /// older on/off switch wrote one batch ahead when it was on.
    #[must_use]
    pub fn write_ahead(&self) -> WriteAhead {
        self.ahead.unwrap_or(if self.write_ahead {
            WriteAhead::Batches(1)
        } else {
            WriteAhead::Batches(0)
        })
    }

    /// Lets the agent write `ahead` of the reviewer.
    #[must_use]
    pub fn writing_ahead(mut self, ahead: WriteAhead) -> Self {
        self.ahead = Some(ahead);
        self.write_ahead = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        SourceIdentity,
        harness::{HarnessState, ReviewOrigin},
    };

    use super::WriteAhead;

    fn state() -> HarnessState {
        let origin = ReviewOrigin {
            base: "HEAD".into(),
            head: None,
            working_tree: true,
        };
        HarnessState::new("r", SourceIdentity::new("b", "h", b""), origin)
    }

    #[test]
    fn runs_that_recorded_the_old_switch_keep_its_meaning() {
        let mut old = state();
        assert_eq!(old.write_ahead(), WriteAhead::Batches(0));
        old.write_ahead = true;
        assert_eq!(old.write_ahead(), WriteAhead::Batches(1));
        assert_eq!(
            old.writing_ahead(WriteAhead::Unlimited).write_ahead(),
            WriteAhead::Unlimited
        );
    }

    #[test]
    fn reads_and_writes_as_json_a_person_can_set() {
        assert_eq!(
            serde_json::to_string(&WriteAhead::Unlimited).unwrap(),
            "\"unlimited\""
        );
        assert_eq!(
            serde_json::to_string(&WriteAhead::Batches(3)).unwrap(),
            r#"{"batches":3}"#
        );
    }
}
