use super::{CliState, GitHub, SignInMethod};
use crate::{GitHubError, Recorder, SignInStart};

#[test]
fn the_sandbox_switches_between_its_two_sign_ins() {
    let github = GitHub::recorded(Recorder::default());
    let methods = github.methods().expect("methods");
    assert_eq!(methods.method, SignInMethod::Gewu);
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

    use super::super::{CliState, GitHub, SignInMethod};
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
                SignInMethod::Gewu,
                Some(saved.clone()),
            )
        };

        let github = open();
        let methods = github.methods().expect("methods");
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
            SignInMethod::Gewu,
            None,
        );
        assert_eq!(github.methods().expect("methods").cli, CliState::Missing);
    }
}
