//! The reviewer's workbench: repositories, review tasks, and the agents
//! writing them. Every method is safe to call from several threads.

mod actions;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

use gewu_agent::Preset;
use gewu_app::{Action, Snapshot};
use gewu_git::RepositoryRefs;

use crate::{
    TaskLog, TaskStatus, WorkbenchError, WorkbenchSnapshot,
    home::Home,
    log::TaskLogFile,
    repository::RepositoryRecord,
    task::{Task, TaskRecord},
};

pub(crate) struct State {
    pub(crate) repositories: Vec<RepositoryRecord>,
    /// Newest first.
    pub(crate) tasks: Vec<Task>,
}

impl State {
    pub(crate) fn task(&mut self, id: &str) -> Result<&mut Task, WorkbenchError> {
        self.tasks
            .iter_mut()
            .find(|task| task.record.id == id)
            .ok_or_else(|| WorkbenchError::NotFound(format!("review {id}")))
    }

    pub(crate) fn repository(&self, id: &str) -> Result<&RepositoryRecord, WorkbenchError> {
        self.repositories
            .iter()
            .find(|repository| repository.id == id)
            .ok_or_else(|| WorkbenchError::NotFound(format!("repository {id}")))
    }
}

pub(crate) struct Inner {
    pub(crate) home: Home,
    /// The Gewu binary agents run harness commands with.
    pub(crate) gewu: PathBuf,
    pub(crate) state: Mutex<State>,
}

#[derive(Clone)]
pub struct Workbench {
    pub(crate) inner: Arc<Inner>,
}

impl Workbench {
    /// Load the workbench kept in `home`. A batch that was running when the
    /// previous process stopped is marked failed, since nothing runs it now.
    pub fn open(home: Home, gewu: &Path) -> Result<Self, WorkbenchError> {
        let repositories = Home::read(&home.repositories())?.unwrap_or_default();
        let mut tasks = Vec::new();
        if let Ok(entries) = std::fs::read_dir(home.tasks()) {
            for entry in entries.flatten() {
                let directory = entry.path();
                let Some(mut record) = Home::read::<TaskRecord>(&directory.join("task.json"))?
                else {
                    continue;
                };
                if record.status == TaskStatus::Generating {
                    record.set(
                        TaskStatus::Failed,
                        record.stage.clone(),
                        Some(
                            "Gewu closed while this batch was running. Continue to run it again."
                                .to_owned(),
                        ),
                    );
                    Home::write(&directory.join("task.json"), &record)?;
                }
                tasks.push(Task::new(
                    record,
                    TaskLogFile::open(directory.join("log.jsonl")),
                ));
            }
        }
        tasks.sort_by_key(|task| std::cmp::Reverse(task.record.created_at));
        Ok(Self {
            inner: Arc::new(Inner {
                home,
                gewu: gewu.to_path_buf(),
                state: Mutex::new(State {
                    repositories,
                    tasks,
                }),
            }),
        })
    }

    /// Where repositories and tasks are kept.
    #[must_use]
    pub fn home(&self) -> &Path {
        self.inner.home.root()
    }

    pub(crate) fn state(&self) -> Result<MutexGuard<'_, State>, WorkbenchError> {
        self.inner
            .state
            .lock()
            .map_err(|_| WorkbenchError::invalid("workbench state lock poisoned"))
    }

    pub(crate) fn save_task(&self, record: &TaskRecord) -> Result<(), WorkbenchError> {
        Home::write(&self.inner.home.task(&record.id).join("task.json"), record)
    }

    pub(crate) fn save_repositories(&self, state: &State) -> Result<(), WorkbenchError> {
        Home::write(&self.inner.home.repositories(), &state.repositories)
    }

    pub fn snapshot(&self) -> Result<WorkbenchSnapshot, WorkbenchError> {
        let mut state = self.state()?;
        let repositories: Vec<_> = state
            .repositories
            .iter()
            .map(RepositoryRecord::view)
            .collect();
        let names: std::collections::HashMap<_, _> = state
            .repositories
            .iter()
            .map(|repository| (repository.id.clone(), repository.name()))
            .collect();
        let tasks = state
            .tasks
            .iter_mut()
            .map(|task| {
                let name = names
                    .get(&task.record.repository_id)
                    .cloned()
                    .unwrap_or_default();
                task.view(name)
            })
            .collect();
        Ok(WorkbenchSnapshot {
            repositories,
            tasks,
            agents: Preset::views(),
        })
    }

    /// Commits, branches, pull requests, and directories to choose from.
    pub fn refs(&self, repository_id: &str) -> Result<RepositoryRefs, WorkbenchError> {
        let repository = self.state()?.repository(repository_id)?.clone();
        Ok(repository.git().refs()?)
    }

    pub fn log(&self, task_id: &str, after: usize) -> Result<TaskLog, WorkbenchError> {
        Ok(self.state()?.task(task_id)?.log.since(after))
    }

    /// The task's walkthrough, as the review screens render it.
    pub fn task_snapshot(&self, task_id: &str) -> Result<Snapshot, WorkbenchError> {
        let mut state = self.state()?;
        let review = state
            .task(task_id)?
            .review()?
            .ok_or_else(|| WorkbenchError::invalid("no walkthrough has been written yet"))?;
        Ok(review.snapshot())
    }

    pub fn task_dispatch(&self, task_id: &str, action: Action) -> Result<Snapshot, WorkbenchError> {
        let mut state = self.state()?;
        let task = state.task(task_id)?;
        let snapshot = task
            .review()?
            .ok_or_else(|| WorkbenchError::invalid("no walkthrough has been written yet"))?
            .dispatch(action)?;
        task.progress = None;
        self.finish_if_written(task, &snapshot)?;
        Ok(snapshot)
    }
}
