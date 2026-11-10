import { bundle, issue, pullRequest, scope, type Note } from "./publicationText";
import type { Feedback, Snapshot } from "./protocol";
import type { Source } from "./workbench/protocol";

/**
 * What a review would post, in the JSON shape of
 * `crates/gewu-review/src/publication`. A workbench task gets its draft from
 * the backend; the mock and a single-review window build the same draft here.
 * Building a draft does not call GitHub or an agent.
 */
export const GEWU_REPOSITORY = "Roger-luo/gewu";

export type Venue =
  | { type: "pull_request"; repository: string; number: number }
  | { type: "commits"; repository: string; base: string; head: string }
  | { type: "directory"; repository: string; path: string; reference: string }
  | { type: "working_tree"; repository: string; base: string };

export interface InlineComment {
  feedback_id: string;
  path: string;
  side: string;
  start_line: number;
  end_line: number;
  body: string;
}

export interface IssueDraft {
  feedback_id?: string;
  title: string;
  body: string;
}

export interface IssueBundle {
  repository: string;
  tracking?: IssueDraft;
  issues: IssueDraft[];
}

export interface PullRequestDraft {
  repository: string;
  number: number;
  comments: InlineComment[];
  summary?: string;
  summarized?: string[];
  /** The reviewed commit the line comments are placed on. */
  commit_id?: string;
}

export interface Plan {
  pull_request?: PullRequestDraft;
  code_issues?: IssueBundle;
  gewu_issues?: IssueBundle;
}

export interface GitHubRequest {
  method: string;
  path: string;
  body: unknown;
}

/** One thing a publish run posted, and the notes it carried. */
export interface Posted {
  label: string;
  url: string;
  feedback_ids: string[];
}

export interface Receipt {
  posted: Posted[];
  failure?: string;
}

/** The summary screen's preview of what publishing would send. */
export interface Publication {
  plan: Plan;
  requests: GitHubRequest[];
  /** Why the draft cannot be sent; `null` when it can. */
  blocked: string | null;
  /** Posted by earlier runs; their notes are left out of `plan`. */
  published: Posted[];
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
        feedback_id: note.feedback.id,
        path: anchor.path,
        side: anchor.side,
        start_line: anchor.start_line,
        end_line: anchor.end_line,
        body: note.feedback.body.trim(),
      });
    } else if (venue.type === "pull_request") {
      loose.push(note);
    } else {
      code.push(issue(note, reviewTitle, scopeText, false));
    }
  }
  const plan: Plan = {};
  const review = pullRequest(venue, reviewTitle, comments, loose);
  const codeIssues = bundle(venue.repository, code, reviewTitle, false);
  const gewuIssues = bundle(GEWU_REPOSITORY, gewu, reviewTitle, true);
  if (review) plan.pull_request = review;
  if (codeIssues) plan.code_issues = codeIssues;
  if (gewuIssues) plan.gewu_issues = gewuIssues;
  return plan;
}

export function planIsEmpty(plan: Plan): boolean {
  return !plan.pull_request && !plan.code_issues && !plan.gewu_issues;
}

/** The request paths, in the order a publish run sends them. */
export function planRequests(plan: Plan): GitHubRequest[] {
  const requests: GitHubRequest[] = [];
  if (plan.pull_request) {
    const review = plan.pull_request;
    requests.push({
      method: "POST",
      path: `/repos/${review.repository}/pulls/${review.number}/reviews`,
      body: { event: "COMMENT", body: review.summary, comments: review.comments },
    });
  }
  for (const bundle of [plan.code_issues, plan.gewu_issues]) {
    if (!bundle) continue;
    for (const item of [...bundle.issues, ...(bundle.tracking ? [bundle.tracking] : [])]) {
      requests.push({
        method: "POST",
        path: `/repos/${bundle.repository}/issues`,
        body: { title: item.title, body: item.body },
      });
    }
  }
  return requests;
}

/** A draft built in the window, for hosts with no publication endpoint. */
export function localPublication(
  venue: Venue,
  reviewTitle: string,
  snapshot: Snapshot,
  published: Posted[] = [],
): Publication {
  const sent = new Set(published.flatMap((posted) => posted.feedback_ids));
  const plan = draftPlan(
    venue,
    reviewTitle,
    snapshot.chunks,
    snapshot.feedback.filter((item) => !sent.has(item.id)),
  );
  return {
    plan,
    requests: planRequests(plan),
    blocked: venue.repository.includes("/")
      ? null
      : `${venue.repository} has no GitHub remote, so this draft can be read but not sent.`,
    published,
  };
}
