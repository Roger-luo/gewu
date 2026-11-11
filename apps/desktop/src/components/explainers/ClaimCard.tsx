import type { Explainer } from "../../explainer";
import { InlineMarkdown, Markdown } from "../Markdown";

/** What the change claims, quoted from its author, and how this step serves it. */
export function ClaimCard({ explainer }: { explainer: Extract<Explainer, { kind: "claim" }> }) {
  return (
    <figure className="explainer claim-card" aria-label="Claim this step serves">
      <figcaption className="explainer-label">Serves the claim</figcaption>
      <blockquote className="claim-quote">
        <InlineMarkdown source={explainer.claim} />
      </blockquote>
      {explainer.result && (
        <dl className="claim-figures">
          <dt>Whole change, measured</dt>
          <dd>{explainer.result}</dd>
          {explainer.share && (
            <>
              <dt>This step&apos;s part</dt>
              <dd className="claim-share">{explainer.share}</dd>
            </>
          )}
        </dl>
      )}
      <div className="claim-how">
        <Markdown source={explainer.how} />
      </div>
    </figure>
  );
}
