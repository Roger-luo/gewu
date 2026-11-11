use std::sync::Mutex;

use serde_json::{Value, json};

use super::{LoginEndpoint, MemoryStore, OAuthSession, SCOPE, SignInPoll, SignInStart, TokenStore};
use crate::{GitHubError, Recorder};

/// One posted form: its path and fields.
type Form = (String, Vec<(String, String)>);

/// github.com's login endpoints, answering from a script and logging forms.
#[derive(Default)]
struct Script {
    replies: Mutex<Vec<Value>>,
    forms: Mutex<Vec<Form>>,
}

impl Script {
    fn new(replies: Vec<Value>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().rev().collect()),
            forms: Mutex::default(),
        }
    }
}

impl LoginEndpoint for &'static Script {
    fn post(&self, path: &str, form: &[(&str, &str)]) -> Result<Value, GitHubError> {
        self.forms.lock().expect("forms").push((
            path.to_string(),
            form.iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        ));
        Ok(self
            .replies
            .lock()
            .expect("replies")
            .pop()
            .expect("a scripted reply"))
    }
}

fn code() -> Value {
    json!({
        "device_code": "dev-123",
        "user_code": "ABCD-1234",
        "verification_uri": "https://github.com/login/device",
        "expires_in": 900,
        "interval": 5,
    })
}

fn session(script: &'static Script, store: &'static MemoryStore) -> OAuthSession {
    struct Shared(&'static MemoryStore);
    impl TokenStore for Shared {
        fn get(&self) -> Result<Option<String>, GitHubError> {
            self.0.get()
        }
        fn set(&self, token: &str) -> Result<(), GitHubError> {
            self.0.set(token)
        }
        fn delete(&self) -> Result<(), GitHubError> {
            self.0.delete()
        }
    }
    OAuthSession::with_parts(
        "client-id",
        Box::new(script),
        Box::new(Shared(store)),
        Box::new(|_token| Box::new(Recorder::signed_in("octocat"))),
    )
}

fn leak<T>(value: T) -> &'static T {
    Box::leak(Box::new(value))
}

#[test]
fn approving_the_code_saves_the_token_and_reads_the_account() {
    let script = leak(Script::new(vec![
        code(),
        json!({"error": "authorization_pending"}),
        json!({"error": "slow_down", "interval": 10}),
        json!({"access_token": "gho_token", "token_type": "bearer", "scope": "repo"}),
    ]));
    let store = leak(MemoryStore::default());
    let session = session(script, store);
    assert_eq!(session.account().expect("account"), None);

    let start = session.start().expect("start");
    assert!(
        matches!(start, SignInStart::Device { ref user_code, interval: 5, .. } if user_code == "ABCD-1234")
    );
    assert_eq!(session.poll().expect("poll"), SignInPoll::Pending);
    assert_eq!(
        session.poll().expect("poll"),
        SignInPoll::SlowDown { interval: 10 }
    );
    let SignInPoll::SignedIn { account } = session.poll().expect("poll") else {
        panic!("not signed in");
    };
    assert_eq!(account.login, "octocat");
    assert_eq!(store.get().expect("token").as_deref(), Some("gho_token"));

    let forms = script.forms.lock().expect("forms");
    assert_eq!(forms[0].0, "/login/device/code");
    assert!(forms[0].1.contains(&("scope".into(), SCOPE.into())));
    assert!(
        forms[0]
            .1
            .contains(&("client_id".into(), "client-id".into()))
    );
    assert!(
        forms[1]
            .1
            .contains(&("device_code".into(), "dev-123".into()))
    );
}

#[test]
fn a_denied_code_saves_nothing_and_needs_a_new_start() {
    let script = leak(Script::new(vec![code(), json!({"error": "access_denied"})]));
    let store = leak(MemoryStore::default());
    let session = session(script, store);
    session.start().expect("start");
    assert_eq!(session.poll().expect("poll"), SignInPoll::Denied);
    assert_eq!(
        session.poll().expect("poll"),
        SignInPoll::Expired,
        "the code is spent"
    );
    assert_eq!(store.get().expect("token"), None);
}

#[test]
fn a_disabled_device_flow_says_why() {
    let script = leak(Script::new(vec![json!({
        "error": "device_flow_disabled",
        "error_description": "Device flow must be explicitly enabled for this App",
    })]));
    let session = session(script, leak(MemoryStore::default()));
    let error = session.start().expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("Device flow must be explicitly enabled"),
        "{error}"
    );
}

#[test]
fn signing_out_forgets_the_token() {
    let store = leak(MemoryStore::default());
    store.set("gho_token").expect("set");
    let session = session(leak(Script::default()), store);
    assert!(session.account().expect("account").is_some());
    session.sign_out().expect("sign out");
    assert_eq!(session.account().expect("account"), None);
}
