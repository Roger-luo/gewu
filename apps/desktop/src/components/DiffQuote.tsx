import type { QuoteView } from "../protocol";
import { TokenLine } from "./TokenLine";
import { useHighlightedLines } from "./useHighlightedLines";
import { QuoteFrame } from "./QuoteFrame";

export function DiffQuote({
  quote,
  selectedLines,
  onToggleLine,
}: {
  quote: QuoteView;
  selectedLines: Set<number>;
  onToggleLine: (changeId: string, line: number) => void;
}) {
  const highlighted = useHighlightedLines(
    quote.lines.map((row) => row.text).join("\n"),
    quote.language,
  );

  return (
    <QuoteFrame quote={quote} presentationLabel="diff">
      <table className="diff-table">
        <tbody>
          {quote.lines.map((row, index) => {
            const selectable =
              row.kind !== "removed" && row.new_line != null;
            const selected =
              selectable && selectedLines.has(row.new_line as number);
            return (
              <tr key={`${quote.change_id}-${index}`} className={row.kind}>
                <td className="ln">{row.old_line ?? ""}</td>
                <td
                  className={`ln${selected ? " selected" : ""}`}
                  onClick={() => {
                    if (selectable && row.new_line != null) {
                      onToggleLine(quote.change_id, row.new_line);
                    }
                  }}
                  onKeyDown={(event) => {
                    if (
                      selectable &&
                      row.new_line != null &&
                      (event.key === "Enter" || event.key === " ")
                    ) {
                      event.preventDefault();
                      onToggleLine(quote.change_id, row.new_line);
                    }
                  }}
                  role={selectable ? "button" : undefined}
                  tabIndex={selectable ? 0 : undefined}
                  aria-pressed={selectable ? selected : undefined}
                  aria-label={
                    selectable && row.new_line != null
                      ? `Select line ${row.new_line}`
                      : undefined
                  }
                >
                  {row.new_line ?? ""}
                </td>
                <td className="marker">
                  {row.kind === "added" ? "+" : row.kind === "removed" ? "-" : " "}
                </td>
                <td>
                  <TokenLine tokens={highlighted?.[index]} plain={row.text} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </QuoteFrame>
  );
}
