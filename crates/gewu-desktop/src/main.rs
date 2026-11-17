//! Thin Tauri shell: transport only. Review logic lives in `gewu-app`, and
//! repositories, tasks, and agents in `gewu-workbench`.

mod commands;
mod github;
mod root;
mod state;

use std::{env, sync::Mutex};

use gewu_app::ReviewWorkspace;
use gewu_workbench::{Home, Workbench};
use root::Launch;
use state::AppState;

fn main() {
    let state = AppState::open(Launch::from_env()).unwrap_or_else(|message| fail(&message));
    let github = github::from_env().unwrap_or_else(|message| fail(&message));
    tauri::Builder::default()
        .manage(state)
        .manage(github)
        .invoke_handler(tauri::generate_handler![
            commands::mode,
            commands::snapshot,
            commands::dispatch,
            commands::workbench,
            commands::workbench_dispatch,
            commands::repository_refs,
            commands::prompts,
            commands::task_log,
            commands::task_snapshot,
            commands::task_dispatch,
            commands::pick_directory,
            commands::publication,
            commands::publish,
            commands::contributors,
            commands::github_account,
            commands::github_sign_in,
            commands::github_sign_in_start,
            commands::github_sign_in_poll,
            commands::github_sign_out,
            commands::github_settings,
            commands::github_update_settings,
            commands::github_use_method,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| fail(&format!("Gewu failed to start:\n\n{error}")));
}

impl AppState {
    fn open(launch: Launch) -> Result<Self, String> {
        match launch {
            Launch::Review(root) => ReviewWorkspace::open(&root)
                .map(|workspace| Self::Review(Box::new(Mutex::new(workspace))))
                .map_err(|error| {
                    format!(
                        "Could not open the Gewu review at {}:\n\n{error}",
                        root.display()
                    )
                }),
            Launch::Workbench => {
                let exe = env::current_exe().map_err(|error| error.to_string())?;
                let gewu = Launch::gewu(&exe, env::var_os("GEWU_BIN").as_deref()).ok_or(
                    "Could not find the gewu binary agents run. Install it on your PATH or set \
                     GEWU_BIN.",
                )?;
                let home = Home::from_env();
                Workbench::open(home, &gewu)
                    .map(Self::Workbench)
                    .map_err(|error| format!("Could not open the Gewu workbench:\n\n{error}"))
            }
        }
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Gewu")
        .set_description(message)
        .show();
    std::process::exit(1);
}
