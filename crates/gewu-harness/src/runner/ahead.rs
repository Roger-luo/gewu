//! Changing how far a started run writes ahead.

use gewu_review::harness::WriteAhead;

use super::HarnessRunner;
use crate::Result;

impl HarnessRunner {
    /// Has a run already started write `ahead` of the reviewer from its next
    /// step on, so a reviewer who changes the setting mid-review gets it
    /// without starting again. Does nothing before a run starts.
    pub fn update_write_ahead(&self, ahead: WriteAhead) -> Result<()> {
        if let Some(state) = self.store.state()?
            && state.write_ahead() != ahead
        {
            self.store.save_state(&state.writing_ahead(ahead))?;
        }
        Ok(())
    }
}
