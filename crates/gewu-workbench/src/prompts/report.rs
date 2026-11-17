//! A reader's report about a prompt, filed as a public issue on Gewu's own
//! repository like a note about a walkthrough.

use gewu_github::{Receipt, Recorder, Transport};
use gewu_prompts::Prompt;
use gewu_review::{GEWU_REPOSITORY, IssueBundle, IssueDraft, Plan};
use serde::{Deserialize, Serialize};

use crate::WorkbenchError;

/// The longest issue title, before the reader's words are cut.
const TITLE_WORDS: usize = 60;

/// What the reader saw wrong in one prompt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptReport {
    /// The prompt's file stem, e.g. `expand`.
    pub prompt_id: String,
    /// The backend the reader was looking at it for, if any.
    #[serde(default)]
    pub backend: Option<String>,
    /// The reader's own words; the only free text the issue carries.
    pub body: String,
}

impl PromptReport {
    /// The issue it files, refused before anything is sent when the prompt
    /// is unknown or nothing was written.
    pub fn plan(&self) -> Result<Plan, WorkbenchError> {
        let prompt = Prompt::find(&self.prompt_id).ok_or_else(|| {
            WorkbenchError::invalid(format!("there is no `{}` prompt", self.prompt_id))
        })?;
        let words = self.body.trim();
        if words.is_empty() {
            return Err(WorkbenchError::invalid("say what is wrong with the prompt"));
        }
        let backend = self
            .backend
            .as_deref()
            .filter(|backend| !backend.is_empty());
        let sent_to = backend.map_or_else(
            || "as every agent gets it".to_owned(),
            |backend| format!("as sent to `{backend}`"),
        );
        let body = format!(
            "Reported from Gewu {version}'s Settings about the **{title}** prompt \
             (`harness/prompts/{id}.md`), {sent_to}.\n\n{words}\n",
            version = env!("CARGO_PKG_VERSION"),
            title = prompt.title(),
            id = prompt.id(),
        );
        Ok(Plan {
            pull_request: None,
            code_issues: None,
            gewu_issues: Some(IssueBundle {
                repository: GEWU_REPOSITORY.to_owned(),
                tracking: None,
                issues: vec![IssueDraft {
                    feedback_id: None,
                    title: format!("Prompt `{}`: {}", prompt.id(), headline(words)),
                    body,
                }],
            }),
        })
    }

    /// Files the issue through `transport`.
    pub fn send(&self, transport: &dyn Transport) -> Result<Receipt, WorkbenchError> {
        Ok(gewu_github::publish(transport, &self.plan()?))
    }

    /// What filing it would send, answered by a [`Recorder`] signed in as
    /// `login`. Nothing reaches GitHub.
    pub fn rehearse(&self, login: &str) -> Result<Receipt, WorkbenchError> {
        let mut receipt = self.send(&Recorder::signed_in(login))?;
        receipt.rehearsed = true;
        Ok(receipt)
    }
}

/// The report's first line, cut at a word to fit a title.
fn headline(words: &str) -> String {
    let line = words.lines().next().unwrap_or_default().trim();
    if line.chars().count() <= TITLE_WORDS {
        return line.to_owned();
    }
    let cut: String = line.chars().take(TITLE_WORDS).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use gewu_review::GEWU_REPOSITORY;

    use super::PromptReport;

    fn report(prompt: &str, backend: Option<&str>, body: &str) -> PromptReport {
        PromptReport {
            prompt_id: prompt.to_owned(),
            backend: backend.map(str::to_owned),
            body: body.to_owned(),
        }
    }

    #[test]
    fn files_one_issue_on_gewu_naming_the_prompt_and_backend() {
        let plan = report(
            "expand",
            Some("codex"),
            "  The rubric contradicts itself.\nMore.  ",
        )
        .plan()
        .expect("plan");
        let bundle = plan.gewu_issues.expect("a gewu issue");
        assert_eq!(bundle.repository, GEWU_REPOSITORY);
        assert!(plan.pull_request.is_none() && plan.code_issues.is_none());
        let [issue] = bundle.issues.as_slice() else {
            panic!("one issue");
        };
        assert_eq!(
            issue.title,
            "Prompt `expand`: The rubric contradicts itself."
        );
        assert!(issue.body.contains("`harness/prompts/expand.md`"));
        assert!(issue.body.contains("as sent to `codex`"));
        assert!(
            issue
                .body
                .ends_with("The rubric contradicts itself.\nMore.\n")
        );
    }

    #[test]
    fn a_long_first_line_is_cut_at_a_word() {
        let long = "word ".repeat(30);
        let plan = report("plan", None, &long).plan().expect("plan");
        let title = &plan.gewu_issues.expect("issue").issues[0].title;
        assert!(title.ends_with("word…"), "{title}");
        assert!(title.chars().count() < 80);
    }

    #[test]
    fn refuses_an_unknown_prompt_or_an_empty_report() {
        assert!(report("nope", None, "x").plan().is_err());
        assert!(report("plan", None, "  \n ").plan().is_err());
    }

    #[test]
    fn a_rehearsal_sends_nothing_and_says_so() {
        let receipt = report("plan", None, "Too long.")
            .rehearse("ada")
            .expect("receipt");
        assert!(receipt.rehearsed);
        assert_eq!(receipt.posted.len(), 1);
        assert!(receipt.failure.is_none());
    }
}
