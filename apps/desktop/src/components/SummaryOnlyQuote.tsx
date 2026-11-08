import type { QuoteView } from "../protocol";
import { CodeHighlight } from "./CodeHighlight";
import { QuoteFrame } from "./QuoteFrame";

export function SummaryOnlyQuote({ quote }: { quote: QuoteView }) {
  const text = quote.lines.map((line) => line.text).join("\n");

  return (
    <QuoteFrame quote={quote} presentationLabel="summary">
      {text ? (
        <CodeHighlight className="summary-only" code={text} language={quote.language} />
      ) : (
        <p className="muted">Contents summarized; not shown in full.</p>
      )}
    </QuoteFrame>
  );
}
