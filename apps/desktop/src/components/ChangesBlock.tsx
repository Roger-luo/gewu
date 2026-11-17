import {
  lineSpan,
  narrowSpan,
  quoteSpan,
  type CommentRequest,
  type LineSelection,
} from "../anchors";
import type { Composer, InlineRow } from "../composer";
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
  composer = null,
}: {
  block: Extract<BlockView, { type: "changes" }>;
  /** Prose for each quote, empty when the walkthrough named no lines in it. */
  notes: string[];
  selected: LineSelection | null;
  onSelectLine: (changeId: string, line: number, extend: boolean) => void;
  onComment: (request: CommentRequest) => void;
  onCommentLines: (request: CommentRequest) => void;
  /** The open comment box when it belongs to quote `quote` of this block. */
  composer?: (Composer & { quote: number }) | null;
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
        const here = composer?.quote === index ? composer : null;
        const underNote = here?.place.at === "note" && note ? here.node : null;
        const inline: InlineRow | undefined =
          here && here.place.at === "line" && LINED.has(block.presentation)
            ? { side: here.place.side, line: here.place.line, node: here.node }
            : undefined;
        const underCode = here && !underNote && !inline ? here.node : null;
        return (
          <div
            className={note ? "quote-row has-note" : "quote-row"}
            key={`${quote.change_id}-${quote.start}-${quote.end}`}
          >
            {/* The note comes first so that, floated into the side column,
                it starts level with its quote. */}
            {note && (
              <aside className="quote-note">
                <Markdown source={note} />
                {span && (
                  <CommentActions target="explanation" span={span} onComment={onComment} />
                )}
                {underNote}
              </aside>
            )}
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
                inline={inline}
              />
              {underCode && <div className="quote-composer">{underCode}</div>}
            </div>
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
  inline,
}: {
  quote: QuoteView;
  presentation: Extract<BlockView, { type: "changes" }>["presentation"];
  selectedLines: Set<number>;
  onSelectLine: (changeId: string, line: number, extend: boolean) => void;
  onCommentLine?: (line: number) => void;
  inline?: InlineRow;
}) {
  switch (presentation) {
    case "diff":
      return (
        <DiffQuote
          quote={quote}
          selectedLines={selectedLines}
          onSelectLine={onSelectLine}
          onCommentLine={onCommentLine}
          inline={inline}
        />
      );
    case "added_code":
      return (
        <AddedCodeQuote
          quote={quote}
          selectedLines={selectedLines}
          onSelectLine={onSelectLine}
          onCommentLine={onCommentLine}
          inline={inline}
        />
      );
    case "rendered_markdown":
      return <MarkdownQuote quote={quote} />;
    case "summary_only":
      return <SummaryOnlyQuote quote={quote} />;
  }
}

const NONE: Set<number> = new Set();

/** Presentations that draw one row per line, so a comment box can sit under one. */
const LINED = new Set(["diff", "added_code"]);
