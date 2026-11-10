//! GitHub access for a review: who is signed in, and sending a publication
//! plan once the reviewer confirms it.
//!
//! Every call goes through a [`Transport`]. [`GhCli`] is the real one and
//! uses the GitHub CLI's own sign-in, so Gewu never stores a token.
//! [`Recorder`] answers like GitHub without contacting it, which is what
//! tests and the UI sandbox use.

mod account;
mod error;
mod publish;
mod slug;
mod transport;

pub use account::Account;
pub use error::GitHubError;
pub use publish::{Posted, Receipt, publish};
pub use slug::Slug;
pub use transport::{GhCli, Recorder, Transport};
