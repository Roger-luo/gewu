import {
  lineSpan,
  narrowSpan,
  quoteSpan,
  type CommentRequest,
  type LineSelection,
} from "../anchors";
import type { BlockView, QuoteView } from "../protocol";
import { AddedCodeQuote } from "./AddedCodeQuote";
import { CommentActions } from "./CommentActions";
import { DiffQuote } from "./DiffQuote";
import { Markdown } from "./Markdown";
import { MarkdownQuote } from "./MarkdownQuote";
import { SummaryOnlyQuote } from "./SummaryOnlyQuote";

export function ChangesBlock({
  block,
  notes,
  selected,
  onSelectLine,
  onComment,
  onCommentLines,
}: {
  block: Extract<BlockView, { type: "changes" }>;
  /** Prose for each quote, empty when the walkthrough named no lines in it. */
  notes: string[];
  selected: LineSelection | null;
  onSelectLine: (changeId: string, line: number, extend: boolean) => void;
  onComment: (request: CommentRequest) => void;
  onCommentLines: (request: CommentRequest) => void;
}) {
  return (
    <div className="block changes-block">
      {block.quotes.map((quote, index) => {
        const note = notes[index] ?? "";
        const span = quoteSpan(quote);
        const lines = selected?.changeId === quote.change_id ? selected.lines : NONE;
        // The block buttons say which lines they land on, so a comment on a
        // whole block is a choice, not a surprise on GitHub.
        const covered = span && narrowSpan(span, lines);
        const commentLine =
          span && span.side === "new"
            ? (line: number) =>
                onCommentLines({ kind: "note", target: "code", span: lineSpan(span, lines, line) })
            : undefined;
        return (
          <div
            className={note ? "quote-row has-note" : "quote-row"}
            key={`${quote.change_id}-${quote.start}-${quote.end}`}
          >
            <div className="quote-row-code">
              {covered && (
                <CommentActions target="code" span={covered} onComment={onComment} />
              )}
              <QuotePresentation
                quote={quote}
                presentation={block.presentation}
                selectedLines={lines}
                onSelectLine={onSelectLine}
                onCommentLine={commentLine}
              />
            </div>
            {note && (
              <aside className="quote-note">
                <Markdown source={note} />
                {span && (
                  <CommentActions target="explanation" span={span} onComment={onComment} />
                )}
              </aside>
            )}
          </div>
        );
      })}
    </div>
  );
}

function QuotePresentation({
  quote,
  presentation,
  selectedLines,
  onSelectLine,
  onCommentLine,
}: {
  quote: QuoteView;
  presentation: Extract<BlockView, { type: "changes" }>["presentation"];
  selectedLines: Set<number>;
  onSelectLine: (changeId: string, line: number, extend: boolean) => void;
  onCommentLine?: (line: number) => void;
}) {
  switch (presentation) {
    case "diff":
      return (
        <DiffQuote
          quote={quote}
          selectedLines={selectedLines}
          onSelectLine={onSelectLine}
          onCommentLine={onCommentLine}
        />
      );
    case "added_code":
      return (
        <AddedCodeQuote
          quote={quote}
          selectedLines={selectedLines}
          onSelectLine={onSelectLine}
          onCommentLine={onCommentLine}
        />
      );
    case "rendered_markdown":
      return <MarkdownQuote quote={quote} />;
    case "summary_only":
      return <SummaryOnlyQuote quote={quote} />;
  }
}

const NONE: Set<number> = new Set();
