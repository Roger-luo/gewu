//! The GitHub API requests a draft stands for. Building them sends nothing.

use serde_json::{Value, json};

use super::{IssueBundle, Plan};

/// One request a publish step would make. Tests inspect this instead of
/// calling GitHub.
#[derive(Clone, Debug, PartialEq)]
pub struct GitHubRequest {
    pub method: &'static str,
    pub path: String,
    pub body: Value,
}

impl Plan {
    /// Requests in preview order. `code_owner` owns the reviewed repository
    /// and `gewu_owner` owns the Gewu repository. Issue numbers do not exist
    /// yet, so the tracking issue lists titles.
    #[must_use]
    pub fn github_requests(&self, code_owner: &str, gewu_owner: &str) -> Vec<GitHubRequest> {
        let mut requests = Vec::new();
        if let Some(review) = &self.pull_request {
            let comments: Vec<_> = review
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
                    }
                    item
                })
                .collect();
            let mut body = json!({
                "event": "COMMENT",
                "comments": comments,
            });
            if let Some(summary) = &review.summary {
                body["body"] = json!(summary);
            }
            requests.push(GitHubRequest {
                method: "POST",
                path: format!(
                    "/repos/{code_owner}/{}/pulls/{}/reviews",
                    review.repository, review.number
                ),
                body,
            });
        }
        if let Some(bundle) = &self.code_issues {
            requests.extend(bundle.requests(code_owner));
        }
        if let Some(bundle) = &self.gewu_issues {
            requests.extend(bundle.requests(gewu_owner));
        }
        requests
    }
}

impl IssueBundle {
    fn requests(&self, owner: &str) -> Vec<GitHubRequest> {
        self.issues
            .iter()
            .chain(self.tracking.iter())
            .map(|issue| GitHubRequest {
                method: "POST",
                path: format!("/repos/{owner}/{}/issues", self.repository),
                body: json!({ "title": issue.title, "body": issue.body }),
            })
            .collect()
    }
}

fn github_side(side: &str) -> &'static str {
    if side == "old" { "LEFT" } else { "RIGHT" }
}
