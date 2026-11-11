import type { QuoteView } from "../protocol";
import { QuoteFrame } from "./QuoteFrame";

const MARK = { added: "+", removed: "−", context: " " } as const;

/**
 * A summarized artifact (a lockfile, generated output): its lines as a small
 * diff, numbered and marked added or removed, so the change still reads as a
 * change. Found by the final UI judge: it used to run old and new lines
 * together with no gutter.
 */
export function SummaryOnlyQuote({ quote }: { quote: QuoteView }) {
  if (quote.lines.length === 0) {
    return (
      <QuoteFrame quote={quote} presentationLabel="summary">
        <p className="muted summary-empty">Contents summarized; not shown in full.</p>
      </QuoteFrame>
    );
  }
  return (
    <QuoteFrame quote={quote} presentationLabel="summary">
      <table className="summary-lines">
        <tbody>
          {quote.lines.map((line, index) => {
            const kind = line.kind === "added" || line.kind === "removed" ? line.kind : "context";
            return (
              <tr key={index} className={`summary-line ${kind}`}>
                <td className="summary-number" aria-hidden>
                  {line.new_line ?? line.old_line ?? ""}
                </td>
                <td className="summary-mark">
                  <span aria-hidden>{MARK[kind]}</span>
                  <span className="sr-only">{kind === "context" ? "" : kind}</span>
                </td>
                <td className="summary-text">{line.text}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </QuoteFrame>
  );
}
