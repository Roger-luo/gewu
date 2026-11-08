import { useState } from "react";

import { Markdown } from "./Markdown";
import type { QuoteView } from "../protocol";
import { QuoteFrame } from "./QuoteFrame";

export function MarkdownQuote({ quote }: { quote: QuoteView }) {
  const [raw, setRaw] = useState(false);
  const source = quote.lines
    .filter((line) => line.kind !== "removed")
    .map((line) => line.text)
    .join("\n");

  return (
    <QuoteFrame quote={quote} presentationLabel="document">
      <div className="markdown-doc">
        <div className="actions-row" style={{ padding: "0.5rem 0.75rem" }}>
          <button
            type="button"
            className="btn ghost"
            aria-pressed={raw}
            onClick={() => setRaw((value) => !value)}
          >
            {raw ? "Show rendered" : "Show raw source"}
          </button>
        </div>
        {raw ? (
          <pre className="code-plain" style={{ padding: "0 0.75rem 0.75rem" }}>
            {source}
          </pre>
        ) : (
          <div style={{ padding: "0 0.75rem 0.75rem" }}>
            <Markdown source={source} />
          </div>
        )}
      </div>
    </QuoteFrame>
  );
}
