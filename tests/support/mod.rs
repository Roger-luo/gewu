//! A Git repository and a workbench in a temporary directory, shared by the
//! workbench integration tests.

#![allow(dead_code)]

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use gewu::workbench::{
    AgentChoice, Home, Source, TaskStatus, TaskView, Workbench, WorkbenchAction,
};

pub struct Fixture {
    _directory: tempfile::TempDir,
    pub repository: PathBuf,
    pub workbench: Workbench,
}

impl Fixture {
    /// A repository whose last commit changes two files.
    pub fn new() -> Self {
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

    pub fn git(&self, args: &[&str]) {
        let _ = self.git_text(args);
    }

    pub fn git_text(&self, args: &[&str]) -> String {
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

    pub fn write(&self, path: &str, text: &str) {
        std::fs::write(self.repository.join(path), text).expect("write");
    }

    pub fn add(&self) -> String {
        let snapshot = self
            .workbench
            .dispatch(WorkbenchAction::AddRepository {
                path: self.repository.display().to_string(),
            })
            .expect("add repository");
        snapshot.repositories[0].id.clone()
    }

    pub fn start(&self, source: Source, agent: AgentChoice) -> String {
        let repository_id = self.add();
        self.workbench
            .create_task(repository_id, source, agent, None)
            .expect("create task")
    }

    pub fn task(&self, id: &str) -> TaskView {
        self.workbench
            .snapshot()
            .expect("snapshot")
            .tasks
            .into_iter()
            .find(|task| task.id == id)
            .expect("task")
    }

    /// The task once its batch has ended.
    pub fn settled(&self, id: &str) -> TaskView {
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

    /// Removes a task's preparation version on disk and reopens the
    /// workbench, the way a task an older Gewu prepared was recorded.
    pub fn forget_preparation(&mut self, id: &str) {
        let home = self._directory.path().join("home");
        let path = home.join("tasks").join(id).join("task.json");
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("task")).expect("json");
        record.as_object_mut().expect("record").remove("prepared");
        std::fs::write(&path, serde_json::to_vec(&record).expect("json")).expect("write");
        self.workbench = Workbench::open(Home::new(home), Path::new(env!("CARGO_BIN_EXE_gewu")))
            .expect("reopen");
    }

    /// Rewrites a task's source on disk and reopens the workbench, the way a
    /// pull-request task would have been recorded.
    pub fn set_source(&mut self, id: &str, source: Source) {
        let home = self._directory.path().join("home");
        let path = home.join("tasks").join(id).join("task.json");
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("task")).expect("json");
        record["source"] = serde_json::to_value(source).expect("source");
        std::fs::write(&path, serde_json::to_vec(&record).expect("json")).expect("write");
        self.workbench = Workbench::open(Home::new(home), Path::new(env!("CARGO_BIN_EXE_gewu")))
            .expect("reopen");
    }

    pub fn chunks(&self, id: &str) -> usize {
        self.workbench
            .task_snapshot(id)
            .expect("review")
            .chunks
            .len()
    }
}

pub fn none() -> AgentChoice {
    AgentChoice {
        preset_id: "none".into(),
        command: None,
    }
}

pub fn fake(mode: &str) -> AgentChoice {
    slow_fake(mode, 0.0)
}

/// The stand-in agent, pausing `delay` seconds between steps.
pub fn slow_fake(mode: &str, delay: f32) -> AgentChoice {
    let agent = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/agents/fake_acp_agent.py");
    AgentChoice {
        preset_id: "custom".into(),
        command: Some(format!(
            "env FAKE_AGENT_MODE={mode} FAKE_AGENT_DELAY={delay} python3 {}",
            agent.display()
        )),
    }
}

pub fn range() -> Source {
    Source::CommitRange {
        base: "HEAD^".into(),
        head: "HEAD".into(),
    }
}
