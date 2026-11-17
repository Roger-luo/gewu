import type { QuoteView } from "../protocol";
import { LineComment } from "./LineComment";
import { QuoteFrame } from "./QuoteFrame";
import { TokenLine } from "./TokenLine";
import { useHighlightedLines } from "./useHighlightedLines";

/** New code, one row per line so a long line wraps beside its own number. */
export function AddedCodeQuote({
  quote,
  selectedLines,
  onSelectLine,
  onCommentLine,
}: {
  quote: QuoteView;
  selectedLines: Set<number>;
  onSelectLine: (changeId: string, line: number, extend: boolean) => void;
  onCommentLine?: (line: number) => void;
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
              <tr key={`${quote.change_id}-${index}`} className={selected ? "selected" : undefined}>
                <td className="ln-cell">
                  <button
                    type="button"
                    className={`ln-btn${selected ? " selected" : ""}`}
                    disabled={row.new_line == null}
                    aria-pressed={row.new_line != null ? selected : undefined}
                    aria-label={row.new_line != null ? `Select line ${row.new_line}` : undefined}
                    onClick={(event) => {
                      if (row.new_line != null) {
                        onSelectLine(quote.change_id, row.new_line, event.shiftKey);
                      }
                    }}
                  >
                    {row.new_line ?? ""}
                  </button>
                </td>
                <td className="code-cell">
                  {row.new_line != null && onCommentLine && (
                    <LineComment line={row.new_line} onComment={onCommentLine} />
                  )}
                  <code className="diff-code">
                    <TokenLine tokens={highlighted?.[index]} plain={row.text} language={quote.language} />
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
