import type { BlockView } from "./protocol";
import { assignNotes } from "./quoteNotes";

/** One piece of a chunk's body, in reading order. */
export type BodyPiece =
  | { type: "lead"; key: string; markdown: string }
  | { type: "explainer"; key: string; index: number; block: Extract<BlockView, { type: "explainer" }> }
  | {
      type: "changes";
      key: string;
      index: number;
      block: Extract<BlockView, { type: "changes" }>;
      /** Prose for each quote, empty when the walkthrough named no lines in it. */
      notes: string[];
    };

/**
 * The body as it is read: narrative runs merged into lead paragraphs, and
 * sentences that cite a quote's lines moved out as that quote's note.
 */
export function planBody(blocks: BlockView[]): BodyPiece[] {
  const pieces: BodyPiece[] = [];
  let pending = "";
  const flushLead = (key: string) => {
    if (pending) {
      pieces.push({ type: "lead", key, markdown: pending });
    }
    pending = "";
  };
  blocks.forEach((block, index) => {
    if (block.type === "narrative") {
      pending = pending ? `${pending}\n\n${block.markdown}` : block.markdown;
      return;
    }
    if (block.type === "explainer") {
      flushLead(`lead-${index}`);
      pieces.push({ type: "explainer", key: `explainer-${index}`, index, block });
      return;
    }
    const assigned = assignNotes(pending, block.quotes);
    pending = assigned.lead;
    flushLead(`lead-${index}`);
    pieces.push({ type: "changes", key: `changes-${index}`, index, block, notes: assigned.notes });
  });
  flushLead("lead-end");
  return pieces;
}

/** Whether any quote in the plan has a note. */
export function hasNotes(pieces: BodyPiece[]): boolean {
  return pieces.some((piece) => piece.type === "changes" && piece.notes.some(Boolean));
}
