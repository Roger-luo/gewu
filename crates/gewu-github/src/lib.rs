//! GitHub access for a review: who is signed in, and sending a publication
//! plan once the reviewer confirms it.
//!
//! Every call goes through a [`Transport`]. [`TokenApi`] calls
//! api.github.com with the token an [`OAuthSession`] got through Gewu's
//! OAuth app and keeps in the system keychain. [`GhCli`] uses the GitHub
//! CLI's own sign-in instead. [`Recorder`] answers like GitHub without
//! contacting it, which is what tests and the UI sandbox use.

mod account;
mod api;
mod contributors;
mod error;
mod oauth;
mod publish;
mod session;
mod slug;
mod transport;

pub use account::Account;
pub use api::TokenApi;
pub use contributors::Contributor;
pub use error::GitHubError;
pub use oauth::{
    Cached, GEWU_CLIENT_ID, GitHubLogin, Keychain, LoginEndpoint, MemoryStore, OAuthSession, SCOPE,
    SignInPoll, SignInStart, TokenStore,
};
pub use publish::{Posted, Receipt, publish};
pub use session::{CliState, GitHub, GitHubSettings, Preferences, SignInMethod};
pub use slug::Slug;
pub use transport::{GhCli, Recorder, Transport};
