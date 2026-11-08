use gewu_agent::{Cancel, PresetKind};

use super::Workbench;
use crate::{
    AgentChoice, Source, TaskStatus, WorkbenchAction, WorkbenchError, WorkbenchSnapshot,
    home::now,
    log::TaskLogFile,
    repository::RepositoryRecord,
    task::{Task, TaskRecord},
};

impl Workbench {
    pub fn dispatch(&self, action: WorkbenchAction) -> Result<WorkbenchSnapshot, WorkbenchError> {
        match action {
            WorkbenchAction::AddRepository { path } => self.add_repository(&path)?,
            WorkbenchAction::RemoveRepository { repository_id } => {
                self.remove_repository(&repository_id)?;
            }
            WorkbenchAction::CreateTask {
                repository_id,
                source,
                agent,
                title,
            } => {
                self.create_task(repository_id, source, agent, title)?;
            }
            WorkbenchAction::ContinueTask { task_id } => self.continue_task(&task_id)?,
            WorkbenchAction::CancelTask { task_id } => self.cancel_task(&task_id)?,
            WorkbenchAction::DeleteTask { task_id } => self.delete_task(&task_id)?,
        }
        self.snapshot()
    }

    fn add_repository(&self, path: &str) -> Result<(), WorkbenchError> {
        let record = RepositoryRecord::locate(path)?;
        let mut state = self.state()?;
        if state.repositories.iter().all(|known| known.id != record.id) {
            state.repositories.push(record);
            self.save_repositories(&state)?;
        }
        Ok(())
    }

    /// Delete the repository's reviews, then forget it; its own files are untouched.
    fn remove_repository(&self, repository_id: &str) -> Result<(), WorkbenchError> {
        let reviews: Vec<String> = {
            let state = self.state()?;
            state.repository(repository_id)?;
            state
                .tasks
                .iter()
                .filter(|task| task.record.repository_id == repository_id)
                .map(|task| task.record.id.clone())
                .collect()
        };
        for task_id in &reviews {
            self.delete_task(task_id)?;
        }
        let mut state = self.state()?;
        state.repositories.retain(|known| known.id != repository_id);
        self.save_repositories(&state)
    }

    /// The id of the new task, which starts its first batch at once.
    pub fn create_task(
        &self,
        repository_id: String,
        source: Source,
        agent: AgentChoice,
        title: Option<String>,
    ) -> Result<String, WorkbenchError> {
        source.validate()?;
        let preset = gewu_agent::Preset::find(&agent.preset_id).ok_or_else(|| {
            WorkbenchError::invalid(format!("unknown agent `{}`", agent.preset_id))
        })?;
        if preset.kind == PresetKind::Acp && preset.command(agent.command.as_deref()).is_empty() {
            return Err(WorkbenchError::invalid(
                "enter the command that starts the agent",
            ));
        }
        let mut state = self.state()?;
        let repository = state.repository(&repository_id)?;
        if source.in_place()
            && state.tasks.iter().any(|task| {
                task.record.repository_id == repository_id && task.record.source.in_place()
            })
        {
            return Err(WorkbenchError::invalid(format!(
                "{} already has a review of its uncommitted changes; open or delete it first",
                repository.name()
            )));
        }
        let created_at = now();
        let id = format!("task-{created_at:x}-{}", state.tasks.len());
        let titled = title.as_ref().is_some_and(|title| !title.trim().is_empty());
        let record = TaskRecord {
            id: id.clone(),
            repository_id,
            title: title
                .filter(|_| titled)
                .map_or_else(|| source.title(), |title| title.trim().to_owned()),
            titled,
            source,
            agent,
            status: TaskStatus::Generating,
            stage: None,
            message: Some("Preparing the review".to_owned()),
            root: None,
            isolated: false,
            created_at,
            updated_at: created_at,
        };
        self.save_task(&record)?;
        let log = TaskLogFile::open(self.inner.home.task(&id).join("log.jsonl"));
        state.tasks.insert(0, Task::new(record, log));
        drop(state);
        self.start_batch(&id)?;
        Ok(id)
    }

    fn continue_task(&self, task_id: &str) -> Result<(), WorkbenchError> {
        let status = self.state()?.task(task_id)?.record.status;
        match status {
            TaskStatus::Generating => Err(WorkbenchError::invalid("a batch is already running")),
            TaskStatus::Complete => Err(WorkbenchError::invalid("the walkthrough is complete")),
            _ => self.start_batch(task_id),
        }
    }

    fn cancel_task(&self, task_id: &str) -> Result<(), WorkbenchError> {
        let mut state = self.state()?;
        let task = state.task(task_id)?;
        let cancel = task
            .cancel
            .as_ref()
            .ok_or_else(|| WorkbenchError::invalid("no batch is running"))?;
        cancel.cancel();
        task.record.message = Some("Cancelling".to_owned());
        Ok(())
    }

    fn delete_task(&self, task_id: &str) -> Result<(), WorkbenchError> {
        let mut state = self.state()?;
        let task = state.task(task_id)?;
        if let Some(cancel) = &task.cancel {
            cancel.cancel();
        }
        let record = task.record.clone();
        state.tasks.retain(|task| task.record.id != task_id);
        let repository = state.repository(&record.repository_id).ok().cloned();
        drop(state);
        if let (Some(root), Some(repository)) = (&record.root, repository) {
            if record.isolated {
                repository.git().remove_worktree(root)?;
            } else {
                gewu_harness::HarnessRunner::in_project(
                    root,
                    gewu_harness::DEFAULT_MAX_CHUNKS,
                    false,
                )
                .reset()?;
            }
        }
        let directory = self.inner.home.task(task_id);
        if directory.exists() {
            std::fs::remove_dir_all(directory)?;
        }
        Ok(())
    }

    /// Mark the task generating and run one batch on a thread of its own.
    pub(crate) fn start_batch(&self, task_id: &str) -> Result<(), WorkbenchError> {
        let cancel = Cancel::default();
        {
            let mut state = self.state()?;
            let task = state.task(task_id)?;
            let message = match task.record.root {
                Some(_) => "Preparing the next batch",
                None => "Preparing the review",
            };
            task.record.set(
                TaskStatus::Generating,
                task.record.stage.clone(),
                Some(message.to_owned()),
            );
            task.cancel = Some(cancel.clone());
            self.save_task(&task.record)?;
        }
        let workbench = self.clone();
        let id = task_id.to_owned();
        std::thread::Builder::new()
            .name(format!("gewu-{id}"))
            .spawn(move || workbench.run_batch(&id, &cancel))?;
        Ok(())
    }
}
