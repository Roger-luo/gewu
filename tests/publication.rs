//! Publishing a task's notes, end to end against a real repository. GitHub
//! is the [`Recorder`], so no request leaves the machine.

mod support;

use gewu::workbench::{Recorder, Source, TaskStatus};
use gewu_app::{Action, AnchorRef, BlockView};
use gewu_review::{FeedbackKind, FeedbackTarget, Side};
use support::{Fixture, none, range};

/// A finished raw-diff review of the fixture's last commit, with one note on
/// a line and one on the walkthrough.
fn reviewed(fixture: &Fixture) -> String {
    let id = fixture.start(range(), none());
    assert_eq!(fixture.settled(&id).status, TaskStatus::Complete);
    let snapshot = fixture.workbench.task_snapshot(&id).expect("review");
    let chunk = &snapshot.chunks[0];
    let quote = chunk
        .blocks
        .iter()
        .find_map(|block| match block {
            BlockView::Changes { quotes, .. } => quotes.first(),
            BlockView::Narrative { .. } | BlockView::Explainer(_) => None,
        })
        .expect("quote");
    let change_id = quote.change_id.clone();
    let line = quote
        .lines
        .iter()
        .find_map(|line| line.new_line)
        .expect("line");
    for (target, body, anchor) in [
        (
            FeedbackTarget::Code,
            "Bumping A needs a migration note.",
            Some(AnchorRef {
                change_id,
                start_line: line,
                end_line: line,
                side: Side::New,
            }),
        ),
        (
            FeedbackTarget::Explanation,
            "This step never says why A changed.",
            None,
        ),
    ] {
        fixture
            .workbench
            .task_dispatch(
                &id,
                Action::AddFeedback {
                    chunk_id: chunk.id.clone(),
                    kind: FeedbackKind::Issue,
                    target,
                    body: body.into(),
                    anchor,
                },
            )
            .expect("note");
    }
    id
}

#[test]
fn a_repository_without_a_github_remote_previews_but_does_not_send() {
    let fixture = Fixture::new();
    let id = reviewed(&fixture);

    let publication = fixture.workbench.publication(&id).expect("publication");
    let blocked = publication.blocked.expect("blocked");
    assert!(
        blocked.contains("no GitHub remote named `origin`"),
        "{blocked}"
    );
    let code = publication.plan.code_issues.expect("code issues");
    assert_eq!(code.repository, "project");
    assert_eq!(publication.requests.len(), 2);

    let github = Recorder::signed_in("octocat");
    let receipt = fixture.workbench.publish(&id, &github).expect("receipt");
    assert!(receipt.posted.is_empty());
    assert!(github.calls().is_empty());
}

#[test]
fn notes_are_sent_once_and_left_out_of_the_next_draft() {
    let fixture = Fixture::new();
    fixture.git(&["remote", "add", "origin", "git@github.com:acme/project.git"]);
    let id = reviewed(&fixture);

    let publication = fixture.workbench.publication(&id).expect("publication");
    assert_eq!(publication.blocked, None);
    assert_eq!(
        publication.requests[0].path, "/repos/acme/project/issues",
        "a commit range outside a pull request is an issue on the repository"
    );
    assert_eq!(publication.requests[1].path, "/repos/Roger-luo/gewu/issues");

    let github = Recorder::signed_in("octocat");
    let receipt = fixture.workbench.publish(&id, &github).expect("receipt");
    assert_eq!(receipt.failure, None);
    assert_eq!(receipt.posted.len(), 2);
    assert_eq!(github.calls().len(), 2);

    let after = fixture.workbench.publication(&id).expect("publication");
    assert!(after.plan.is_empty(), "{:?}", after.plan);
    assert_eq!(after.published.len(), 2);
    let again = fixture.workbench.publish(&id, &github).expect("receipt");
    assert!(again.posted.is_empty());
    assert_eq!(github.calls().len(), 2, "nothing is sent twice");
}

#[test]
fn a_pull_request_task_posts_one_review_to_its_remote() {
    let mut fixture = Fixture::new();
    fixture.git(&[
        "remote",
        "add",
        "upstream",
        "https://github.com/acme/project.git",
    ]);
    let id = reviewed(&fixture);
    // The raw review came from a range; reading it as a pull request is
    // what the venue decides, so swap the source the way a PR task has it.
    fixture.set_source(
        &id,
        Source::PullRequest {
            number: 12,
            remote: "upstream".into(),
        },
    );

    let publication = fixture.workbench.publication(&id).expect("publication");
    let review = publication.plan.pull_request.expect("review");
    assert_eq!(review.repository, "acme/project");
    assert_eq!(review.comments.len(), 1);
    assert_eq!(
        publication.requests[0].path,
        "/repos/acme/project/pulls/12/reviews"
    );
}

#[test]
fn contributors_are_read_from_the_remote_once_per_run() {
    let fixture = Fixture::new();
    let id = reviewed(&fixture);
    let github = Recorder::signed_in("octocat");
    assert!(
        fixture
            .workbench
            .contributors(&id, &github)
            .expect("contributors")
            .is_empty(),
        "a repository off GitHub has nobody to mention"
    );
    assert!(github.calls().is_empty());

    fixture.git(&["remote", "add", "origin", "https://github.com/acme/project"]);
    assert!(
        fixture
            .workbench
            .contributors(&id, &Recorder::default())
            .expect("signed out")
            .is_empty()
    );
    for _ in 0..2 {
        let logins: Vec<String> = fixture
            .workbench
            .contributors(&id, &github)
            .expect("contributors")
            .into_iter()
            .map(|contributor| contributor.login)
            .collect();
        assert_eq!(logins, ["octocat", "grace", "linus"]);
    }
    assert_eq!(github.calls().len(), 1, "the second read is cached");
    assert_eq!(
        github.calls()[0].1,
        "/repos/acme/project/contributors?per_page=100"
    );
}

#[test]
fn a_rehearsal_sends_nothing_and_marks_nothing_as_sent() {
    let fixture = Fixture::new();
    fixture.git(&["remote", "add", "origin", "git@github.com:acme/project.git"]);
    let id = reviewed(&fixture);

    let receipt = fixture.workbench.rehearse(&id, "octocat").expect("receipt");
    assert!(receipt.rehearsed);
    assert_eq!(receipt.failure, None);
    assert_eq!(receipt.posted.len(), 2);
    let after = fixture.workbench.publication(&id).expect("publication");
    assert!(after.published.is_empty(), "a rehearsal records no posts");
    assert_eq!(
        after.requests.len(),
        2,
        "the notes are still waiting to be sent"
    );
}

#[test]
fn a_remote_off_github_is_named_in_the_blocked_message() {
    let fixture = Fixture::new();
    fixture.git(&["remote", "add", "origin", "/srv/git/project.git"]);
    let id = reviewed(&fixture);
    let blocked = fixture
        .workbench
        .publication(&id)
        .expect("publication")
        .blocked
        .expect("blocked");
    assert!(
        blocked.contains("`origin` points at /srv/git/project.git, not github.com"),
        "{blocked}"
    );
}
