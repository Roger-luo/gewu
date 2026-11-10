import type {
  Action,
  Decision,
  Feedback,
  FeedbackAnchorInput,
  LineAnchor,
  Snapshot,
} from "../protocol";
import {
  localPublication,
  planRequests,
  type Posted,
  type Publication,
  type Receipt,
  type Venue,
} from "../publication";
import { mockGitHub, mockSignedIn } from "./mockGitHub";
import { BackendError, type Backend } from "./types";

function clone<T>(value: T): T {
  return structuredClone(value);
}

function rebuildSummary(snapshot: Snapshot): void {
  const unreviewed = snapshot.chunks
    .filter((chunk) => !chunk.status.reviewed)
    .map((chunk) => chunk.id);
  const unresolved = snapshot.feedback
    .filter((item) => item.kind === "issue" && !item.resolved)
    .map((item) => item.id);

  let suggested: Decision = "approve";
  if (unresolved.length > 0) {
    suggested = "request_changes";
  } else if (snapshot.feedback.length > 0) {
    suggested = "comment";
  }

  snapshot.summary = {
    reviewed: snapshot.chunks.length - unreviewed.length,
    total: snapshot.chunks.length,
    unreviewed,
    unresolved_issues: unresolved,
    decision: snapshot.summary.decision,
    suggested_decision: suggested,
    batch_scoped: !snapshot.generation.complete,
  };

  for (const chunk of snapshot.chunks) {
    const items = snapshot.feedback.filter((item) => item.chunk_id === chunk.id);
    chunk.status.feedback_count = items.length;
    chunk.status.open_issues = items.filter(
      (item) => item.kind === "issue" && !item.resolved,
    ).length;
  }
}

function resolveAnchor(
  snapshot: Snapshot,
  chunkId: string,
  anchor: FeedbackAnchorInput,
): LineAnchor | null {
  if (!anchor) {
    return null;
  }
  const chunk = snapshot.chunks.find((item) => item.id === chunkId);
  if (!chunk) {
    throw new BackendError(`Unknown chunk: ${chunkId}`);
  }
  for (const block of chunk.blocks) {
    if (block.type !== "changes") {
      continue;
    }
    const quote = block.quotes.find((item) => item.change_id === anchor.change_id);
    if (quote) {
      return {
        change_id: anchor.change_id,
        path: quote.path,
        side: anchor.side ?? "new",
        start_line: anchor.start_line,
        end_line: anchor.end_line,
        patch_sha256: "mock-patch",
      };
    }
  }
  throw new BackendError(`Unknown change: ${anchor.change_id}`);
}

/** Where the mock's notes would be published, and under what title. */
export interface MockVenue {
  venue: Venue;
  title: string;
}

export class MockBackend implements Backend {
  private state: Snapshot;
  private nextFeedback = 1;
  private target: MockVenue;
  private posted: Posted[] = [];
  private nextNumber = 1;

  readonly github = mockGitHub;

  constructor(initial: Snapshot, target?: MockVenue) {
    this.state = clone(initial);
    this.target = target ?? {
      venue: {
        type: "commits",
        repository: "acme/demo",
        base: initial.source.base,
        head: initial.source.head,
      },
      title: "This review",
    };
    rebuildSummary(this.state);
  }

  async publication(): Promise<Publication> {
    return localPublication(this.target.venue, this.target.title, this.state, clone(this.posted));
  }

  /** Answers like GitHub would, without sending anything. */
  async publish(): Promise<Receipt> {
    if (!mockSignedIn()) {
      return { posted: [], failure: "Nothing was sent: sign in to GitHub first." };
    }
    const publication = await this.publication();
    if (publication.blocked) {
      return { posted: [], failure: publication.blocked };
    }
    const { plan } = publication;
    const posted: Posted[] = [];
    if (plan.pull_request) {
      const review = plan.pull_request;
      posted.push({
        label: `Review on pull request #${review.number}`,
        url: `https://github.com/${review.repository}/pull/${review.number}#pullrequestreview-${this.nextNumber++}`,
        feedback_ids: [...review.comments.map((comment) => comment.feedback_id), ...(review.summarized ?? [])],
      });
    }
    for (const bundle of [plan.code_issues, plan.gewu_issues]) {
      if (!bundle) continue;
      for (const issue of bundle.issues) {
        const number = this.nextNumber++;
        posted.push({
          label: `Issue #${number} on ${bundle.repository}: ${issue.title}`,
          url: `https://github.com/${bundle.repository}/issues/${number}`,
          feedback_ids: issue.feedback_id ? [issue.feedback_id] : [],
        });
      }
      if (bundle.tracking) {
        const number = this.nextNumber++;
        posted.push({
          label: `Tracking issue #${number} on ${bundle.repository}`,
          url: `https://github.com/${bundle.repository}/issues/${number}`,
          feedback_ids: [],
        });
      }
    }
    this.requestsSent += planRequests(plan).length;
    this.posted.push(...posted);
    return { posted };
  }

  /** How many requests the mock pretended to send, for tests. */
  requestsSent = 0;

  async snapshot(): Promise<Snapshot> {
    return clone(this.state);
  }

  /** Add the chunks of a newly generated batch, as an agent would. */
  append(chunks: Snapshot["chunks"], generation: Snapshot["generation"]): void {
    this.state.chunks.push(...clone(chunks));
    this.state.generation = { ...generation };
    rebuildSummary(this.state);
  }

  async dispatch(action: Action): Promise<Snapshot> {
    switch (action.type) {
      case "visit": {
        const chunk = this.requireChunk(action.chunk_id);
        chunk.status.visited = true;
        this.state.last_visited = chunk.id;
        break;
      }
      case "set_reviewed": {
        const chunk = this.requireChunk(action.chunk_id);
        chunk.status.reviewed = action.reviewed;
        if (action.reviewed) {
          chunk.status.visited = true;
        }
        break;
      }
      case "add_feedback": {
        this.requireChunk(action.chunk_id);
        const body = action.body.trim();
        if (!body) {
          throw new BackendError("Feedback body cannot be empty");
        }
        const feedback: Feedback = {
          id: `fb-${this.nextFeedback++}`,
          chunk_id: action.chunk_id,
          kind: action.kind,
          target: action.target,
          body,
          anchor: resolveAnchor(this.state, action.chunk_id, action.anchor),
          resolved: false,
        };
        this.state.feedback.push(feedback);
        break;
      }
      case "resolve_feedback": {
        const feedback = this.requireFeedback(action.feedback_id);
        feedback.resolved = action.resolved;
        break;
      }
      case "delete_feedback": {
        const index = this.state.feedback.findIndex(
          (item) => item.id === action.feedback_id,
        );
        if (index < 0) {
          throw new BackendError(`Unknown feedback: ${action.feedback_id}`);
        }
        this.state.feedback.splice(index, 1);
        break;
      }
      case "retarget_feedback": {
        const feedback = this.requireFeedback(action.feedback_id);
        feedback.target = action.target;
        break;
      }
      case "set_decision": {
        this.state.summary.decision = action.decision;
        break;
      }
      default: {
        const exhaustive: never = action;
        throw new BackendError(`Unsupported action: ${JSON.stringify(exhaustive)}`);
      }
    }
    rebuildSummary(this.state);
    return clone(this.state);
  }

  private requireChunk(chunkId: string) {
    const chunk = this.state.chunks.find((item) => item.id === chunkId);
    if (!chunk) {
      throw new BackendError(`Unknown chunk: ${chunkId}`);
    }
    return chunk;
  }

  private requireFeedback(feedbackId: string) {
    const feedback = this.state.feedback.find((item) => item.id === feedbackId);
    if (!feedback) {
      throw new BackendError(`Unknown feedback: ${feedbackId}`);
    }
    return feedback;
  }
}
