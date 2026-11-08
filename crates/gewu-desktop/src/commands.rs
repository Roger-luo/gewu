//! Commands run off the main thread (`async`), so Git and `gh` calls never
//! stall the window.

use gewu_app::{Action, Snapshot};
use gewu_workbench::{RepositoryRefs, TaskLog, WorkbenchAction, WorkbenchSnapshot};
use tauri::State;

use crate::state::AppState;

type Reply<T> = Result<T, String>;

fn text<T, E: ToString>(result: Result<T, E>) -> Reply<T> {
    result.map_err(|error| error.to_string())
}

#[tauri::command]
pub fn mode(state: State<'_, AppState>) -> &'static str {
    state.mode()
}

#[tauri::command(async)]
pub fn snapshot(state: State<'_, AppState>) -> Reply<Snapshot> {
    state.snapshot()
}

#[tauri::command(async)]
pub fn dispatch(state: State<'_, AppState>, action: Action) -> Reply<Snapshot> {
    state.dispatch(action)
}

#[tauri::command(async)]
pub fn workbench(state: State<'_, AppState>) -> Reply<WorkbenchSnapshot> {
    text(state.workbench()?.snapshot())
}

#[tauri::command(async)]
pub fn workbench_dispatch(
    state: State<'_, AppState>,
    action: WorkbenchAction,
) -> Reply<WorkbenchSnapshot> {
    text(state.workbench()?.dispatch(action))
}

#[tauri::command(async)]
pub fn repository_refs(state: State<'_, AppState>, repository_id: String) -> Reply<RepositoryRefs> {
    text(state.workbench()?.refs(&repository_id))
}

#[tauri::command(async)]
pub fn task_log(state: State<'_, AppState>, task_id: String, after: usize) -> Reply<TaskLog> {
    text(state.workbench()?.log(&task_id, after))
}

#[tauri::command(async)]
pub fn task_snapshot(state: State<'_, AppState>, task_id: String) -> Reply<Snapshot> {
    text(state.workbench()?.task_snapshot(&task_id))
}

#[tauri::command(async)]
pub fn task_dispatch(
    state: State<'_, AppState>,
    task_id: String,
    action: Action,
) -> Reply<Snapshot> {
    text(state.workbench()?.task_dispatch(&task_id, action))
}

/// A folder the reviewer picks, or `None` if they cancel.
#[tauri::command]
pub async fn pick_directory() -> Option<String> {
    rfd::AsyncFileDialog::new()
        .set_title("Add a Git repository")
        .pick_folder()
        .await
        .map(|folder| folder.path().display().to_string())
}
