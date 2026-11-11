//! The TypeSafe client against a loopback server, and scoring with a mock.
//! No test reaches the network.

use std::{collections::BTreeMap, sync::mpsc, thread};

use gewu_decide::{DecideError, Decider, MockDecider, TypesafeDecider, score};
use gewu_review::{
    ChunkBlock, ChunkKind, Overview, ReviewChunk, ReviewInput, ReviewSpec, SourceIdentity,
    load::{Criterion, Rubric},
};
use serde_json::{Value, json};

fn questions() -> &'static [Criterion] {
    &Rubric::builtin().questions
}

const KEY: &str = "secret-key-1";

/// Serves `replies` in order, sending each request's auth header and body
/// back through the channel.
fn serve(replies: Vec<(u16, String)>) -> (String, mpsc::Receiver<(String, Value)>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/v1/systemone", server.server_addr());
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for (status, body) in replies {
            let Ok(mut request) = server.recv() else {
                return;
            };
            let auth = request
                .headers()
                .iter()
                .find(|header| header.field.equiv("Authorization"))
                .map(|header| header.value.to_string())
                .unwrap_or_default();
            let mut text = String::new();
            request.as_reader().read_to_string(&mut text).expect("body");
            let _ = sender.send((auth, serde_json::from_str(&text).unwrap_or(Value::Null)));
            let _ =
                request.respond(tiny_http::Response::from_string(body).with_status_code(status));
        }
    });
    (url, receiver)
}

fn all_answered(p: f64) -> String {
    let answers: serde_json::Map<_, _> = questions()
        .iter()
        .map(|c| (c.name.clone(), json!({"noul": p})))
        .collect();
    json!({"answers": answers}).to_string()
}

#[test]
fn asks_every_criterion_in_one_call_with_the_key_as_bearer() {
    let (url, requests) = serve(vec![(200, all_answered(0.25))]);
    let answers = TypesafeDecider::at(url, KEY)
        .answer("# A chunk", questions())
        .expect("answers");
    assert_eq!(answers.len(), questions().len());
    assert!(answers.values().all(|p| (*p - 0.25).abs() < 1e-12));
    let (auth, body) = requests.recv().expect("one request");
    assert_eq!(auth, format!("Bearer {KEY}"));
    assert_eq!(body["model"], "jev-latest");
    assert_eq!(body["state"], "# A chunk");
    let question = &body["questions"]["distant_recall"];
    assert_eq!(question["type"], "noul");
    assert!(question["criteria"]["true"].is_string());
}

#[test]
fn retries_a_busy_server() {
    let (url, _requests) = serve(vec![
        (503, "busy".into()),
        (429, "slow down".into()),
        (200, all_answered(0.5)),
    ]);
    let answers = TypesafeDecider::at(url, KEY).answer("s", questions());
    assert!(answers.is_ok(), "{answers:?}");
}

#[test]
fn a_refusal_is_not_retried_and_never_shows_the_key() {
    let (url, requests) = serve(vec![
        (401, format!("bad key {KEY}")),
        (200, all_answered(0.5)),
    ]);
    let error = TypesafeDecider::at(url, KEY)
        .answer("s", questions())
        .expect_err("refused");
    assert!(
        matches!(error, DecideError::Http { status: 401, .. }),
        "{error}"
    );
    assert!(!error.to_string().contains(KEY), "{error}");
    assert_eq!(requests.try_iter().count(), 1);
}

#[test]
fn a_probability_outside_the_unit_interval_is_malformed() {
    let (url, _requests) = serve(vec![(200, all_answered(1.5))]);
    let error = TypesafeDecider::at(url, KEY).answer("s", questions());
    assert!(matches!(error, Err(DecideError::Malformed(_))), "{error:?}");
}

fn spec(chunks: usize) -> (ReviewSpec, ReviewInput) {
    let source = SourceIdentity::new("base", "head", b"diff");
    let chunk = |index: usize| ReviewChunk {
        id: format!("chunk-{index}"),
        title: "Title".into(),
        summary: "Summary.".into(),
        kind: ChunkKind::Other,
        content: vec![ChunkBlock::Narrative {
            markdown: "Prose.".into(),
        }],
        context: None,
        confidence: None,
        oversized_reason: None,
        section: None,
    };
    let overview = Overview {
        intent: "Intent.".into(),
        changes: Vec::new(),
        illustration: None,
        concepts: Vec::new(),
    };
    let input = ReviewInput::sized(source.clone(), Vec::new(), 40);
    (
        ReviewSpec::new(source, overview, (0..chunks).map(chunk).collect()),
        input,
    )
}

#[test]
fn scoring_combines_answers_through_the_equation() {
    let (spec, input) = spec(4);
    let quiet = score(&spec, &input, Rubric::builtin(), None);
    assert!(!quiet.load.high());
    assert!(
        quiet
            .load
            .chunks
            .iter()
            .all(|c| c.unanswered == questions().len())
    );

    let heavy = MockDecider {
        p: 0.0,
        script: BTreeMap::from([
            ("distant_recall".into(), 1.0),
            ("several_points".into(), 1.0),
        ]),
        fail: false,
    };
    let scored = score(&spec, &input, Rubric::builtin(), Some(&heavy));
    assert!(scored.failures.is_empty());
    assert!(scored.load.chunks.iter().all(|c| c.overloaded == 1.0));
    assert!(scored.load.high());
}

#[test]
fn a_failing_decider_falls_back_to_reading_time_and_says_so() {
    let (spec, input) = spec(2);
    let failing = MockDecider {
        fail: true,
        ..MockDecider::default()
    };
    let scored = score(&spec, &input, Rubric::builtin(), Some(&failing));
    assert_eq!(scored.failures.len(), 2);
    assert!(!scored.load.high());
}
