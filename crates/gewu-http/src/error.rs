use std::io;

use gewu_app::AppError;
use thiserror::Error;

/// Failures starting or running the HTTP server.
#[derive(Debug, Error)]
pub enum HttpError {
    #[error(transparent)]
    App(#[from] AppError),
    #[error(transparent)]
    Workbench(#[from] gewu_workbench::WorkbenchError),
    #[error("could not bind HTTP server: {0}")]
    Bind(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("static directory `{0}` is not a directory")]
    BadStatic(std::path::PathBuf),
}
