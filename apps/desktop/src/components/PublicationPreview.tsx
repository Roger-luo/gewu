import { useState, type ReactNode } from "react";

import { anchorLabel } from "../anchors";
import { InlineMarkdown, Markdown } from "./Markdown";
import type { GitHubAccount } from "../backend/types";
import type { PublicationState } from "../hooks/usePublication";
import {
  GEWU_REPOSITORY,
  planIsEmpty,
  type IssueBundle,
  type IssueDraft,
  type Receipt,
} from "../publication";
import { ConfirmPublish, PublishBar, ReceiptView } from "./PublishControls";

/**
 * The comments and issues a review would post. Rendering it sends nothing;
 * Publish sends only after the reviewer confirms the list of requests.
 */
export function PublicationPreview({
  state,
  account,
  rehearsing = false,
}: {
  state: PublicationState;
  account: GitHubAccount | null;
  /** Settings say to rehearse: Publish records and sends nothing. */
  rehearsing?: boolean;
}) {
  const { publication, error } = state;
  const [confirming, setConfirming] = useState(false);
  const [receipt, setReceipt] = useState<Receipt | null>(null);
  const plan = publication?.plan;
  return (
    <section className="panel publication" aria-label="Publication preview">
      <h3>Publication preview</h3>
      <p className="muted">
        {publication?.blocked ? (
          <>
            This is the text Gewu would post. It cannot be published or rehearsed from here:{" "}
            <InlineMarkdown source={publication.blocked} />
          </>
        ) : !state.publish
          ? "This is the text Gewu would post. Reading it does not call GitHub; publishing is done from the workbench."
          : rehearsing
            ? "This is the text Gewu would post. Rehearsal is on in Settings, so Publish records it and sends nothing."
            : "This is the text Gewu would post. Nothing is sent until you press Publish and confirm, so reading it does not call GitHub."}
      </p>
      {error && (
        <p className="publication-failure" role="alert">
          {error}
        </p>
      )}
      {receipt && <ReceiptView receipt={receipt} />}
      {publication && publication.published.length > 0 && (
        <div className="publication-destination">
          <h4 className="publication-destination-title">Already on GitHub</h4>
          <ul className="publication-posted">
            {publication.published.map((posted) => (
              <li key={posted.url || posted.label}>
                <a href={posted.url} target="_blank" rel="noreferrer">
                  {posted.label}
                </a>
              </li>
            ))}
          </ul>
        </div>
      )}
      {!plan ? (
        !error && <p className="muted">Drafting…</p>
      ) : planIsEmpty(plan) ? (
        <p className="muted">No open notes to publish.</p>
      ) : (
        <>
          {plan.pull_request && (
            <Destination
              heading={`Pull request #${plan.pull_request.number} on ${plan.pull_request.repository}`}
              blocked={publication.blocked}
              request={`POST /repos/${plan.pull_request.repository}/pulls/${plan.pull_request.number}/reviews`}
            >
              {plan.pull_request.comments.map((comment) => (
                <article key={comment.feedback_id} className="publication-item">
                  <h4>{anchorLabel(comment)}</h4>
                  <Markdown source={comment.body} />
                </article>
              ))}
              {plan.pull_request.summary && (
                <article className="publication-item">
                  <h4>Review comment</h4>
                  <Markdown source={plan.pull_request.summary} />
                </article>
              )}
            </Destination>
          )}
          {plan.code_issues && <Issues bundle={plan.code_issues} blocked={publication.blocked} />}
          {plan.gewu_issues && <Issues bundle={plan.gewu_issues} blocked={null} />}
          {state.publish && (
            <PublishBar
              blocked={publication.blocked}
              account={account}
              count={publication.requests.length}
              rehearsing={rehearsing}
              onPublish={() => setConfirming(true)}
            />
          )}
        </>
      )}
      {confirming && publication && state.publish && account && (
        <ConfirmPublish
          account={account}
          paths={publication.requests.map((request) => `${request.method} ${request.path}`)}
          rehearsing={rehearsing}
          onCancel={() => setConfirming(false)}
          onConfirm={async () => {
            const send = state.publish;
            if (!send) return;
            try {
              setReceipt(await send());
            } catch (err) {
              setReceipt({ posted: [], failure: err instanceof Error ? err.message : String(err) });
            }
            setConfirming(false);
          }}
        />
      )}
    </section>
  );
}

function Issues({ bundle, blocked }: { bundle: IssueBundle; blocked: string | null }) {
  const gewu = bundle.repository === GEWU_REPOSITORY;
  return (
    <Destination
      heading={
        gewu
          ? `Issues on ${bundle.repository} (public; only what you wrote about the walkthrough)`
          : `Issues on ${bundle.repository}`
      }
      blocked={gewu ? null : blocked}
      request={`POST /repos/${bundle.repository}/issues`}
    >
      {bundle.issues.map((issue) => (
        <IssueCard key={issue.feedback_id ?? issue.title} issue={issue} />
      ))}
      {bundle.tracking && <IssueCard issue={bundle.tracking} />}
    </Destination>
  );
}

function IssueCard({ issue }: { issue: IssueDraft }) {
  return (
    // GitHub renders Markdown in issue titles and bodies; show it the same way.
    <article className="publication-item">
      <h4>
        <InlineMarkdown source={issue.title} />
      </h4>
      <Markdown source={issue.body} />
    </article>
  );
}

function Destination({
  heading,
  request,
  blocked,
  children,
}: {
  heading: string;
  request: string;
  /** Why this destination cannot be reached; its address is then unknown. */
  blocked: string | null;
  children: ReactNode;
}) {
  return (
    <div className="publication-destination">
      <h4 className="publication-destination-title">{heading}</h4>
      <p className="muted publication-request">
        {blocked ? "Not sent · no GitHub repository to send it to" : `Not sent · ${request}`}
      </p>
      {children}
    </div>
  );
}
