use std::thread;

use serde_json::json;

use super::TokenApi;
use crate::{GitHubError, Transport};

/// Answers one request with `status` and `body`, and returns what it saw.
fn serve_once(
    status: u16,
    body: &'static str,
) -> (String, thread::JoinHandle<(String, String, String)>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("server");
    let base = format!("http://{}", server.server_addr().to_ip().expect("ip"));
    let handle = thread::spawn(move || {
        let mut request = server.recv().expect("request");
        let auth = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Authorization"))
            .map(|header| header.value.to_string())
            .unwrap_or_default();
        let mut sent = String::new();
        request.as_reader().read_to_string(&mut sent).expect("body");
        let line = format!("{} {}", request.method(), request.url());
        request
            .respond(tiny_http::Response::from_string(body).with_status_code(status))
            .expect("respond");
        (line, auth, sent)
    });
    (base, handle)
}

#[test]
fn posts_json_with_the_token() {
    let (base, seen) = serve_once(201, r#"{"number": 5}"#);
    let api = TokenApi::with_base(base, "gho_secret");
    let reply = api
        .call("POST", "/repos/a/b/issues", Some(&json!({"title": "T"})))
        .expect("reply");
    assert_eq!(reply["number"], 5);
    let (line, auth, sent) = seen.join().expect("server");
    assert_eq!(line, "POST /repos/a/b/issues");
    assert_eq!(auth, "Bearer gho_secret");
    assert_eq!(sent, r#"{"title":"T"}"#);
}

#[test]
fn a_revoked_token_reads_as_signed_out() {
    let (base, seen) = serve_once(401, r#"{"message": "Bad credentials"}"#);
    let error = TokenApi::with_base(base, "old").call("GET", "/user", None);
    assert!(matches!(error, Err(GitHubError::SignedOut)));
    seen.join().expect("server");
}

#[test]
fn a_refusal_keeps_github_s_message() {
    let (base, seen) = serve_once(422, r#"{"message": "Validation Failed"}"#);
    let error = TokenApi::with_base(base, "t")
        .call("POST", "/repos/a/b/issues", Some(&json!({})))
        .expect_err("refused");
    assert!(
        matches!(error, GitHubError::Refused { ref message, .. } if message == "Validation Failed")
    );
    seen.join().expect("server");
}
