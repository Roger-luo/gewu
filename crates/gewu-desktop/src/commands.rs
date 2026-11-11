//! Commands run off the main thread (`async`), so Git and `gh` calls never
//! stall the window.

use gewu_app::{Action, Snapshot};
use gewu_workbench::{
    Account, Contributor, GitHub, GitHubError, Publication, Receipt, RepositoryRefs, SignInMethod,
    SignInMethods, SignInPoll, SignInStart, TaskLog, WorkbenchAction, WorkbenchSnapshot,
};
use tauri::State;

use crate::{github, state::AppState};

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

/// The task's publication draft. Reading it does not contact GitHub.
#[tauri::command(async)]
pub fn publication(state: State<'_, AppState>, task_id: String) -> Reply<Publication> {
    text(state.workbench()?.publication(&task_id))
}

/// Sends the task's draft. The UI calls this only after the reviewer
/// confirms the preview.
#[tauri::command(async)]
pub fn publish(
    state: State<'_, AppState>,
    github: State<'_, GitHub>,
    task_id: String,
) -> Reply<Receipt> {
    let workbench = state.workbench()?;
    text(github.with_transport(|transport| workbench.publish(&task_id, transport)))?
        .map_err(|error| error.to_string())
}

/// Who a note on the task can `@`-mention: the repository's contributors.
/// Empty when signed out.
#[tauri::command(async)]
pub fn contributors(
    state: State<'_, AppState>,
    github: State<'_, GitHub>,
    task_id: String,
) -> Reply<Vec<Contributor>> {
    let workbench = state.workbench()?;
    match github.with_transport(|transport| workbench.contributors(&task_id, transport)) {
        Ok(listed) => text(listed),
        Err(GitHubError::SignedOut) => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

/// Who publishing would post as, or `None`.
#[tauri::command(async)]
pub fn github_account(github: State<'_, GitHub>) -> Reply<Option<Account>> {
    text(github.account())
}

/// Starts a sign-in: a device code to approve on github.com, or a request
/// for a pasted token when Gewu uses the GitHub CLI.
#[tauri::command(async)]
pub fn github_sign_in_start(github: State<'_, GitHub>) -> Reply<SignInStart> {
    text(github::start(&github))
}

/// Checks once whether the reviewer approved the device code.
#[tauri::command(async)]
pub fn github_sign_in_poll(github: State<'_, GitHub>) -> Reply<SignInPoll> {
    text(github.poll())
}

/// Signs in with a pasted token (the GitHub CLI fallback).
#[tauri::command(async)]
pub fn github_sign_in(github: State<'_, GitHub>, token: String) -> Reply<Option<Account>> {
    text(github.sign_in_with_token(&token))
}

/// Forgets Gewu's token, or stops using the GitHub CLI's login.
#[tauri::command(async)]
pub fn github_sign_out(github: State<'_, GitHub>) -> Reply<()> {
    text(github.sign_out())
}

/// The sign-in method in use, and whether `gh` is signed in.
#[tauri::command(async)]
pub fn github_methods(github: State<'_, GitHub>) -> Reply<SignInMethods> {
    text(github.methods())
}

/// Switches between Gewu's OAuth app and the GitHub CLI.
#[tauri::command(async)]
pub fn github_use_method(github: State<'_, GitHub>, method: SignInMethod) -> Reply<SignInMethods> {
    text(github.use_method(method))
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
