//! Who a note can `@`-mention.

use gewu_github::{Contributor, Transport};

use super::Workbench;
use crate::WorkbenchError;

impl Workbench {
    /// The contributors of the GitHub repository `task_id` publishes to,
    /// read from GitHub once per run and then from memory. Empty when the
    /// repository is not on GitHub or nobody is signed in.
    pub fn contributors(
        &self,
        task_id: &str,
        transport: &dyn Transport,
    ) -> Result<Vec<Contributor>, WorkbenchError> {
        let Some(slug) = self.remote(task_id)?.slug else {
            return Ok(Vec::new());
        };
        let key = slug.to_string();
        if let Some(cached) = self.cache().get(&key) {
            return Ok(cached.clone());
        }
        let contributors = Contributor::list(transport, &slug)?;
        // Signed out reads as empty; keep asking so signing in fills it.
        if !contributors.is_empty() {
            self.cache().insert(key, contributors.clone());
        }
        Ok(contributors)
    }

    fn cache(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, Vec<Contributor>>> {
        self.inner
            .contributors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
