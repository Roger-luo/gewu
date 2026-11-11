use gewu_review::{GitHubRequest, IssueBundle, Plan};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{GitHubError, Slug, Transport};

/// One thing that was posted, and the notes it carried.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Posted {
    pub label: String,
    pub url: String,
    pub feedback_ids: Vec<String>,
}

/// What a publish run sent. Requests go one at a time and stop at the first
/// refusal, so `posted` is exactly what exists on GitHub afterwards.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Receipt {
    pub posted: Vec<Posted>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    /// Recorded, not sent: the links are what GitHub would have made.
    #[serde(default)]
    pub rehearsed: bool,
}

/// Sends `plan`: the pull-request review, then each bundle's issues, then
/// its tracking issue linking them by number.
///
/// A plan whose repository is not `owner/name` is refused before anything
/// is sent, because it has no GitHub remote to post to.
pub fn publish(transport: &dyn Transport, plan: &Plan) -> Receipt {
    let mut receipt = Receipt::default();
    if let Some(repository) = repositories(plan).find(|name| Slug::parse(name).is_none()) {
        receipt.failure = Some(format!(
            "Nothing was sent: {repository} has no GitHub remote to publish to."
        ));
        return receipt;
    }
    if let Err(failure) = send_all(transport, plan, &mut receipt) {
        receipt.failure = Some(failure);
    }
    receipt
}

fn repositories(plan: &Plan) -> impl Iterator<Item = &str> {
    let review = plan
        .pull_request
        .iter()
        .map(|review| review.repository.as_str());
    let bundles = [&plan.code_issues, &plan.gewu_issues]
        .into_iter()
        .flatten()
        .map(|bundle| bundle.repository.as_str());
    review.chain(bundles)
}

fn send_all(transport: &dyn Transport, plan: &Plan, receipt: &mut Receipt) -> Result<(), String> {
    if let Some(review) = &plan.pull_request {
        let label = format!("Review on pull request #{}", review.number);
        let reply = send(transport, &review.request(), &label)?;
        let feedback_ids = review
            .comments
            .iter()
            .map(|comment| comment.feedback_id.clone())
            .chain(review.summarized.iter().cloned())
            .collect();
        receipt.posted.push(Posted {
            label,
            url: url(&reply),
            feedback_ids,
        });
    }
    for bundle in [&plan.code_issues, &plan.gewu_issues].into_iter().flatten() {
        send_bundle(transport, bundle, receipt)?;
    }
    Ok(())
}

fn send_bundle(
    transport: &dyn Transport,
    bundle: &IssueBundle,
    receipt: &mut Receipt,
) -> Result<(), String> {
    let mut numbers = Vec::new();
    for issue in &bundle.issues {
        let reply = send(transport, &bundle.request(issue), &issue.title)?;
        let number = reply["number"].as_u64().unwrap_or_default();
        numbers.push(number);
        receipt.posted.push(Posted {
            label: format!("Issue #{number} on {}: {}", bundle.repository, issue.title),
            url: url(&reply),
            feedback_ids: issue.feedback_id.iter().cloned().collect(),
        });
    }
    if let Some(tracking) = bundle.linked_tracking(&numbers) {
        let reply = send(transport, &bundle.request(&tracking), &tracking.title)?;
        let number = reply["number"].as_u64().unwrap_or_default();
        receipt.posted.push(Posted {
            label: format!("Tracking issue #{number} on {}", bundle.repository),
            url: url(&reply),
            feedback_ids: Vec::new(),
        });
    }
    Ok(())
}

fn send(transport: &dyn Transport, request: &GitHubRequest, label: &str) -> Result<Value, String> {
    transport
        .call(&request.method, &request.path, Some(&request.body))
        .map_err(|error| match error {
            GitHubError::SignedOut => {
                format!("{label} was not sent: sign in to GitHub first.")
            }
            error => format!("{label} was not sent: {error}"),
        })
}

fn url(reply: &Value) -> String {
    reply["html_url"].as_str().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests;
