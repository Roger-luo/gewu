//! What a task's notes would become on GitHub, and sending them once the
//! reviewer confirms.

use std::collections::HashSet;
use std::path::PathBuf;

use gewu_app::Snapshot;
use gewu_github::{Posted, Receipt, Recorder, Slug, Transport};
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
        let (snapshot, title, source) = {
            let mut state = self.state()?;
            let task = state.task(task_id)?;
            let snapshot = task
                .review()?
                .ok_or_else(|| WorkbenchError::invalid("no walkthrough has been written yet"))?
                .snapshot();
            (
                snapshot,
                task.record.title.clone(),
                task.record.source.clone(),
            )
        };
        let remote = self.remote(task_id)?;
        let blocked = remote.slug.is_none().then(|| match &remote.url {
            Some(url) => format!(
                "{}'s remote `{}` points at {url}, not github.com, so this draft can be read but \
                 not sent.",
                remote.repository, remote.name
            ),
            None => format!(
                "{} has no GitHub remote named `{}`, so this draft can be read but not sent.",
                remote.repository, remote.name
            ),
        });
        let name = remote
            .slug
            .map_or(remote.repository, |slug| slug.to_string());
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
                failure: Some(blocked),
                ..Receipt::default()
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

    /// The GitHub repository `task_id` publishes to: the remote a pull
    /// request came from, otherwise `origin`.
    pub(super) fn remote(&self, task_id: &str) -> Result<Remote, WorkbenchError> {
        let mut state = self.state()?;
        let record = state.task(task_id)?.record.clone();
        let repository = state.repository(&record.repository_id)?;
        let name = match &record.source {
            Source::PullRequest { remote, .. } => remote.clone(),
            _ => "origin".to_string(),
        };
        let url = repository.git().remote_url(&name);
        let slug = url.as_deref().and_then(Slug::from_remote_url);
        Ok(Remote {
            repository: repository.name(),
            name,
            url,
            slug,
        })
    }

    /// What publishing would send, answered by a [`Recorder`] signed in as
    /// `login`. Nothing reaches GitHub and nothing is marked as sent.
    pub fn rehearse(&self, task_id: &str, login: &str) -> Result<Receipt, WorkbenchError> {
        let publication = self.publication(task_id)?;
        let mut receipt = match publication.blocked {
            Some(blocked) => Receipt {
                failure: Some(blocked),
                ..Receipt::default()
            },
            None => gewu_github::publish(&Recorder::signed_in(login), &publication.plan),
        };
        receipt.rehearsed = true;
        Ok(receipt)
    }

    fn published(&self, task_id: &str) -> Result<Vec<Posted>, WorkbenchError> {
        Ok(Home::read(&self.published_path(task_id))?.unwrap_or_default())
    }

    fn published_path(&self, task_id: &str) -> PathBuf {
        self.inner.home.task(task_id).join("published.json")
    }
}

pub(super) struct Remote {
    /// The repository's display name.
    pub(super) repository: String,
    /// The Git remote, such as `origin`.
    pub(super) name: String,
    /// Where it points, if it exists.
    pub(super) url: Option<String>,
    /// `None` when that remote is not on github.com.
    pub(super) slug: Option<Slug>,
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
