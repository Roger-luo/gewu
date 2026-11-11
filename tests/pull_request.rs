//! A pull-request task saves the request's title and body beside the review
//! input, where the generating agent reads the author's claims. Its own test
//! binary: it puts the fake `gh` from `fixtures/bin` first on `PATH`.

mod support;

use std::process::Command;

use gewu::workbench::{Source, TaskStatus};
use support::{Fixture, none};

#[test]
fn a_pull_request_task_saves_the_authors_description() {
    let fixture = Fixture::new();
    let root = fixture.repository.parent().expect("parent").to_path_buf();
    let origin = root.join("origin.git");
    let git = |dir: &std::path::Path, args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git");
        assert!(out.status.success(), "{out:?}");
    };
    git(
        &root,
        &["init", "-q", "--bare", origin.to_str().expect("path")],
    );
    fixture.git(&["remote", "add", "origin", origin.to_str().expect("path")]);
    // The request asks to merge the last commit into a `main` one behind it.
    fixture.git(&[
        "push",
        "-q",
        "origin",
        "HEAD~1:refs/heads/main",
        "HEAD:refs/pull/7/head",
    ]);

    let pulls = root.join("pulls.json");
    std::fs::write(
        &pulls,
        r#"{"project": [{"number": 7, "title": "Print receipts", "author": {"login": "ada"},
            "headRefName": "feature/receipts", "baseRefName": "main",
            "body": "Prints receipts 2× faster by reusing the buffer."}]}"#,
    )
    .expect("pulls");
    let bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/bin");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    // SAFETY: this binary holds this one test, so nothing else reads the
    // environment while it changes.
    unsafe {
        std::env::set_var("PATH", path);
        std::env::set_var("GEWU_FAKE_GH", &pulls);
    }

    let id = fixture.start(
        Source::PullRequest {
            number: 7,
            remote: "origin".into(),
        },
        none(),
    );
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::Complete, "{:?}", task.message);
    assert_eq!(task.title, "#7 Print receipts");

    let description = root
        .join("home/tasks")
        .join(&id)
        .join("checkout/.gewu/description.md");
    let text = std::fs::read_to_string(&description).expect("description saved");
    assert_eq!(
        text,
        "# #7 Print receipts\n\nPrints receipts 2× faster by reusing the buffer.\n"
    );
}
