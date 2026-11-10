//! Lean wording for a draft. The reviewer's own sentence stays the point;
//! the added lines say where it was found, and nothing else.

use super::{IssueDraft, Note, Venue};

pub(super) fn scope(venue: &Venue) -> String {
    match venue {
        Venue::PullRequest { repository, number } => {
            format!("pull request #{number} on {repository}")
        }
        Venue::Commits {
            repository,
            base,
            head,
        } => {
            format!("commits {base}..{head} on {repository}")
        }
        Venue::Directory {
            repository,
            path,
            reference,
        } => {
            format!("directory {path} at {reference} on {repository}")
        }
        Venue::WorkingTree { repository, base } => {
            format!("uncommitted changes against {base} on {repository}")
        }
    }
}

pub(super) fn review_summary(review_title: &str, number: u64, notes: &[&Note<'_>]) -> String {
    let mut text = format!(
        "These notes on \"{review_title}\" cover the change as a whole, so they are one review comment on pull request #{number}.\n"
    );
    for note in notes {
        let body = note.feedback.body.trim();
        text.push_str(&format!("\n**{}.** {body}", step(note)));
    }
    text
}

pub(super) fn issue(
    note: &Note<'_>,
    review_title: &str,
    scope: &str,
    walkthrough: bool,
) -> IssueDraft {
    let body_text = note.feedback.body.trim();
    let mut body = format!(
        "{body_text}\n\nNoted on \"{}\" during \"{review_title}\" ({scope}).",
        step(note)
    );
    if let Some(anchor) = note.feedback.anchor.as_ref() {
        let span = span(anchor.start_line, anchor.end_line);
        let version = if anchor.side == "old" {
            "previous version"
        } else {
            "new version"
        };
        body.push_str(&format!(
            "\nIt refers to `{}` {span} of the {version}.",
            anchor.path
        ));
    }
    if walkthrough {
        body.push_str("\nThis is about the walkthrough Gewu wrote.");
    }
    IssueDraft {
        feedback_id: Some(note.feedback.id.clone()),
        title: title_from(body_text),
        body,
    }
}

pub(super) fn tracking(review_title: &str, walkthrough: bool, issues: &[IssueDraft]) -> IssueDraft {
    let heading = if walkthrough {
        "Walkthrough notes"
    } else {
        "Review notes"
    };
    let mut body = format!(
        "{} separate notes from \"{review_title}\". Each has its own issue.\n",
        issues.len()
    );
    for issue in issues {
        body.push_str(&format!("\n- {}", issue.title));
    }
    IssueDraft {
        feedback_id: None,
        title: format!("{heading}: {review_title}"),
        body,
    }
}

fn step<'a>(note: &'a Note<'a>) -> &'a str {
    let title = note.chunk_title.trim();
    if title.is_empty() { "this step" } else { title }
}

fn span(start: u32, end: u32) -> String {
    if start == end {
        format!("line {start}")
    } else {
        format!("lines {start}-{end}")
    }
}

fn title_from(body: &str) -> String {
    let line = body.lines().next().unwrap_or("").trim();
    let sentence = line
        .split(". ")
        .next()
        .unwrap_or(line)
        .trim()
        .trim_end_matches('.');
    let mut out = String::new();
    for (index, ch) in sentence.chars().enumerate() {
        if index == 72 {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    if out.is_empty() {
        "Review note".to_string()
    } else {
        out
    }
}
