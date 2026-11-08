import type { ReactNode } from "react";

import type { QuoteView } from "../protocol";

export function QuoteFrame({
  quote,
  presentationLabel,
  children,
}: {
  quote: QuoteView;
  presentationLabel: string;
  children: ReactNode;
}) {
  return (
    <figure className="quote-frame">
      <figcaption className="quote-gutter">
        <span className="path">{quote.path}</span>
        <span className="presentation">{presentationLabel}</span>
      </figcaption>
      {quote.more_above && (
        <div className="cue">⋯ Hunk continues above; not part of this step</div>
      )}
      {children}
      {quote.more_below && (
        <div className="cue below">⋯ Hunk continues below; not part of this step</div>
      )}
    </figure>
  );
}
