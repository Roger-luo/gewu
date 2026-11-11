//! Items of tests Gewu writes itself, unless the run explains tests: the
//! harness expands them from the test code and moves on, so no agent spends
//! time walking through tests line by line.

use gewu_review::harness::{Expansion, WorkList, test_expansion};

use super::{HarnessRunner, Run};

impl HarnessRunner {
    /// Whether the next item is one Gewu writes itself, so a host can call
    /// `next` rather than start an agent only to run it.
    pub fn writes_next(&self) -> crate::Result<bool> {
        let run = self.run()?;
        if run.state.stage != gewu_review::harness::HarnessStage::Expand {
            return Ok(false);
        }
        let work = self.work_list()?;
        Ok(matches!(self.store.expansion(), Ok(None)) && self.written_tests(&run, &work).is_some())
    }

    /// The expansion Gewu writes for the next item, when it is an item of
    /// tests this run does not explain and its code splits into chunks that
    /// fit. Otherwise an agent writes the item.
    pub(super) fn written_tests(&self, run: &Run, work: &WorkList) -> Option<Expansion> {
        let item = work.next_item()?;
        if !item.tests || run.state.explain_tests {
            return None;
        }
        test_expansion(&run.input, item, self.max_chunks)
    }
}

/// What an agent that reaches an item Gewu writes is told to do.
pub(super) fn written_prompt(item_id: &str) -> String {
    format!(
        "# Gewu harness: expand `{item_id}`\n\nThis item only walks through tests, and Gewu \
         writes it itself from the test code. Run `gewu harness next`; do not write \
         `.gewu/harness/expansion.json` for it.\n"
    )
}

#[cfg(test)]
mod tests {
    use gewu_review::{ChunkKind, harness::ReviewOrigin};

    use super::super::tests::{Fixture, fixture};

    /// Starts a run and plans it with the first item marked as tests, then
    /// returns what accepting the plan asked for.
    fn plan_tests_first(fixture: &Fixture) -> String {
        fixture
            .runner
            .start(
                "review-test".into(),
                fixture.input.clone(),
                ReviewOrigin {
                    base: "HEAD".into(),
                    head: Some("HEAD".into()),
                    working_tree: false,
                },
            )
            .expect("start");
        fixture
            .runner
            .store
            .save_investigation(&fixture.investigation())
            .expect("save investigation");
        fixture.runner.next().expect("advance to plan");
        let mut list = fixture.work_list();
        list.items[0].tests = true;
        fixture
            .runner
            .store
            .save_work_list(&list)
            .expect("save work list");
        fixture.runner.next().expect("accept the plan")
    }

    /// Asked for after timing Cursor on #276: a third of the agent's time
    /// went to walking through tests.
    #[test]
    fn an_item_of_tests_is_written_without_the_agent_unless_tests_are_explained() {
        let mut quiet = fixture();
        quiet.runner.explain_tests = false;
        let after = plan_tests_first(&quiet);
        assert!(
            after.starts_with("# Gewu harness: waiting for the reviewer"),
            "{after}"
        );
        let spec = quiet.spec();
        assert_eq!(spec.chunks.len(), 1);
        assert_eq!(spec.chunks[0].kind, ChunkKind::Test);
        assert_eq!(spec.chunks[0].id, "item-0-tests-1");

        let explained = fixture();
        let after = plan_tests_first(&explained);
        assert!(
            after.starts_with("# Gewu harness: expand `item-0`"),
            "{after}"
        );
        assert!(!after.contains("Gewu writes it itself"), "{after}");
    }
}
