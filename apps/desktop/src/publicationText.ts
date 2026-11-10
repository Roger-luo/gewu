import type { Feedback } from "./protocol";
import type { InlineComment, IssueBundle, IssueDraft, PullRequestDraft, Venue } from "./publication";

/** One open note and the step it was left on. */
export interface Note {
  feedback: Feedback;
  chunkTitle: string;
}

export function scope(venue: Venue): string {
  switch (venue.type) {
    case "pull_request":
      return `pull request #${venue.number} on ${venue.repository}`;
    case "commits":
      return `commits ${venue.base}..${venue.head} on ${venue.repository}`;
    case "directory":
      return `directory ${venue.path} at ${venue.reference} on ${venue.repository}`;
    case "working_tree":
      return `uncommitted changes against ${venue.base} on ${venue.repository}`;
  }
}

export function pullRequest(
  venue: Venue,
  reviewTitle: string,
  comments: InlineComment[],
  loose: Note[],
): PullRequestDraft | null {
  if (venue.type !== "pull_request" || (comments.length === 0 && loose.length === 0)) {
    return null;
  }
  const lines = loose.map((note) => `**${step(note)}.** ${note.feedback.body.trim()}`);
  return {
    repository: venue.repository,
    number: venue.number,
    comments,
    summary:
      loose.length === 0
        ? null
        : `These notes on "${reviewTitle}" cover the change as a whole, so they are one review comment on pull request #${venue.number}.\n\n${lines.join("\n")}`,
  };
}

export function issue(
  note: Note,
  reviewTitle: string,
  scopeText: string,
  walkthrough: boolean,
): IssueDraft {
  const bodyText = note.feedback.body.trim();
  const parts = [`${bodyText}\n\nNoted on "${step(note)}" during "${reviewTitle}" (${scopeText}).`];
  const anchor = note.feedback.anchor;
  if (anchor) {
    const span =
      anchor.start_line === anchor.end_line
        ? `line ${anchor.start_line}`
        : `lines ${anchor.start_line}-${anchor.end_line}`;
    const version = anchor.side === "old" ? "previous version" : "new version";
    parts.push(`It refers to \`${anchor.path}\` ${span} of the ${version}.`);
  }
  if (walkthrough) {
    parts.push("This is about the walkthrough Gewu wrote.");
  }
  return { feedbackId: note.feedback.id, title: titleFrom(bodyText), body: parts.join("\n") };
}

export function bundle(
  repository: string,
  issues: IssueDraft[],
  reviewTitle: string,
  walkthrough: boolean,
): IssueBundle | null {
  if (issues.length === 0) {
    return null;
  }
  const heading = walkthrough ? "Walkthrough notes" : "Review notes";
  const tracking =
    issues.length > 1
      ? {
          feedbackId: null,
          title: `${heading}: ${reviewTitle}`,
          body: `${issues.length} separate notes from "${reviewTitle}". Each has its own issue.\n\n${issues
            .map((item) => `- ${item.title}`)
            .join("\n")}`,
        }
      : null;
  return { repository, tracking, issues };
}

function step(note: Note): string {
  const title = note.chunkTitle.trim();
  return title.length === 0 ? "this step" : title;
}

function titleFrom(body: string): string {
  const line = body.split("\n")[0]?.trim() ?? "";
  const sentence = (line.split(". ")[0] ?? line).trim().replace(/\.$/, "");
  if (sentence.length === 0) {
    return "Review note";
  }
  return sentence.length > 72 ? `${sentence.slice(0, 72)}…` : sentence;
}
