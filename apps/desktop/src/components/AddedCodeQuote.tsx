import type { QuoteView } from "../protocol";
import { QuoteFrame } from "./QuoteFrame";
import { TokenLine } from "./TokenLine";
import { useHighlightedLines } from "./useHighlightedLines";

/** New code, one row per line so a long line wraps beside its own number. */
export function AddedCodeQuote({
  quote,
  selectedLines,
  onToggleLine,
}: {
  quote: QuoteView;
  selectedLines: Set<number>;
  onToggleLine: (changeId: string, line: number) => void;
}) {
  const rows = quote.lines.filter((line) => line.kind !== "removed");
  const { lines: highlighted, pending } = useHighlightedLines(
    rows.map((line) => line.text).join("\n"),
    quote.language,
  );

  return (
    <QuoteFrame quote={quote} presentationLabel="added code">
      <table
        className="diff-table added-table"
        data-gewu-pending={pending ? "highlight" : undefined}
      >
        <tbody>
          {rows.map((row, index) => {
            const selected = row.new_line != null && selectedLines.has(row.new_line);
            return (
              <tr key={`${quote.change_id}-${index}`}>
                <td className="ln-cell">
                  <button
                    type="button"
                    className={`ln-btn${selected ? " selected" : ""}`}
                    disabled={row.new_line == null}
                    aria-pressed={row.new_line != null ? selected : undefined}
                    aria-label={row.new_line != null ? `Select line ${row.new_line}` : undefined}
                    onClick={() => {
                      if (row.new_line != null) {
                        onToggleLine(quote.change_id, row.new_line);
                      }
                    }}
                  >
                    {row.new_line ?? ""}
                  </button>
                </td>
                <td>
                  <code className="diff-code">
                    <TokenLine tokens={highlighted?.[index]} plain={row.text} />
                  </code>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </QuoteFrame>
  );
}
