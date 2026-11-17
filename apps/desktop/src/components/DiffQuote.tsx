import type { QuoteView } from "../protocol";
import { TokenLine } from "./TokenLine";
import { useHighlightedLines } from "./useHighlightedLines";
import { Fragment } from "react";

import { belongsUnder, type InlineRow } from "../composer";
import { InlineRowView } from "./InlineRow";
import { LineComment } from "./LineComment";
import { QuoteFrame } from "./QuoteFrame";

export function DiffQuote({
  quote,
  selectedLines,
  onSelectLine,
  onCommentLine,
  inline,
}: {
  quote: QuoteView;
  selectedLines: Set<number>;
  onSelectLine: (changeId: string, line: number, extend: boolean) => void;
  onCommentLine?: (line: number) => void;
  /** A comment box drawn under one line. */
  inline?: InlineRow;
}) {
  const { lines: highlighted, pending } = useHighlightedLines(
    quote.lines.map((row) => row.text).join("\n"),
    quote.language,
  );

  return (
    <QuoteFrame quote={quote} presentationLabel="diff">
      <table className="diff-table" data-gewu-pending={pending ? "highlight" : undefined}>
        <tbody>
          {quote.lines.map((row, index) => {
            const selectable =
              row.kind !== "removed" && row.new_line != null;
            const selected =
              selectable && selectedLines.has(row.new_line as number);
            return (
              <Fragment key={`${quote.change_id}-${index}`}>
              <tr className={selected ? `${row.kind} selected` : row.kind}>
                <td className="ln">{row.old_line ?? ""}</td>
                <td
                  className={`ln${selected ? " selected" : ""}`}
                  onClick={(event) => {
                    if (selectable && row.new_line != null) {
                      onSelectLine(quote.change_id, row.new_line, event.shiftKey);
                    }
                  }}
                  onKeyDown={(event) => {
                    if (
                      selectable &&
                      row.new_line != null &&
                      (event.key === "Enter" || event.key === " ")
                    ) {
                      event.preventDefault();
                      onSelectLine(quote.change_id, row.new_line, event.shiftKey);
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
                <td className="code-cell">
                  {selectable && row.new_line != null && onCommentLine && (
                    <LineComment line={row.new_line} onComment={onCommentLine} />
                  )}
                  <code className="diff-code">
                    <TokenLine tokens={highlighted?.[index]} plain={row.text} language={quote.language} />
                  </code>
                </td>
              </tr>
              {belongsUnder(inline, row) && inline && (
                <InlineRowView inline={inline} columns={4} />
              )}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </QuoteFrame>
  );
}
