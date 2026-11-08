use std::sync::Mutex;

use gewu_app::{Action, ReviewWorkspace, Snapshot};
use gewu_workbench::Workbench;

/// Managed Tauri state: one review workspace, or the workbench.
pub enum AppState {
    Review(Box<Mutex<ReviewWorkspace>>),
    Workbench(Workbench),
}

impl AppState {
    pub fn mode(&self) -> &'static str {
        match self {
            Self::Review(_) => "review",
            Self::Workbench(_) => "workbench",
        }
    }

    pub fn workbench(&self) -> Result<&Workbench, String> {
        match self {
            Self::Workbench(workbench) => Ok(workbench),
            Self::Review(_) => Err("Gewu was opened on a single review".to_owned()),
        }
    }

    pub fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(self.workspace()?.snapshot())
    }

    pub fn dispatch(&self, action: Action) -> Result<Snapshot, String> {
        self.workspace()?
            .dispatch(action)
            .map_err(|error| error.to_string())
    }

    fn workspace(&self) -> Result<std::sync::MutexGuard<'_, ReviewWorkspace>, String> {
        match self {
            Self::Review(workspace) => workspace
                .lock()
                .map_err(|_| "workspace lock poisoned".to_owned()),
            Self::Workbench(_) => Err("open a review task first".to_owned()),
        }
    }
}
