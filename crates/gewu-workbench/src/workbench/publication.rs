//! What a task's notes would become on GitHub, and sending them once the
//! reviewer confirms.

use std::collections::HashSet;
use std::path::PathBuf;

use gewu_app::Snapshot;
use gewu_github::{Posted, Receipt, Slug, Transport};
use gewu_review::{GitHubRequest, Note, Plan, Venue};
use serde::{Deserialize, Serialize};

use super::Workbench;
use crate::{Source, WorkbenchError, home::Home};

/// The summary screen's preview: the draft, the requests it stands for, and
/// what earlier runs already posted.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Publication {
    pub plan: Plan,
    pub requests: Vec<GitHubRequest>,
    /// Why this draft cannot be sent, such as a repository with no GitHub
    /// remote. `None` when it can.
    pub blocked: Option<String>,
    /// Posted by earlier runs. Their notes are left out of `plan`.
    pub published: Vec<Posted>,
}

impl Workbench {
    /// The draft for `task_id`. Building it does not contact GitHub.
    pub fn publication(&self, task_id: &str) -> Result<Publication, WorkbenchError> {
        let (snapshot, title, source, repository) = {
            let mut state = self.state()?;
            let task = state.task(task_id)?;
            let snapshot = task
                .review()?
                .ok_or_else(|| WorkbenchError::invalid("no walkthrough has been written yet"))?
                .snapshot();
            let record = task.record.clone();
            let repository = state.repository(&record.repository_id)?.clone();
            (snapshot, record.title, record.source, repository)
        };
        let remote = match &source {
            Source::PullRequest { remote, .. } => remote.as_str(),
            _ => "origin",
        };
        let slug = repository
            .git()
            .remote_url(remote)
            .as_deref()
            .and_then(Slug::from_remote_url);
        let blocked = slug.is_none().then(|| {
            format!(
                "{} has no GitHub remote named `{remote}`, so this draft can be read but not sent.",
                repository.name()
            )
        });
        let name = slug.map_or_else(|| repository.name(), |slug| slug.to_string());
        let published = self.published(task_id)?;
        let mut plan = draft(&snapshot, &venue(&source, name), &title, &published);
        if let Some(review) = plan.pull_request.as_mut() {
            review.commit_id = Some(snapshot.source.head.clone());
        }
        Ok(Publication {
            requests: plan.github_requests(),
            plan,
            blocked,
            published,
        })
    }

    /// Sends `task_id`'s draft and records what was posted, so the same notes
    /// are not sent again. One run at a time; GitHub is called without
    /// holding the workbench lock, so the window keeps updating.
    pub fn publish(
        &self,
        task_id: &str,
        transport: &dyn Transport,
    ) -> Result<Receipt, WorkbenchError> {
        let _running = self
            .inner
            .publishing
            .try_lock()
            .map_err(|_| WorkbenchError::invalid("a publish run is already sending"))?;
        let publication = self.publication(task_id)?;
        if let Some(blocked) = publication.blocked {
            return Ok(Receipt {
                posted: Vec::new(),
                failure: Some(blocked),
            });
        }
        let receipt = gewu_github::publish(transport, &publication.plan);
        if !receipt.posted.is_empty() {
            let mut published = publication.published;
            published.extend(receipt.posted.iter().cloned());
            Home::write(&self.published_path(task_id), &published)?;
        }
        Ok(receipt)
    }

    fn published(&self, task_id: &str) -> Result<Vec<Posted>, WorkbenchError> {
        Ok(Home::read(&self.published_path(task_id))?.unwrap_or_default())
    }

    fn published_path(&self, task_id: &str) -> PathBuf {
        self.inner.home.task(task_id).join("published.json")
    }
}

fn venue(source: &Source, repository: String) -> Venue {
    match source {
        Source::PullRequest { number, .. } => Venue::PullRequest {
            repository,
            number: *number,
        },
        Source::CommitRange { base, head } => Venue::Commits {
            repository,
            base: base.clone(),
            head: head.clone(),
        },
        Source::WorkingTree { base } => Venue::WorkingTree {
            repository,
            base: base.clone(),
        },
        Source::Directory { path, reference } => Venue::Directory {
            repository,
            path: path.clone(),
            reference: reference.clone(),
        },
    }
}

fn draft(snapshot: &Snapshot, venue: &Venue, title: &str, published: &[Posted]) -> Plan {
    let sent: HashSet<&str> = published
        .iter()
        .flat_map(|posted| posted.feedback_ids.iter().map(String::as_str))
        .collect();
    let notes: Vec<Note<'_>> = snapshot
        .feedback
        .iter()
        .filter(|feedback| !sent.contains(feedback.id.as_str()))
        .map(|feedback| Note {
            feedback,
            chunk_title: snapshot
                .chunks
                .iter()
                .find(|chunk| chunk.id == feedback.chunk_id)
                .map_or("", |chunk| chunk.title.as_str()),
        })
        .collect();
    Plan::draft(venue, title, &notes)
}
