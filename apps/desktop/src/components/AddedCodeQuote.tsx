import type { QuoteView } from "../protocol";
import { CodeHighlight } from "./CodeHighlight";
import { QuoteFrame } from "./QuoteFrame";

export function AddedCodeQuote({
  quote,
  selectedLines,
  onToggleLine,
}: {
  quote: QuoteView;
  selectedLines: Set<number>;
  onToggleLine: (changeId: string, line: number) => void;
}) {
  const lines = quote.lines.filter((line) => line.kind !== "removed");
  const code = lines.map((line) => line.text).join("\n");

  return (
    <QuoteFrame quote={quote} presentationLabel="added code">
      <div className="added-code-layout">
        <div className="added-gutter" aria-hidden={false}>
          {lines.map((row, index) => {
            const selectable = row.new_line != null;
            const selected =
              selectable && selectedLines.has(row.new_line as number);
            return (
              <button
                key={`${quote.change_id}-ln-${index}`}
                type="button"
                className={`ln-btn${selected ? " selected" : ""}`}
                disabled={!selectable}
                aria-pressed={selectable ? selected : undefined}
                aria-label={
                  row.new_line != null
                    ? `Select line ${row.new_line}`
                    : undefined
                }
                onClick={() => {
                  if (row.new_line != null) {
                    onToggleLine(quote.change_id, row.new_line);
                  }
                }}
              >
                {row.new_line ?? ""}
              </button>
            );
          })}
        </div>
        <CodeHighlight
          className="added-code"
          code={code}
          language={quote.language}
        />
      </div>
    </QuoteFrame>
  );
}
