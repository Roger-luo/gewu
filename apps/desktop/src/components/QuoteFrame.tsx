import type { ReactNode } from "react";

import type { QuoteView } from "../protocol";
import { QuotePreview } from "./QuotePreview";

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
      <div className={quote.preview ? "quote-body with-preview" : "quote-body"}>
        <div className="quote-source">{children}</div>
        {quote.preview && <QuotePreview preview={quote.preview} />}
      </div>
      {quote.more_below && (
        <div className="cue below">⋯ Hunk continues below; not part of this step</div>
      )}
    </figure>
  );
}
