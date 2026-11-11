//! The ACP client against the stand-in agent in `fixtures/agents`.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

use gewu_agent::{AcpRun, AgentEvent, Cancel, EventKind, EventSink};

fn fake_agent() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/agents/fake_acp_agent.py")
        .canonicalize()
        .expect("fake agent")
        .to_string_lossy()
        .into_owned()
}

fn project() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temporary project");
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(root.path())
            .output()
            .expect("git");
        assert!(status.status.success(), "{status:?}");
    };
    git(&["init", "-q"]);
    std::fs::write(root.path().join("a.rs"), "const A: u32 = 1;\n").expect("write");
    root
}

fn recorder() -> (EventSink, Arc<Mutex<Vec<AgentEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink: EventSink = {
        let events = Arc::clone(&events);
        Arc::new(move |event| events.lock().expect("events").push(event))
    };
    (sink, events)
}

fn run(
    root: &Path,
    mode: &str,
) -> (
    Result<gewu_agent::Outcome, gewu_agent::AgentError>,
    Vec<AgentEvent>,
) {
    run_with(root, mode, "")
}

fn run_with(
    root: &Path,
    mode: &str,
    login: &str,
) -> (
    Result<gewu_agent::Outcome, gewu_agent::AgentError>,
    Vec<AgentEvent>,
) {
    let (sink, events) = recorder();
    let command = vec![
        "env".to_owned(),
        format!("FAKE_AGENT_MODE={mode}"),
        "FAKE_AGENT_DELAY=0".to_owned(),
        format!("FAKE_AGENT_LOGIN={login}"),
        "python3".to_owned(),
        fake_agent(),
    ];
    let result = AcpRun::new(root, command, &PathBuf::from("/usr/local/bin/gewu"))
        .run(sink, &Cancel::default());
    let events = events.lock().expect("events").clone();
    (result, events)
}

#[test]
fn allowed_requests_are_served_and_others_refused() {
    let root = project();
    let (result, events) = run(root.path(), "probe");
    let outcome = result.expect("session ends");
    assert_eq!(outcome.stop_reason, "end_turn");
    assert_eq!(
        std::fs::read_to_string(root.path().join(".gewu/harness/probe.txt")).expect("probe"),
        "allowed"
    );
    assert!(!root.path().join("outside.txt").exists());
    assert_eq!(outcome.denials.len(), 2, "{:?}", outcome.denials);
    assert!(outcome.denials[0].contains("outside.txt"));
    assert!(outcome.denials[1].contains("rm -rf"));
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::Output && event.text == "Probing the policy.")
    );
}

#[test]
fn a_crashing_agent_is_an_error_with_its_stderr() {
    let root = project();
    let (result, _) = run(root.path(), "fail");
    let error = result.expect_err("the agent exits").to_string();
    assert!(error.contains("simulated failure"), "{error}");
}

#[test]
fn a_missing_program_is_named() {
    let root = project();
    let (sink, _) = recorder();
    let error = AcpRun::new(
        root.path(),
        vec!["gewu-no-such-agent".to_owned()],
        Path::new("/usr/local/bin/gewu"),
    )
    .run(sink, &Cancel::default())
    .expect_err("missing program");
    assert!(error.to_string().contains("gewu-no-such-agent"));
}

#[test]
fn an_agent_that_needs_authenticate_is_signed_in_and_retried() {
    let root = project();
    let (result, events) = run_with(root.path(), "probe", "required");
    assert_eq!(result.expect("session ends").stop_reason, "end_turn");
    assert!(
        events
            .iter()
            .any(|event| event.text == "signing in to the agent with Fake Login"),
        "{events:?}"
    );
}

#[test]
fn a_signed_out_agent_says_to_sign_in() {
    let root = project();
    let (result, _) = run_with(root.path(), "probe", "refused");
    let error = result.expect_err("signed out").to_string();
    assert!(error.contains("sign in to it first"), "{error}");
    assert!(error.contains("run 'fake login' first"), "{error}");
}

fn run_hanging(
    root: &Path,
    hang: &str,
) -> (
    Result<gewu_agent::Outcome, gewu_agent::AgentError>,
    Vec<AgentEvent>,
) {
    let (sink, events) = recorder();
    let command = vec![
        "env".to_owned(),
        "FAKE_AGENT_MODE=probe".to_owned(),
        "FAKE_AGENT_DELAY=0".to_owned(),
        format!("FAKE_AGENT_HANG={hang}"),
        "python3".to_owned(),
        fake_agent(),
    ];
    let result = AcpRun::new(root, command, &PathBuf::from("/usr/local/bin/gewu"))
        .with_handshake_limit(std::time::Duration::from_secs(2))
        .run(sink, &Cancel::default());
    let events = events.lock().expect("events").clone();
    (result, events)
}

/// Found in a live run of ppvm #276: Cursor's CLI hung before opening a
/// session four times, 25 minutes each, and the run failed.
#[test]
fn an_agent_that_hangs_before_its_session_opens_is_started_again() {
    let root = project();
    let marker = root.path().join("hung-once");
    let (result, events) = run_hanging(root.path(), &marker.to_string_lossy());
    assert_eq!(result.expect("second start works").stop_reason, "end_turn");
    assert!(
        events
            .iter()
            .any(|event| event.text.starts_with("session open after ")),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| event.text.contains("starting it again (attempt 2 of 3)")),
        "{events:?}"
    );
    let pid = std::fs::read_to_string(&marker).expect("the hung agent's pid");
    let alive = Command::new("kill")
        .args(["-0", pid.trim()])
        .output()
        .expect("kill")
        .status
        .success();
    assert!(!alive, "the hung agent {pid} is left running");
}

#[test]
fn an_agent_that_never_opens_a_session_fails_after_three_starts() {
    let root = project();
    let (result, events) = run_hanging(root.path(), "always");
    let error = result.expect_err("never opens");
    assert!(
        matches!(error, gewu_agent::AgentError::Unresponsive { waited: 2 }),
        "{error}"
    );
    let restarts = events
        .iter()
        .filter(|event| event.text.contains("starting it again"))
        .count();
    assert_eq!(restarts, 2, "{events:?}");
}
