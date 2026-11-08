//! HTTP bridge for the Gewu app protocol.
//!
//! Serves either one prepared review or the whole workbench as JSON, plus a
//! static web UI directory. Bind to loopback only.

mod error;
mod response;
mod routes;
mod server;
mod static_files;

pub use error::HttpError;
pub use server::{Mode, ServeOptions, Server};
