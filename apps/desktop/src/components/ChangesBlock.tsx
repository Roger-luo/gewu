import { quoteSpan, type CommentRequest } from "../anchors";
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
  onToggleLine,
  onComment,
}: {
  block: Extract<BlockView, { type: "changes" }>;
  /** Prose for each quote, empty when the walkthrough named no lines in it. */
  notes: string[];
  selected: { changeId: string; lines: Set<number> } | null;
  onToggleLine: (changeId: string, line: number) => void;
  onComment: (request: CommentRequest) => void;
}) {
  return (
    <div className="block changes-block">
      {block.quotes.map((quote, index) => {
        const note = notes[index] ?? "";
        const span = quoteSpan(quote);
        return (
          <div
            className={note ? "quote-row has-note" : "quote-row"}
            key={`${quote.change_id}-${quote.start}-${quote.end}`}
          >
            <div className="quote-row-code">
              {span && <CommentActions target="code" span={span} onComment={onComment} />}
              <QuotePresentation
                quote={quote}
                presentation={block.presentation}
                selectedLines={
                  selected?.changeId === quote.change_id ? selected.lines : new Set()
                }
                onToggleLine={onToggleLine}
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
  onToggleLine,
}: {
  quote: QuoteView;
  presentation: Extract<BlockView, { type: "changes" }>["presentation"];
  selectedLines: Set<number>;
  onToggleLine: (changeId: string, line: number) => void;
}) {
  switch (presentation) {
    case "diff":
      return (
        <DiffQuote
          quote={quote}
          selectedLines={selectedLines}
          onToggleLine={onToggleLine}
        />
      );
    case "added_code":
      return (
        <AddedCodeQuote
          quote={quote}
          selectedLines={selectedLines}
          onToggleLine={onToggleLine}
        />
      );
    case "rendered_markdown":
      return <MarkdownQuote quote={quote} />;
    case "summary_only":
      return <SummaryOnlyQuote quote={quote} />;
  }
}
