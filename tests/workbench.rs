//! The workbench end to end: real Git repositories, the real `gewu` binary,
//! and the stand-in ACP agent in `fixtures/agents`. No model is called.

mod support;

use std::{path::Path, time::Duration};

use gewu::workbench::{Home, Settings, Source, TaskStatus, Workbench, WorkbenchAction};
use gewu_app::Action;
use gewu_review::{FeedbackKind, FeedbackTarget};
use support::{Fixture, fake, none, range, slow_fake};

#[test]
fn a_commit_range_without_an_agent_opens_as_a_raw_diff_in_its_own_checkout() {
    let fixture = Fixture::new();
    let id = fixture.start(range(), none());
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::Complete, "{task:?}");
    assert_eq!(fixture.chunks(&id), 2);
    assert!(
        !fixture.repository.join(".gewu").exists(),
        "a range review must not write into the reviewer's checkout"
    );
    fixture
        .workbench
        .dispatch(WorkbenchAction::DeleteTask { task_id: id })
        .expect("delete");
    assert!(
        fixture
            .workbench
            .snapshot()
            .expect("snapshot")
            .tasks
            .is_empty()
    );
}

/// Asked for by the reviewer after a Cursor run of #276 made them wait
/// minutes after every section: the agent writes one batch ahead, and
/// finishing a batch starts the next without pressing Continue.
#[test]
fn an_acp_agent_writes_one_batch_ahead_of_the_reviewer() {
    let fixture = Fixture::new();
    fixture.write("src/a.rs", "pub const A: u32 = 3;\n");
    fixture.write("src/b.rs", "pub const B: u32 = 3;\n");
    fixture.write("src/c.rs", "pub const C: u32 = 3;\n");
    fixture.git(&["add", "-A"]);
    fixture.git(&["commit", "-qm", "test: three files"]);
    let id = fixture.start(range(), fake("harness"));

    // The fake stops after each batch; it is started again for the one ahead.
    let first = fixture.settled(&id);
    assert_eq!(first.status, TaskStatus::AwaitingReviewer, "{first:?}");
    assert_eq!(fixture.chunks(&id), 2, "the second batch is written ahead");
    let log = fixture.workbench.log(&id, 0).expect("log");
    assert!(
        log.events
            .iter()
            .any(|event| event.text.contains("Planning one step per file")),
        "{log:?}"
    );

    // Continue is still held while the first batch is unread.
    fixture
        .workbench
        .dispatch(WorkbenchAction::ContinueTask {
            task_id: id.clone(),
        })
        .expect("continue");
    let held = fixture.settled(&id);
    assert!(
        held.message
            .as_deref()
            .is_some_and(|message| message.starts_with("2 steps are still unreviewed")),
        "{held:?}"
    );
    assert_eq!(fixture.chunks(&id), 2);

    // Reading the first batch starts the third, with no Continue.
    let mark = |chunk_id: String| {
        fixture
            .workbench
            .task_dispatch(
                &id,
                Action::SetReviewed {
                    chunk_id,
                    reviewed: true,
                },
            )
            .expect("mark reviewed");
    };
    let chunks = fixture.workbench.task_snapshot(&id).expect("review").chunks;
    mark(chunks[0].id.clone());
    let third = fixture.settled(&id);
    assert_eq!(third.status, TaskStatus::AwaitingReviewer, "{third:?}");
    assert_eq!(fixture.chunks(&id), 3);

    for chunk in fixture.workbench.task_snapshot(&id).expect("review").chunks {
        mark(chunk.id);
    }
    // Reading the last step of a finished walkthrough closes it; nothing is left to continue.
    let last = fixture.task(&id);
    assert_eq!(last.status, TaskStatus::Complete, "{last:?}");
    let progress = last.review.expect("progress");
    assert_eq!((progress.reviewed, progress.total), (3, 3));
}

/// A run writing ahead accepts a batch and goes on to the next; the reviewer
/// sees the accepted one while the run is still going, not when it ends.
#[test]
fn a_batch_accepted_mid_run_is_readable_before_the_run_ends() {
    let fixture = Fixture::new();
    fixture.write("src/a.rs", "pub const A: u32 = 3;\n");
    fixture.write("src/b.rs", "pub const B: u32 = 3;\n");
    fixture.write("src/c.rs", "pub const C: u32 = 3;\n");
    fixture.git(&["add", "-A"]);
    fixture.git(&["commit", "-qm", "test: three files"]);
    // Without proofreading, `follow` keeps writing in one session, as a real
    // agent does, so no new author start reloads the review in between.
    fixture
        .workbench
        .dispatch(WorkbenchAction::UpdateSettings {
            settings: Settings {
                proofread_rounds: 0,
                default_agent: None,
                explain_tests: false,
            },
        })
        .expect("settings");
    let id = fixture.start(range(), slow_fake("follow", 2.0));
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let wait_for = |count: usize| loop {
        if let Ok(review) = fixture.workbench.task_snapshot(&id)
            && review.chunks.len() >= count
        {
            return review;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fewer than {count} chunks"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let first = wait_for(1);
    fixture
        .workbench
        .task_dispatch(
            &id,
            Action::SetReviewed {
                chunk_id: first.chunks[0].id.clone(),
                reviewed: true,
            },
        )
        .expect("mark reviewed");

    // Batch two is accepted while the run goes on to write batch three.
    wait_for(2);
    assert_eq!(fixture.task(&id).status, TaskStatus::Generating);
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::AwaitingReviewer, "{task:?}");
    assert_eq!(fixture.chunks(&id), 3);
    let log = fixture.workbench.log(&id, 0).expect("log");
    let starts = log
        .events
        .iter()
        .filter(|event| event.text.starts_with("Starting the agent"))
        .count();
    assert_eq!(starts, 1, "one session wrote all three: {log:?}");
}

/// Found by reviewing the write-ahead change: feedback left on a batch while
/// the agent writes the next one ahead used to fail the task when that run
/// ended, with a garbled message. It waits for the reviewer instead.
#[test]
fn feedback_left_while_the_agent_writes_ahead_waits_for_the_reviewer() {
    let fixture = Fixture::new();
    let id = fixture.start(range(), slow_fake("harness", 1.5));
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let chunk_id = loop {
        if let Ok(review) = fixture.workbench.task_snapshot(&id)
            && let Some(chunk) = review.chunks.first()
        {
            break chunk.id.clone();
        }
        assert!(std::time::Instant::now() < deadline, "no batch was written");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(fixture.task(&id).status, TaskStatus::Generating);
    fixture
        .workbench
        .task_dispatch(
            &id,
            Action::AddFeedback {
                chunk_id,
                kind: FeedbackKind::Issue,
                target: FeedbackTarget::Explanation,
                body: "Say why A changed.".into(),
                anchor: None,
            },
        )
        .expect("feedback");

    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::AwaitingReviewer, "{task:?}");
    assert!(
        task.message
            .as_deref()
            .is_some_and(|message| message.contains("feedback is waiting")),
        "{task:?}"
    );
}

#[test]
fn an_agent_that_edits_the_project_fails_the_task() {
    let fixture = Fixture::new();
    let id = fixture.start(range(), fake("meddle"));
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::Failed);
    assert!(
        task.message
            .as_deref()
            .is_some_and(|message| message.contains("outside .gewu/")),
        "{task:?}"
    );
}

#[test]
fn a_cancelled_batch_says_so_rather_than_reporting_a_crash() {
    let fixture = Fixture::new();
    let id = fixture.start(range(), slow_fake("harness", 3.0));
    std::thread::sleep(Duration::from_millis(1000));
    fixture
        .workbench
        .dispatch(WorkbenchAction::CancelTask {
            task_id: id.clone(),
        })
        .expect("cancel");
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::Cancelled);
    assert_eq!(
        task.message.as_deref(),
        Some("Cancelled. Continue to resume.")
    );
}

#[test]
fn uncommitted_changes_are_reviewed_in_place_once_per_repository() {
    let fixture = Fixture::new();
    fixture.write("src/a.rs", "pub const A: u32 = 3;\n");
    let source = Source::WorkingTree {
        base: "HEAD".into(),
    };
    let id = fixture.start(source.clone(), none());
    assert_eq!(fixture.settled(&id).status, TaskStatus::Complete);
    assert_eq!(fixture.chunks(&id), 1);
    let repository_id = fixture.add();
    let second = fixture
        .workbench
        .create_task(repository_id, source, none(), None);
    assert!(second.is_err(), "a second in-place review must be refused");
}

#[test]
fn a_directory_is_reviewed_as_if_newly_added() {
    let fixture = Fixture::new();
    let source = Source::Directory {
        path: "src".into(),
        reference: "HEAD".into(),
    };
    let id = fixture.start(source, none());
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::Complete, "{task:?}");
    assert_eq!(task.title, "src at HEAD");
    assert_eq!(fixture.chunks(&id), 2);
}

#[test]
fn a_missing_directory_fails_with_a_reason() {
    let fixture = Fixture::new();
    let source = Source::Directory {
        path: "nowhere".into(),
        reference: "HEAD".into(),
    };
    let id = fixture.start(source, none());
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::Failed);
    assert!(
        task.message
            .unwrap_or_default()
            .contains("no directory `nowhere`")
    );
}

#[test]
fn removing_a_repository_deletes_its_reviews() {
    let fixture = Fixture::new();
    let id = fixture.start(range(), none());
    fixture.settled(&id);
    let repository_id = fixture.workbench.snapshot().expect("snapshot").repositories[0]
        .id
        .clone();
    let snapshot = fixture
        .workbench
        .dispatch(WorkbenchAction::RemoveRepository { repository_id })
        .expect("remove repository");
    assert!(snapshot.repositories.is_empty(), "{snapshot:?}");
    assert!(snapshot.tasks.is_empty(), "{snapshot:?}");
    assert!(fixture.repository.join("src/a.rs").exists());
}

#[test]
fn reviews_survive_a_restart() {
    let fixture = Fixture::new();
    let id = fixture.start(range(), none());
    fixture.settled(&id);
    let home = fixture.workbench.home().to_path_buf();
    let reopened =
        Workbench::open(Home::new(home), Path::new(env!("CARGO_BIN_EXE_gewu"))).expect("reopen");
    let snapshot = reopened.snapshot().expect("snapshot");
    assert_eq!(snapshot.repositories.len(), 1);
    assert_eq!(snapshot.tasks[0].id, id);
    assert_eq!(reopened.task_snapshot(&id).expect("review").chunks.len(), 2);
}

#[test]
fn a_root_commit_reviews_against_the_empty_tree() {
    let fixture = Fixture::new();
    let root = fixture.git_text(&["rev-list", "--max-parents=0", "HEAD"]);
    let id = fixture.start(
        Source::CommitRange {
            base: "4b825dc642cb6eb9a060e54bf8d69288fbee4904".into(),
            head: root,
        },
        none(),
    );
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::Complete, "{task:?}");
    assert!(fixture.chunks(&id) >= 1, "{task:?}");
}

/// Found by QA: two reviews of `HEAD~1..HEAD` could not be told apart in the
/// list, so a range is named by its head commit and repeats are numbered.
#[test]
fn reviews_of_one_range_read_apart() {
    let fixture = Fixture::new();
    let first = fixture.start(range(), none());
    let first = fixture.settled(&first);
    let second = fixture.start(range(), none());
    let second = fixture.settled(&second);
    assert!(first.title.starts_with("test: head ("), "{}", first.title);
    assert_eq!(second.title, format!("{} · 2", first.title));
}

/// Found by the final UI judge: an empty range named the commit `HEAD`
/// resolved to, not what the reader typed.
#[test]
fn an_empty_range_names_the_base_the_reader_typed() {
    let fixture = Fixture::new();
    let id = fixture.start(
        Source::CommitRange {
            base: "HEAD".into(),
            head: "HEAD".into(),
        },
        none(),
    );
    let task = fixture.settled(&id);
    assert_eq!(task.status, TaskStatus::Failed);
    let message = task.message.expect("message");
    assert!(
        message.starts_with("Nothing to review from HEAD:"),
        "{message}"
    );
}
