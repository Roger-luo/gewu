import type { BlockView, QuoteView } from "../protocol";
import { AddedCodeQuote } from "./AddedCodeQuote";
import { DiffQuote } from "./DiffQuote";
import { MarkdownQuote } from "./MarkdownQuote";
import { SummaryOnlyQuote } from "./SummaryOnlyQuote";

export function ChangesBlock({
  block,
  selected,
  onToggleLine,
}: {
  block: Extract<BlockView, { type: "changes" }>;
  selected: { changeId: string; lines: Set<number> } | null;
  onToggleLine: (changeId: string, line: number) => void;
}) {
  return (
    <div className="block changes-block">
      {block.quotes.map((quote) => (
        <QuotePresentation
          key={`${quote.change_id}-${quote.start}-${quote.end}`}
          quote={quote}
          presentation={block.presentation}
          selectedLines={
            selected?.changeId === quote.change_id
              ? selected.lines
              : new Set()
          }
          onToggleLine={onToggleLine}
        />
      ))}
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
