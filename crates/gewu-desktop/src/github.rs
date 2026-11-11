//! Which GitHub the desktop app signs in to and publishes through.
//!
//! The reviewer picks the sign-in in the window: Gewu's OAuth app (a device
//! code, the token in the system keychain) or the GitHub CLI's own login.
//! The choice is kept in the workbench home's `github.json`.
//!
//! - `GEWU_GITHUB=gh`: start on the GitHub CLI when nothing is saved.
//! - `GEWU_GITHUB=record`: the [`Recorder`], which sends nothing.
//! - `GEWU_GITHUB_CLIENT_ID` replaces Gewu's OAuth client ID.

use gewu_workbench::{
    GEWU_CLIENT_ID, GhCli, GitHub, GitHubError, Home, OAuthSession, Recorder, SignInMethod,
    SignInStart,
};

pub fn from_env() -> Result<GitHub, String> {
    let wanted = std::env::var("GEWU_GITHUB");
    if wanted.as_deref() == Ok("record") {
        eprintln!("GEWU_GITHUB=record: publishing is recorded, nothing is sent to GitHub.");
        return Ok(GitHub::recorded(Recorder::default()));
    }
    let method = if wanted.as_deref() == Ok("gh") {
        SignInMethod::Cli
    } else {
        SignInMethod::Gewu
    };
    let client_id =
        std::env::var("GEWU_GITHUB_CLIENT_ID").unwrap_or_else(|_| GEWU_CLIENT_ID.to_string());
    let oauth = OAuthSession::open(client_id)
        .map_err(|error| format!("Could not open the GitHub sign-in:\n\n{error}"))?;
    // `gh` on the same search path agents use, since an app opened from the
    // Finder does not inherit the shell's `PATH`.
    let cli = GhCli::new(gewu_agent::Preset::which("gh").unwrap_or_else(|| "gh".into()));
    let saved = Home::from_env().root().join("github.json");
    Ok(GitHub::live(oauth, cli, method, Some(saved)))
}

/// Starts a sign-in; a device code also opens GitHub's approval page.
pub fn start(github: &GitHub) -> Result<SignInStart, GitHubError> {
    let start = github.start()?;
    if let SignInStart::Device {
        verification_uri, ..
    } = &start
    {
        open_in_browser(verification_uri);
    }
    Ok(start)
}

/// Best effort: the dialog also shows the address and the code.
fn open_in_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(url).spawn();
}
