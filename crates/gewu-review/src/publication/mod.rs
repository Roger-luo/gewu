//! Where a reviewer's notes go, and the text that would be posted.
//!
//! The draft is composed from the notes themselves, so a preview can show it
//! without asking an agent to write it and without calling GitHub. Sending
//! the draft is a separate step; [`Plan::github_requests`] is the request
//! list that step would make.

mod github;
mod prose;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::{Feedback, FeedbackTarget};

pub use github::GitHubRequest;

/// The repository critiques of Gewu's own walkthrough are filed against.
pub const GEWU_REPOSITORY: &str = "Roger-luo/gewu";

/// Where notes about the code can be published. Notes about the walkthrough
/// always go to [`GEWU_REPOSITORY`], whichever variant this is.
///
/// `repository` is `owner/name` when the repository has a GitHub remote, and
/// only its folder name otherwise; such a plan can be previewed, not sent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Venue {
    PullRequest {
        repository: String,
        number: u64,
    },
    /// Commits that are not part of an open pull request.
    Commits {
        repository: String,
        base: String,
        head: String,
    },
    Directory {
        repository: String,
        path: String,
        #[serde(rename = "ref")]
        reference: String,
    },
    WorkingTree {
        repository: String,
        base: String,
    },
}

impl Venue {
    #[must_use]
    pub fn repository(&self) -> &str {
        match self {
            Self::PullRequest { repository, .. }
            | Self::Commits { repository, .. }
            | Self::Directory { repository, .. }
            | Self::WorkingTree { repository, .. } => repository,
        }
    }
}

/// One note plus the step it was left on.
pub struct Note<'a> {
    pub feedback: &'a Feedback,
    pub chunk_title: &'a str,
}

/// Everything a preview shows, and everything a later publish step would send.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Plan {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<PullRequestDraft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_issues: Option<IssueBundle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gewu_issues: Option<IssueBundle>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PullRequestDraft {
    pub repository: String,
    pub number: u64,
    /// Notes that name a line. Each becomes one review comment.
    pub comments: Vec<InlineComment>,
    /// Notes with no line, written as the review's single summary comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The notes the summary covers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summarized: Vec<String>,
    /// The commit that was reviewed. Line comments are placed on it, so a
    /// push after the review does not shift them onto other lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InlineComment {
    pub feedback_id: String,
    pub path: String,
    pub side: String,
    pub start_line: u32,
    pub end_line: u32,
    pub body: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IssueBundle {
    pub repository: String,
    /// Present when the review raised more than one problem, so the set has a parent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking: Option<IssueDraft>,
    pub issues: Vec<IssueDraft>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IssueDraft {
    /// Absent on the tracking issue, which covers the whole review.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_id: Option<String>,
    pub title: String,
    pub body: String,
}

impl Plan {
    /// Route open notes. Resolved notes are omitted. Each remaining note is one
    /// problem: its own comment or its own issue, with a tracking issue when a
    /// destination has more than one.
    #[must_use]
    pub fn draft(venue: &Venue, review_title: &str, notes: &[Note<'_>]) -> Self {
        let scope = prose::scope(venue);
        let mut comments = Vec::new();
        let mut loose: Vec<&Note<'_>> = Vec::new();
        let mut code = Vec::new();
        let mut gewu = Vec::new();
        for note in notes.iter().filter(|note| !note.feedback.resolved) {
            if note.feedback.target == FeedbackTarget::Explanation {
                gewu.push(prose::issue(note, review_title, &scope, true));
                continue;
            }
            match venue {
                Venue::PullRequest { .. } => match note.feedback.anchor.as_ref() {
                    Some(anchor) => comments.push(InlineComment {
                        feedback_id: note.feedback.id.clone(),
                        path: anchor.path.clone(),
                        side: anchor.side.clone(),
                        start_line: anchor.start_line,
                        end_line: anchor.end_line,
                        body: note.feedback.body.trim().to_string(),
                    }),
                    None => loose.push(note),
                },
                Venue::Commits { .. } | Venue::Directory { .. } | Venue::WorkingTree { .. } => {
                    code.push(prose::issue(note, review_title, &scope, false));
                }
            }
        }
        let pull_request = pull_request(venue, review_title, comments, &loose);
        Self {
            pull_request,
            code_issues: bundle(venue.repository(), code, review_title, false),
            gewu_issues: bundle(GEWU_REPOSITORY, gewu, review_title, true),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pull_request.is_none() && self.code_issues.is_none() && self.gewu_issues.is_none()
    }
}

fn pull_request(
    venue: &Venue,
    review_title: &str,
    comments: Vec<InlineComment>,
    loose: &[&Note<'_>],
) -> Option<PullRequestDraft> {
    let Venue::PullRequest { repository, number } = venue else {
        return None;
    };
    if comments.is_empty() && loose.is_empty() {
        return None;
    }
    let summary = (!loose.is_empty()).then(|| prose::review_summary(review_title, *number, loose));
    Some(PullRequestDraft {
        repository: repository.clone(),
        number: *number,
        comments,
        summary,
        summarized: loose.iter().map(|note| note.feedback.id.clone()).collect(),
        commit_id: None,
    })
}

fn bundle(
    repository: &str,
    issues: Vec<IssueDraft>,
    review_title: &str,
    walkthrough: bool,
) -> Option<IssueBundle> {
    if issues.is_empty() {
        return None;
    }
    let tracking = (issues.len() > 1).then(|| prose::tracking(review_title, walkthrough, &issues));
    Some(IssueBundle {
        repository: repository.to_string(),
        tracking,
        issues,
    })
}
