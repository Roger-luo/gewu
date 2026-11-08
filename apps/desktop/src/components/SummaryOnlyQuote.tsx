import type { QuoteView } from "../protocol";
import { QuoteFrame } from "./QuoteFrame";

export function SummaryOnlyQuote({ quote }: { quote: QuoteView }) {
  const text =
    quote.lines.map((line) => line.text).join("\n") ||
    "Contents summarized; not shown in full.";

  return (
    <QuoteFrame quote={quote} presentationLabel="summary">
      <div className="summary-only">{text}</div>
    </QuoteFrame>
  );
}
