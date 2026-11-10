use super::{GEWU_REPOSITORY, Note, Plan, Venue};
use crate::{Feedback, FeedbackKind, FeedbackTarget, LineAnchor};

fn note<'a>(feedback: &'a Feedback, chunk_title: &'a str) -> Note<'a> {
    Note {
        feedback,
        chunk_title,
    }
}

fn feedback(
    id: &str,
    target: FeedbackTarget,
    body: &str,
    anchor: Option<LineAnchor>,
    resolved: bool,
) -> Feedback {
    Feedback {
        id: id.into(),
        chunk_id: "chunk".into(),
        kind: FeedbackKind::Issue,
        target,
        body: body.into(),
        anchor,
        resolved,
    }
}

fn anchor(path: &str, side: &str, start: u32, end: u32) -> LineAnchor {
    LineAnchor {
        change_id: "change".into(),
        path: path.into(),
        side: side.into(),
        start_line: start,
        end_line: end,
        patch_sha256: "hash".into(),
    }
}

fn pull(number: u64) -> Venue {
    Venue::PullRequest {
        repository: "acme/demo".into(),
        number,
    }
}

#[test]
fn a_pull_request_keeps_line_notes_inline_and_summarizes_the_rest() {
    let anchored = feedback(
        "line",
        FeedbackTarget::Code,
        "Confirm the path join.",
        Some(anchor("src/lib.rs", "new", 40, 42)),
        false,
    );
    let loose = feedback(
        "whole",
        FeedbackTarget::Code,
        "The name is unclear.",
        None,
        false,
    );
    let plan = Plan::draft(
        &pull(12),
        "Session keys",
        &[note(&anchored, "Event path"), note(&loose, "Naming")],
    );

    let requests = plan.clone().github_requests();
    let review = plan.pull_request.expect("review");
    assert_eq!(review.number, 12);
    assert_eq!(review.comments.len(), 1);
    assert_eq!(review.comments[0].path, "src/lib.rs");
    assert_eq!(review.comments[0].body, "Confirm the path join.");
    let summary = review.summary.expect("summary");
    assert!(summary.contains("one review comment"));
    assert!(summary.contains("**Naming.** The name is unclear."));
    assert!(plan.code_issues.is_none());
    assert!(plan.gewu_issues.is_none());
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/repos/acme/demo/pulls/12/reviews");
    assert_eq!(requests[0].body["event"], "COMMENT");
    assert_eq!(requests[0].body["comments"][0]["side"], "RIGHT");
    assert_eq!(requests[0].body["comments"][0]["line"], 42);
    assert_eq!(requests[0].body["comments"][0]["start_line"], 40);
}

#[test]
fn removed_lines_are_the_left_side() {
    let note_on = feedback(
        "old",
        FeedbackTarget::Code,
        "This deletion drops the check.",
        Some(anchor("src/lib.rs", "old", 7, 7)),
        false,
    );
    let plan = Plan::draft(&pull(3), "Rename", &[note(&note_on, "Rename")]);
    let requests = plan.github_requests();
    assert_eq!(requests[0].body["comments"][0]["side"], "LEFT");
    assert!(requests[0].body["comments"][0].get("start_line").is_none());
    assert_eq!(
        requests[0].body["body"],
        "1 line comment from a review in Gewu."
    );
    assert!(requests[0].body.get("commit_id").is_none());
}

#[test]
fn walkthrough_notes_become_gewu_issues_with_a_tracking_issue() {
    let first = feedback(
        "a",
        FeedbackTarget::Explanation,
        "The example never runs.",
        None,
        false,
    );
    let second = feedback(
        "b",
        FeedbackTarget::Explanation,
        "Define the term first.",
        None,
        false,
    );
    let done = feedback(
        "c",
        FeedbackTarget::Explanation,
        "Already handled.",
        None,
        true,
    );
    let plan = Plan::draft(
        &pull(4),
        "Session keys",
        &[
            note(&first, "Intro"),
            note(&second, "Terms"),
            note(&done, "Done"),
        ],
    );

    let requests = plan.clone().github_requests();
    assert!(plan.pull_request.is_none());
    let issues = plan.gewu_issues.expect("gewu");
    assert_eq!(issues.repository, GEWU_REPOSITORY);
    assert_eq!(issues.issues.len(), 2);
    assert!(
        issues.issues[0]
            .body
            .contains("This is about the walkthrough Gewu wrote.")
    );
    let tracking = issues.tracking.expect("tracking");
    assert!(tracking.feedback_id.is_none());
    assert!(tracking.body.contains("2 separate notes"));
    assert!(tracking.body.contains(&issues.issues[0].title));

    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .iter()
            .all(|request| request.path == "/repos/Roger-luo/gewu/issues")
    );
    assert_eq!(requests[2].body["title"], tracking.title);
}

#[test]
fn a_tracking_issue_links_the_issues_once_they_exist() {
    let first = feedback("a", FeedbackTarget::Code, "The root is wrong.", None, false);
    let second = feedback("b", FeedbackTarget::Code, "Add a test.", None, false);
    let venue = Venue::WorkingTree {
        repository: "acme/demo".into(),
        base: "main".into(),
    };
    let plan = Plan::draft(
        &venue,
        "Paths",
        &[note(&first, "One"), note(&second, "Two")],
    );
    let bundle = plan.code_issues.expect("issues");

    let linked = bundle.linked_tracking(&[17, 18]).expect("tracking");
    assert!(linked.body.contains("- [ ] #17 The root is wrong"));
    assert!(linked.body.contains("- [ ] #18 Add a test"));
    assert!(linked.body.starts_with("2 separate notes"));
    assert_eq!(linked.title, bundle.tracking.expect("tracking").title);
}

#[test]
fn the_review_summary_records_which_notes_it_covers() {
    let loose = feedback("whole", FeedbackTarget::Code, "Unclear name.", None, false);
    let plan = Plan::draft(&pull(9), "Naming", &[note(&loose, "Naming")]);
    let review = plan.pull_request.expect("review");
    assert_eq!(review.summarized, vec!["whole".to_string()]);
    let mut pinned = review.clone();
    pinned.commit_id = Some("abc123".into());
    let request = pinned.request();
    assert_eq!(request.path, "/repos/acme/demo/pulls/9/reviews");
    assert_eq!(request.body["commit_id"], "abc123");
}

#[test]
fn commits_outside_a_pull_request_become_separate_issues() {
    let one = feedback(
        "one",
        FeedbackTarget::Code,
        "The helper returns the wrong root.",
        Some(anchor("src/lib.rs", "old", 8, 9)),
        false,
    );
    let two = feedback(
        "two",
        FeedbackTarget::Code,
        "Add a test for the empty path.",
        None,
        false,
    );
    let venue = Venue::Commits {
        repository: "acme/demo".into(),
        base: "main".into(),
        head: "feature".into(),
    };
    let plan = Plan::draft(
        &venue,
        "Paths",
        &[note(&one, "Helper"), note(&two, "Tests")],
    );

    let requests = plan.clone().github_requests();
    assert!(plan.pull_request.is_none());
    let issues = plan.code_issues.expect("issues");
    assert_eq!(issues.issues.len(), 2);
    assert!(issues.issues[0].body.contains("previous version"));
    assert!(
        issues.issues[0]
            .body
            .contains("commits main..feature on acme/demo")
    );
    assert!(issues.tracking.is_some());
    assert!(
        requests
            .iter()
            .all(|request| request.path == "/repos/acme/demo/issues")
    );
}

#[test]
fn one_directory_note_has_no_tracking_issue() {
    let only = feedback(
        "only",
        FeedbackTarget::Code,
        "The directory listing skips hidden files.",
        None,
        false,
    );
    let venue = Venue::Directory {
        repository: "acme/demo".into(),
        path: "src".into(),
        reference: "main".into(),
    };
    let plan = Plan::draft(&venue, "Listing", &[note(&only, "Listing")]);
    let issues = plan.code_issues.expect("issue");
    assert!(issues.tracking.is_none());
    assert!(
        issues.issues[0]
            .body
            .contains("directory src at main on acme/demo")
    );
}

#[test]
fn an_empty_review_has_nothing_to_publish() {
    let venue = Venue::WorkingTree {
        repository: "acme/demo".into(),
        base: "main".into(),
    };
    let plan = Plan::draft(&venue, "Work", &[]);
    assert!(plan.is_empty());
    assert!(plan.github_requests().is_empty());
}
