use std::io;

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("the GitHub CLI `gh` is not installed; install it to sign in")]
    MissingCli,
    #[error("not signed in to GitHub")]
    SignedOut,
    #[error("GitHub refused {method} {path}: {message}")]
    Refused {
        method: String,
        path: String,
        message: String,
    },
    #[error("could not reach GitHub: {0}")]
    Network(String),
    #[error("could not use the saved GitHub sign-in: {0}")]
    Keychain(String),
    #[error("GitHub sign-in failed: {0}")]
    SignIn(String),
    #[error("GitHub sent a reply Gewu could not read: {0}")]
    Reply(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}
