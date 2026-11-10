use gewu_review::{Feedback, FeedbackKind, FeedbackTarget, LineAnchor, Note, Plan, Venue};

use super::publish;
use crate::Recorder;

fn feedback(id: &str, target: FeedbackTarget, body: &str, line: Option<u32>) -> Feedback {
    Feedback {
        id: id.into(),
        chunk_id: "chunk".into(),
        kind: FeedbackKind::Issue,
        target,
        body: body.into(),
        anchor: line.map(|line| LineAnchor {
            change_id: "change".into(),
            path: "src/lib.rs".into(),
            side: "new".into(),
            start_line: line,
            end_line: line,
            patch_sha256: "hash".into(),
        }),
        resolved: false,
    }
}

fn plan(venue: &Venue, notes: &[Feedback]) -> Plan {
    let notes: Vec<_> = notes
        .iter()
        .map(|feedback| Note {
            feedback,
            chunk_title: "Step",
        })
        .collect();
    Plan::draft(venue, "Session keys", &notes)
}

#[test]
fn a_pull_request_review_is_one_post_carrying_every_code_note() {
    let github = Recorder::signed_in("octocat");
    let venue = Venue::PullRequest {
        repository: "acme/demo".into(),
        number: 12,
    };
    let notes = [
        feedback("line", FeedbackTarget::Code, "Check the join.", Some(4)),
        feedback("whole", FeedbackTarget::Code, "Rename it.", None),
    ];

    let receipt = publish(&github, &plan(&venue, &notes));

    assert_eq!(receipt.failure, None);
    let calls = github.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, "/repos/acme/demo/pulls/12/reviews");
    assert_eq!(receipt.posted[0].feedback_ids, ["line", "whole"]);
    assert!(
        receipt.posted[0]
            .url
            .contains("/acme/demo/pull/12#pullrequestreview-")
    );
}

#[test]
fn the_tracking_issue_is_sent_last_and_links_each_issue() {
    let github = Recorder::signed_in("octocat");
    let venue = Venue::Commits {
        repository: "acme/demo".into(),
        base: "main".into(),
        head: "topic".into(),
    };
    let notes = [
        feedback("a", FeedbackTarget::Code, "The root is wrong.", None),
        feedback("b", FeedbackTarget::Code, "Add a test.", None),
        feedback(
            "c",
            FeedbackTarget::Explanation,
            "The example never runs.",
            None,
        ),
    ];

    let receipt = publish(&github, &plan(&venue, &notes));

    assert_eq!(receipt.failure, None);
    let paths: Vec<_> = github.calls().into_iter().map(|call| call.1).collect();
    assert_eq!(
        paths,
        [
            "/repos/acme/demo/issues",
            "/repos/acme/demo/issues",
            "/repos/acme/demo/issues",
            "/repos/Roger-luo/gewu/issues",
        ]
    );
    let tracking = github.calls()[2].2.clone().expect("body");
    let body = tracking["body"].as_str().expect("text");
    assert!(body.contains("- [ ] #1 The root is wrong"), "{body}");
    assert!(body.contains("- [ ] #2 Add a test"), "{body}");
    assert_eq!(receipt.posted.len(), 4);
    assert!(receipt.posted[2].feedback_ids.is_empty());
    assert_eq!(receipt.posted[3].feedback_ids, ["c"]);
}

#[test]
fn a_refusal_stops_the_run_and_the_receipt_says_what_exists() {
    let github = Recorder::signed_in("octocat");
    let venue = Venue::PullRequest {
        repository: "acme/demo".into(),
        number: 3,
    };
    github.refuse("/repos/Roger-luo/gewu/issues");
    let notes = [
        feedback("line", FeedbackTarget::Code, "Check the join.", Some(4)),
        feedback("x", FeedbackTarget::Explanation, "Unclear step.", None),
        feedback("y", FeedbackTarget::Explanation, "Missing picture.", None),
    ];

    let receipt = publish(&github, &plan(&venue, &notes));

    assert_eq!(receipt.posted.len(), 1);
    let failure = receipt.failure.expect("failure");
    assert!(failure.contains("Unclear step was not sent"), "{failure}");
    assert!(failure.contains("Validation Failed"), "{failure}");
    assert_eq!(github.calls().len(), 1);
}

#[test]
fn signed_out_sends_nothing_and_says_so() {
    let github = Recorder::default();
    let venue = Venue::WorkingTree {
        repository: "acme/demo".into(),
        base: "main".into(),
    };
    let notes = [feedback("a", FeedbackTarget::Code, "Broken.", None)];

    let receipt = publish(&github, &plan(&venue, &notes));

    assert!(receipt.posted.is_empty());
    assert!(
        receipt
            .failure
            .expect("failure")
            .contains("sign in to GitHub first")
    );
}

#[test]
fn a_repository_without_a_github_remote_is_refused_up_front() {
    let github = Recorder::signed_in("octocat");
    let venue = Venue::WorkingTree {
        repository: "demo".into(),
        base: "main".into(),
    };
    let notes = [feedback("a", FeedbackTarget::Code, "Broken.", None)];

    let receipt = publish(&github, &plan(&venue, &notes));

    assert!(github.calls().is_empty());
    assert!(
        receipt
            .failure
            .expect("failure")
            .contains("demo has no GitHub remote")
    );
}
