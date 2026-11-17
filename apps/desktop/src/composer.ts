import type { ReactNode } from "react";

import { quoteSpan } from "./anchors";
import type { BlockView, QuoteLine } from "./protocol";

/**
 * Where an open comment box sits in the chunk, so it opens beside what it
 * is about instead of covering it: under a code line, under a quote's note,
 * or at the end of the chunk when it is about the whole chunk.
 */
export type ComposerPlace =
  | { at: "line"; changeId: string; side: "new" | "old"; line: number }
  | { at: "note"; changeId: string; side: "new" | "old"; line: number }
  | { at: "chunk" };

/** The comment box and where it goes. */
export interface Composer {
  place: ComposerPlace;
  node: ReactNode;
}

/** The first quote, as `[block, quote]` indexes, that holds the place's line. */
export function locate(blocks: BlockView[], place: ComposerPlace): [number, number] | null {
  if (place.at === "chunk") {
    return null;
  }
  for (const [blockIndex, block] of blocks.entries()) {
    if (block.type !== "changes") {
      continue;
    }
    for (const [quoteIndex, quote] of block.quotes.entries()) {
      const span = quoteSpan(quote);
      if (
        span &&
        span.change_id === place.changeId &&
        span.side === place.side &&
        place.line >= span.start_line &&
        place.line <= span.end_line
      ) {
        return [blockIndex, quoteIndex];
      }
    }
  }
  return null;
}

/** Content drawn as its own table row under one line of a quote. */
export interface InlineRow {
  side: "new" | "old";
  line: number;
  node: ReactNode;
}

/** Whether `inline` goes under `row`. */
export function belongsUnder(inline: InlineRow | undefined, row: QuoteLine): boolean {
  if (!inline) {
    return false;
  }
  const number = inline.side === "new" ? row.new_line : row.old_line;
  return number === inline.line;
}
