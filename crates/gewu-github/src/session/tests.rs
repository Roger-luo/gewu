use super::{CliState, GitHub, SignInMethod};
use crate::{GitHubError, Recorder, SignInStart};

#[test]
fn the_sandbox_switches_between_its_two_sign_ins() {
    let github = GitHub::recorded(Recorder::default());
    let methods = github.settings().expect("methods");
    assert_eq!(methods.preferences.method, SignInMethod::Gewu);
    assert_eq!(
        methods.cli,
        CliState::SignedIn {
            login: Recorder::SANDBOX_LOGIN.into()
        }
    );
    assert_eq!(github.account().expect("account"), None);

    github.use_method(SignInMethod::Cli).expect("use gh");
    let account = github.account().expect("account").expect("signed in");
    assert_eq!(account.login, Recorder::SANDBOX_LOGIN);
    assert_eq!(github.start().expect("start"), SignInStart::Token);

    github.sign_out().expect("leave gh");
    assert_eq!(github.method(), SignInMethod::Gewu);
    assert_eq!(github.account().expect("account"), None);
    assert!(matches!(
        github.start().expect("start"),
        SignInStart::Device { .. }
    ));
}

#[test]
fn a_pasted_token_is_only_for_the_cli() {
    let github = GitHub::recorded(Recorder::default());
    assert!(matches!(
        github.sign_in_with_token("ghp_example"),
        Err(GitHubError::SignIn(_))
    ));
}

#[cfg(unix)]
mod live {
    use std::os::unix::fs::PermissionsExt;

    use super::super::{CliState, GitHub, Preferences, SignInMethod};
    use crate::{GhCli, GitHubError, LoginEndpoint, MemoryStore, OAuthSession};

    struct NoLogin;
    impl LoginEndpoint for NoLogin {
        fn post(&self, _: &str, _: &[(&str, &str)]) -> Result<serde_json::Value, GitHubError> {
            panic!("these tests never sign in through github.com")
        }
    }

    fn oauth() -> OAuthSession {
        OAuthSession::with_parts(
            "client",
            Box::new(NoLogin),
            Box::new(MemoryStore::default()),
            Box::new(|_| panic!("signed out: no API is made")),
        )
    }

    /// A `gh` that answers `gh api /user` as `login`.
    fn gh(dir: &std::path::Path, login: &str) -> GhCli {
        let script = dir.join("gh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s' '{{\"login\":\"{login}\",\"avatar_url\":\"a\",\"html_url\":\"h\"}}'\n"
            ),
        )
        .expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        GhCli::new(script)
    }

    #[test]
    fn offers_a_signed_in_cli_and_remembers_the_choice() {
        let dir = tempfile::tempdir().expect("dir");
        let saved = dir.path().join("home/github.json");
        let open = || {
            GitHub::live(
                oauth(),
                gh(dir.path(), "Roger-luo"),
                Preferences::default(),
                Some(saved.clone()),
            )
        };

        let github = open();
        let methods = github.settings().expect("methods");
        assert_eq!(
            methods.cli,
            CliState::SignedIn {
                login: "Roger-luo".into()
            }
        );
        assert_eq!(
            github.account().expect("account"),
            None,
            "no Gewu token yet"
        );

        github.use_method(SignInMethod::Cli).expect("use gh");
        let account = github.account().expect("account").expect("gh account");
        assert_eq!(account.login, "Roger-luo");

        let reopened = open();
        assert_eq!(
            reopened.method(),
            SignInMethod::Cli,
            "the choice outlives the run"
        );
    }

    #[test]
    fn a_missing_cli_is_reported_not_an_error() {
        let dir = tempfile::tempdir().expect("dir");
        let github = GitHub::live(
            oauth(),
            GhCli::new(dir.path().join("no-such-gh")),
            Preferences::default(),
            None,
        );
        assert_eq!(github.settings().expect("methods").cli, CliState::Missing);
    }
}

#[test]
fn the_sandbox_always_rehearses_and_keeps_preferences() {
    use super::Preferences;
    let github = GitHub::recorded(Recorder::default());
    assert!(github.rehearsing(), "the sandbox never sends");
    let settings = github
        .set_preferences(Preferences {
            method: SignInMethod::Gewu,
            client_id: Some("  ".into()),
            rehearse: true,
        })
        .expect("save");
    assert_eq!(
        settings.preferences.client_id, None,
        "blank is Gewu's own app"
    );
    assert!(settings.preferences.rehearse);
    assert!(settings.sandbox);
    assert_eq!(settings.default_client_id, crate::GEWU_CLIENT_ID);
}

#[cfg(unix)]
#[test]
fn a_live_client_id_and_rehearsal_outlive_the_run() {
    use super::Preferences;
    use crate::{GhCli, MemoryStore, OAuthSession};
    let dir = tempfile::tempdir().expect("dir");
    let saved = dir.path().join("github.json");
    let open = || {
        let oauth = OAuthSession::with_parts(
            "default",
            Box::new(NoLogin),
            Box::new(MemoryStore::default()),
            Box::new(|_| panic!("signed out")),
        );
        let github = GitHub::live(
            oauth,
            GhCli::new(dir.path().join("no-gh")),
            Preferences::default(),
            Some(saved.clone()),
        );
        (github, ())
    };
    let (github, ()) = open();
    assert!(!github.rehearsing());
    github
        .set_preferences(Preferences {
            method: SignInMethod::Gewu,
            client_id: Some(" Ov23custom ".into()),
            rehearse: true,
        })
        .expect("save");
    let (reopened, ()) = open();
    let settings = reopened.settings().expect("settings");
    assert_eq!(
        settings.preferences.client_id.as_deref(),
        Some("Ov23custom")
    );
    assert!(reopened.rehearsing());
}

struct NoLogin;
impl crate::LoginEndpoint for NoLogin {
    fn post(&self, _: &str, _: &[(&str, &str)]) -> Result<serde_json::Value, GitHubError> {
        panic!("these tests never sign in through github.com")
    }
}

#[test]
fn the_sandbox_keeps_the_app_sign_in_across_a_switch() {
    let github = GitHub::recorded(Recorder::default());
    github.start().expect("start");
    github.poll().expect("approved");
    assert!(github.account().expect("account").is_some());
    github.use_method(SignInMethod::Cli).expect("to gh");
    github.use_method(SignInMethod::Gewu).expect("back");
    assert!(
        github.account().expect("account").is_some(),
        "the app's sign-in survives, as the keychain keeps its token"
    );
}
