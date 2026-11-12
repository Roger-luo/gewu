//! Starting a review again: the same repository, source, and agent, prepared
//! by this Gewu. A review an older Gewu prepared lacks what preparation has
//! added since, and nothing short of preparing it again gives it.

use super::Workbench;
use crate::WorkbenchError;

impl Workbench {
    /// Creates the replacement first, so a source that no longer resolves
    /// leaves the old review in place; a review of uncommitted changes must
    /// go first, since a repository holds only one.
    pub(super) fn start_again(&self, task_id: &str) -> Result<(), WorkbenchError> {
        let record = self.state()?.task(task_id)?.record.clone();
        let title = record.titled.then(|| record.title.clone());
        let in_place = record.source.in_place();
        if in_place {
            self.delete_task(task_id)?;
        }
        self.create_task(record.repository_id, record.source, record.agent, title)?;
        if !in_place {
            self.delete_task(task_id)?;
        }
        Ok(())
    }
}
