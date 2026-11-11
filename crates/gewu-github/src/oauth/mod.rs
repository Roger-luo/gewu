//! Signing in with Gewu's GitHub OAuth app through the device flow: Gewu
//! shows a code, the reviewer approves it on github.com, and the token is
//! kept in the system keychain.

mod device;
mod store;

#[cfg(test)]
mod tests;

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::{Account, GitHubError, TokenApi, Transport};
use device::DeviceCode;

pub use device::{GitHubLogin, LoginEndpoint};
pub use store::{Keychain, MemoryStore, TokenStore};

/// Gewu's OAuth app. A client ID is public; the device flow needs no secret.
pub const GEWU_CLIENT_ID: &str = "Ov23liMKKIvqy32L2nuS";

/// Public and private repositories: issues and pull-request reviews.
pub const SCOPE: &str = "repo";

/// How a sign-in begins.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignInStart {
    /// Type `user_code` at `verification_uri`; poll every `interval` seconds.
    Device {
        user_code: String,
        verification_uri: String,
        interval: u64,
        expires_in: u64,
    },
    /// Paste a token instead; the GitHub CLI fallback.
    Token,
}

/// Where a device sign-in stands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SignInPoll {
    Pending,
    /// GitHub asked Gewu to poll less often.
    SlowDown {
        interval: u64,
    },
    SignedIn {
        account: Account,
    },
    Denied,
    Expired,
}

type MakeApi = Box<dyn Fn(&str) -> Box<dyn Transport> + Send + Sync>;

/// One reviewer's OAuth sign-in: the pending device code and the saved token.
pub struct OAuthSession {
    client_id: String,
    login: Box<dyn LoginEndpoint>,
    store: Box<dyn TokenStore>,
    make_api: MakeApi,
    pending: Mutex<Option<DeviceCode>>,
}

impl OAuthSession {
    /// github.com, the system keychain, and api.github.com.
    pub fn open(client_id: impl Into<String>) -> Result<Self, GitHubError> {
        Ok(Self::with_parts(
            client_id,
            Box::new(GitHubLogin::default()),
            Box::new(Keychain::open()?),
            Box::new(|token| Box::new(TokenApi::new(token))),
        ))
    }

    #[must_use]
    pub fn with_parts(
        client_id: impl Into<String>,
        login: Box<dyn LoginEndpoint>,
        store: Box<dyn TokenStore>,
        make_api: MakeApi,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            login,
            store,
            make_api,
            pending: Mutex::new(None),
        }
    }

    /// The API as the signed-in reviewer, or one that answers "signed out".
    pub fn transport(&self) -> Result<Box<dyn Transport>, GitHubError> {
        Ok(match self.store.get()? {
            Some(token) => (self.make_api)(&token),
            None => Box::new(SignedOut),
        })
    }

    /// The signed-in reviewer; `None` when there is no token or GitHub no
    /// longer accepts it.
    pub fn account(&self) -> Result<Option<Account>, GitHubError> {
        Account::current(self.transport()?.as_ref())
    }

    /// Asks GitHub for a device code.
    pub fn start(&self) -> Result<SignInStart, GitHubError> {
        let reply = self.login.post(
            "/login/device/code",
            &[("client_id", &self.client_id), ("scope", SCOPE)],
        )?;
        if let Some(error) = reply["error"].as_str() {
            return Err(GitHubError::SignIn(describe(error, &reply)));
        }
        let code: DeviceCode =
            serde_json::from_value(reply).map_err(|error| GitHubError::Reply(error.to_string()))?;
        let start = SignInStart::Device {
            user_code: code.user_code.clone(),
            verification_uri: code.verification_uri.clone(),
            interval: code.interval,
            expires_in: code.expires_in,
        };
        *self.pending() = Some(code);
        Ok(start)
    }

    /// Checks once whether the reviewer approved the code. Call it no more
    /// often than the interval GitHub gave.
    pub fn poll(&self) -> Result<SignInPoll, GitHubError> {
        let Some(code) = self.pending().clone() else {
            return Ok(SignInPoll::Expired);
        };
        let reply = self.login.post(
            "/login/oauth/access_token",
            &[
                ("client_id", &self.client_id),
                ("device_code", &code.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ],
        )?;
        if let Some(token) = reply["access_token"].as_str() {
            self.store.set(token)?;
            *self.pending() = None;
            return Ok(match self.account()? {
                Some(account) => SignInPoll::SignedIn { account },
                None => SignInPoll::Denied,
            });
        }
        let poll = match reply["error"].as_str() {
            Some("authorization_pending") => SignInPoll::Pending,
            Some("slow_down") => SignInPoll::SlowDown {
                interval: reply["interval"].as_u64().unwrap_or(code.interval + 5),
            },
            Some("access_denied") => SignInPoll::Denied,
            Some("expired_token") => SignInPoll::Expired,
            Some(error) => return Err(GitHubError::SignIn(describe(error, &reply))),
            None => return Err(GitHubError::Reply(reply.to_string())),
        };
        if matches!(poll, SignInPoll::Denied | SignInPoll::Expired) {
            *self.pending() = None;
        }
        Ok(poll)
    }

    /// Forgets the token. GitHub keeps the authorization until the reviewer
    /// revokes it in their settings.
    pub fn sign_out(&self) -> Result<(), GitHubError> {
        *self.pending() = None;
        self.store.delete()
    }

    fn pending(&self) -> std::sync::MutexGuard<'_, Option<DeviceCode>> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn describe(error: &str, reply: &serde_json::Value) -> String {
    reply["error_description"]
        .as_str()
        .map_or_else(|| error.to_string(), str::to_string)
}

/// Every call fails as "not signed in".
struct SignedOut;

impl Transport for SignedOut {
    fn call(
        &self,
        _method: &str,
        _path: &str,
        _body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, GitHubError> {
        Err(GitHubError::SignedOut)
    }
}
