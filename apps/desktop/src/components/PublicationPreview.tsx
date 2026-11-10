import type { ReactNode } from "react";

import { anchorLabel } from "../anchors";
import {
  draftPlan,
  planIsEmpty,
  type IssueBundle,
  type IssueDraft,
  type Venue,
} from "../publication";
import type { Feedback } from "../protocol";

/** The comments and issues a review would post. Rendering it sends nothing. */
export function PublicationPreview({
  venue,
  reviewTitle,
  chunks,
  feedback,
}: {
  venue: Venue;
  reviewTitle: string;
  chunks: { id: string; title: string }[];
  feedback: Feedback[];
}) {
  const plan = draftPlan(venue, reviewTitle, chunks, feedback);
  return (
    <section className="panel publication" aria-label="Publication preview">
      <h3>Publication preview</h3>
      <p className="muted">
        This is the text Gewu would post. Nothing on this page is sent, so reading
        it does not call GitHub.
      </p>
      {planIsEmpty(plan) ? (
        <p className="muted">No open notes to publish.</p>
      ) : (
        <>
          {plan.pullRequest && (
            <Destination
              heading={`Pull request #${plan.pullRequest.number} on ${plan.pullRequest.repository}`}
              request={`POST /repos/{owner}/${plan.pullRequest.repository}/pulls/${plan.pullRequest.number}/reviews`}
            >
              {plan.pullRequest.comments.map((comment) => (
                <article key={comment.feedbackId} className="publication-item">
                  <h4>
                    {anchorLabel({
                      path: comment.path,
                      side: comment.side,
                      start_line: comment.startLine,
                      end_line: comment.endLine,
                    })}
                  </h4>
                  <p>{comment.body}</p>
                </article>
              ))}
              {plan.pullRequest.summary && (
                <article className="publication-item">
                  <h4>Review comment</h4>
                  <p>{plan.pullRequest.summary}</p>
                </article>
              )}
            </Destination>
          )}
          {plan.codeIssues && (
            <Issues
              heading={`Issues on ${plan.codeIssues.repository}`}
              bundle={plan.codeIssues}
            />
          )}
          {plan.gewuIssues && (
            <Issues heading={`Issues on ${plan.gewuIssues.repository}`} bundle={plan.gewuIssues} />
          )}
        </>
      )}
    </section>
  );
}

function Issues({ heading, bundle }: { heading: string; bundle: IssueBundle }) {
  return (
    <Destination heading={heading} request={`POST /repos/{owner}/${bundle.repository}/issues`}>
      {bundle.issues.map((issue) => (
        <IssueCard key={issue.feedbackId ?? issue.title} issue={issue} />
      ))}
      {bundle.tracking && <IssueCard issue={bundle.tracking} />}
    </Destination>
  );
}

function IssueCard({ issue }: { issue: IssueDraft }) {
  return (
    <article className="publication-item">
      <h4>{issue.title}</h4>
      <p>{issue.body}</p>
    </article>
  );
}

function Destination({
  heading,
  request,
  children,
}: {
  heading: string;
  request: string;
  children: ReactNode;
}) {
  return (
    <div className="publication-destination">
      <h4 className="publication-destination-title">{heading}</h4>
      <p className="muted publication-request">Not sent · {request}</p>
      {children}
    </div>
  );
}
