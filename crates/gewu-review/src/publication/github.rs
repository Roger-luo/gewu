//! The GitHub API requests a draft stands for. Building them sends nothing.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{IssueBundle, IssueDraft, Plan, PullRequestDraft};

/// One request a publish step would make. Tests inspect this instead of
/// calling GitHub.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GitHubRequest {
    pub method: String,
    pub path: String,
    pub body: Value,
}

impl GitHubRequest {
    fn post(path: String, body: Value) -> Self {
        Self {
            method: "POST".to_string(),
            path,
            body,
        }
    }
}

impl Plan {
    /// Requests in preview order. Issue numbers do not exist yet, so each
    /// tracking issue lists titles; a publish step sends
    /// [`IssueBundle::linked_tracking`] instead once the issues exist.
    #[must_use]
    pub fn github_requests(&self) -> Vec<GitHubRequest> {
        let mut requests: Vec<_> = self
            .pull_request
            .iter()
            .map(PullRequestDraft::request)
            .collect();
        for bundle in [&self.code_issues, &self.gewu_issues].into_iter().flatten() {
            requests.extend(bundle.issues.iter().map(|issue| bundle.request(issue)));
            requests.extend(bundle.tracking.iter().map(|issue| bundle.request(issue)));
        }
        requests
    }
}

impl PullRequestDraft {
    /// One review holding every inline comment, with the summary as its body.
    #[must_use]
    pub fn request(&self) -> GitHubRequest {
        let comments: Vec<_> = self
            .comments
            .iter()
            .map(|comment| {
                let mut item = json!({
                    "path": comment.path,
                    "body": comment.body,
                    "side": github_side(&comment.side),
                    "line": comment.end_line,
                });
                if comment.start_line != comment.end_line {
                    item["start_line"] = json!(comment.start_line);
                    item["start_side"] = json!(github_side(&comment.side));
                }
                item
            })
            .collect();
        // GitHub requires a body on a `COMMENT` review, even one that only
        // carries line comments.
        let summary = self.summary.clone().unwrap_or_else(|| {
            let count = self.comments.len();
            let noun = if count == 1 { "comment" } else { "comments" };
            format!("{count} line {noun} from a review in Gewu.")
        });
        let mut body = json!({ "event": "COMMENT", "body": summary, "comments": comments });
        if let Some(commit) = &self.commit_id {
            body["commit_id"] = json!(commit);
        }
        GitHubRequest::post(
            format!("/repos/{}/pulls/{}/reviews", self.repository, self.number),
            body,
        )
    }
}

impl IssueBundle {
    /// The request that opens `issue` in this bundle's repository.
    #[must_use]
    pub fn request(&self, issue: &IssueDraft) -> GitHubRequest {
        GitHubRequest::post(
            format!("/repos/{}/issues", self.repository),
            json!({ "title": issue.title, "body": issue.body }),
        )
    }

    /// The tracking issue once its issues exist: `numbers[i]` is the number
    /// GitHub gave `issues[i]`, so each line links the issue it names.
    #[must_use]
    pub fn linked_tracking(&self, numbers: &[u64]) -> Option<IssueDraft> {
        let tracking = self.tracking.as_ref()?;
        let mut numbered = self.issues.iter().zip(numbers);
        let body = tracking
            .body
            .lines()
            .map(|line| match line.strip_prefix("- ") {
                Some(title) => match numbered.next() {
                    Some((issue, number)) if issue.title == title => {
                        format!("- [ ] #{number} {title}")
                    }
                    _ => line.to_string(),
                },
                None => line.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(IssueDraft {
            feedback_id: None,
            title: tracking.title.clone(),
            body,
        })
    }
}

fn github_side(side: &str) -> &'static str {
    if side == "old" { "LEFT" } else { "RIGHT" }
}
