//! The workbench end to end: real Git repositories, the real `gewu` binary,
//! and the stand-in ACP agent in `fixtures/agents`. No model is called.

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use gewu::workbench::{
    AgentChoice, Home, Source, TaskStatus, TaskView, Workbench, WorkbenchAction,
};
use gewu_app::Action;

struct Fixture {
    _directory: tempfile::TempDir,
    repository: PathBuf,
    workbench: Workbench,
}

impl Fixture {
    /// A repository whose last commit changes two files.
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let repository = directory.path().join("project");
        std::fs::create_dir_all(repository.join("src")).expect("project");
        let fixture = Self {
            workbench: Workbench::open(
                Home::new(directory.path().join("home")),
                Path::new(env!("CARGO_BIN_EXE_gewu")),
            )
            .expect("workbench"),
            repository,
            _directory: directory,
        };
        fixture.git(&["init", "-q"]);
        fixture.git(&["config", "user.email", "fixture@example.invalid"]);
        fixture.git(&["config", "user.name", "Fixture"]);
        fixture.write("src/a.rs", "pub const A: u32 = 1;\n");
        fixture.write("src/b.rs", "pub const B: u32 = 1;\n");
        fixture.git(&["add", "."]);
        fixture.git(&["commit", "-qm", "test: base"]);
        fixture.write("src/a.rs", "pub const A: u32 = 2;\n");
        fixture.write("src/b.rs", "pub const B: u32 = 2;\n");
        fixture.git(&["commit", "-qam", "test: head"]);
        fixture
    }

    fn git(&self, args: &[&str]) {
        let _ = self.git_text(args);
    }

    fn git_text(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repository)
            .output()
            .expect("git");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout)
            .expect("utf8")
            .trim()
            .to_owned()
    }

    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.repository.join(path), text).expect("write");
    }

    fn add(&self) -> String {
        let snapshot = self
            .workbench
            .dispatch(WorkbenchAction::AddRepository {
                path: self.repository.display().to_string(),
            })
            .expect("add repository");
        snapshot.repositories[0].id.clone()
    }

    fn start(&self, source: Source, agent: AgentChoice) -> String {
        let repository_id = self.add();
        self.workbench
            .create_task(repository_id, source, agent, None)
            .expect("create task")
    }

    fn task(&self, id: &str) -> TaskView {
        self.workbench
            .snapshot()
            .expect("snapshot")
            .tasks
            .into_iter()
            .find(|task| task.id == id)
            .expect("task")
    }

    /// The task once its batch has ended.
    fn settled(&self, id: &str) -> TaskView {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let task = self.task(id);
            if task.status != TaskStatus::Generating {
                return task;
            }
            assert!(Instant::now() < deadline, "batch did not end: {task:?}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn chunks(&self, id: &str) -> usize {
        self.workbench
            .task_snapshot(id)
            .expect("review")
            .chunks
            .len()
    }
}

fn none() -> AgentChoice {
    AgentChoice {
        preset_id: "none".into(),
        command: None,
    }
}

fn fake(mode: &str) -> AgentChoice {
    slow_fake(mode, 0.0)
}

/// The stand-in agent, pausing `delay` seconds between steps.
fn slow_fake(mode: &str, delay: f32) -> AgentChoice {
    let agent = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/agents/fake_acp_agent.py");
    AgentChoice {
        preset_id: "custom".into(),
        command: Some(format!(
            "env FAKE_AGENT_MODE={mode} FAKE_AGENT_DELAY={delay} python3 {}",
            agent.display()
        )),
    }
}

fn range() -> Source {
    Source::CommitRange {
        base: "HEAD^".into(),
        head: "HEAD".into(),
    }
}

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

#[test]
fn an_acp_agent_writes_one_batch_per_reviewer_gate() {
    let fixture = Fixture::new();
    let id = fixture.start(range(), fake("harness"));
    let first = fixture.settled(&id);
    assert_eq!(first.status, TaskStatus::AwaitingReviewer, "{first:?}");
    assert_eq!(fixture.chunks(&id), 1);
    let log = fixture.workbench.log(&id, 0).expect("log");
    assert!(
        log.events
            .iter()
            .any(|event| event.text.contains("Planning one step per file")),
        "{log:?}"
    );
    let cont = || {
        fixture
            .workbench
            .dispatch(WorkbenchAction::ContinueTask {
                task_id: id.clone(),
            })
            .expect("continue");
        fixture.settled(&id)
    };
    let held = cont();
    assert_eq!(held.status, TaskStatus::AwaitingReviewer);
    assert!(
        held.message
            .as_deref()
            .is_some_and(|message| message.starts_with("1 step is still unreviewed")),
        "{held:?}"
    );
    assert_eq!(fixture.chunks(&id), 1);

    let chunk_id = fixture.workbench.task_snapshot(&id).expect("review").chunks[0]
        .id
        .clone();
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
    let second = cont();
    assert_eq!(second.status, TaskStatus::AwaitingReviewer, "{second:?}");
    assert_eq!(fixture.chunks(&id), 2);
    let newer = fixture.workbench.log(&id, log.next).expect("log");
    assert!(
        newer
            .events
            .iter()
            .any(|event| event.text.contains("src/b.rs")),
        "{newer:?}"
    );

    for chunk in fixture.workbench.task_snapshot(&id).expect("review").chunks {
        fixture
            .workbench
            .task_dispatch(
                &id,
                Action::SetReviewed {
                    chunk_id: chunk.id,
                    reviewed: true,
                },
            )
            .expect("mark reviewed");
    }
    // Reading the last step of a finished walkthrough closes it; nothing is left to continue.
    let last = fixture.task(&id);
    assert_eq!(last.status, TaskStatus::Complete, "{last:?}");
    let progress = last.review.expect("progress");
    assert_eq!((progress.reviewed, progress.total), (2, 2));
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
