import { bundle, issue, pullRequest, scope, type Note } from "./publicationText";
import type { Feedback, Snapshot } from "./protocol";
import type { Source } from "./workbench/protocol";

/**
 * Drafts of what a review would post. The wording matches
 * `crates/gewu-review/src/publication`, which is what a publish step sends.
 * Building a draft does not call GitHub or an agent.
 */
export const GEWU_REPOSITORY = "gewu";

export type Venue =
  | { type: "pull_request"; repository: string; number: number }
  | { type: "commits"; repository: string; base: string; head: string }
  | { type: "directory"; repository: string; path: string; reference: string }
  | { type: "working_tree"; repository: string; base: string };

export interface InlineComment {
  feedbackId: string;
  path: string;
  side: string;
  startLine: number;
  endLine: number;
  body: string;
}

export interface IssueDraft {
  feedbackId: string | null;
  title: string;
  body: string;
}

export interface IssueBundle {
  repository: string;
  tracking: IssueDraft | null;
  issues: IssueDraft[];
}

export interface PullRequestDraft {
  repository: string;
  number: number;
  comments: InlineComment[];
  summary: string | null;
}

export interface Plan {
  pullRequest: PullRequestDraft | null;
  codeIssues: IssueBundle | null;
  gewuIssues: IssueBundle | null;
}

export function reviewVenue(
  source: Source | null,
  repository: string,
  snapshot: Snapshot,
): Venue {
  switch (source?.type) {
    case "pull_request":
      return { type: "pull_request", repository, number: source.number };
    case "directory":
      return { type: "directory", repository, path: source.path, reference: source.ref };
    case "working_tree":
      return { type: "working_tree", repository, base: source.base };
    case "commit_range":
      return { type: "commits", repository, base: source.base, head: source.head };
    default:
      return {
        type: "commits",
        repository,
        base: snapshot.source.base,
        head: snapshot.source.head,
      };
  }
}

export function draftPlan(
  venue: Venue,
  reviewTitle: string,
  chunks: { id: string; title: string }[],
  feedback: Feedback[],
): Plan {
  const titles = new Map(chunks.map((chunk) => [chunk.id, chunk.title]));
  const notes: Note[] = feedback
    .filter((item) => !item.resolved)
    .map((item) => ({ feedback: item, chunkTitle: titles.get(item.chunk_id) ?? "" }));
  const scopeText = scope(venue);
  const comments: InlineComment[] = [];
  const loose: Note[] = [];
  const code: IssueDraft[] = [];
  const gewu: IssueDraft[] = [];
  for (const note of notes) {
    if (note.feedback.target === "explanation") {
      gewu.push(issue(note, reviewTitle, scopeText, true));
      continue;
    }
    if (venue.type === "pull_request" && note.feedback.anchor) {
      const anchor = note.feedback.anchor;
      comments.push({
        feedbackId: note.feedback.id,
        path: anchor.path,
        side: anchor.side,
        startLine: anchor.start_line,
        endLine: anchor.end_line,
        body: note.feedback.body.trim(),
      });
    } else if (venue.type === "pull_request") {
      loose.push(note);
    } else {
      code.push(issue(note, reviewTitle, scopeText, false));
    }
  }
  return {
    pullRequest: pullRequest(venue, reviewTitle, comments, loose),
    codeIssues: bundle(venue.repository, code, reviewTitle, false),
    gewuIssues: bundle(GEWU_REPOSITORY, gewu, reviewTitle, true),
  };
}

export function planIsEmpty(plan: Plan): boolean {
  return !plan.pullRequest && !plan.codeIssues && !plan.gewuIssues;
}
