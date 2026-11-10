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
    #[error("GitHub sent a reply Gewu could not read: {0}")]
    Reply(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}
