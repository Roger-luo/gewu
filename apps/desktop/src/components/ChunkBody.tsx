import type { CommentRequest, LineSelection } from "../anchors";
import type { BodyPiece } from "../bodyPlan";
import { locate, type Composer } from "../composer";
import type { BlockView } from "../protocol";
import { ChangesBlock } from "./ChangesBlock";
import { ExplainerView } from "./explainers/ExplainerView";
import { Markdown } from "./Markdown";

export function ChunkBody({
  blocks,
  pieces,
  lineSelection,
  onSelectLine,
  onComment,
  onCommentLines,
  composer,
}: {
  blocks: BlockView[];
  pieces: BodyPiece[];
  lineSelection: LineSelection | null;
  onSelectLine: (changeId: string, line: number, extend: boolean) => void;
  onComment: (request: CommentRequest) => void;
  onCommentLines: (request: CommentRequest) => void;
  composer: Composer | null;
}) {
  const at = composer && locate(blocks, composer.place);
  return (
    <>
      {pieces.map((piece) => {
        switch (piece.type) {
          case "lead":
            return (
              <div className="block" key={piece.key}>
                <Markdown source={piece.markdown} />
              </div>
            );
          case "explainer":
            return (
              <div className="block" key={piece.key}>
                <ExplainerView explainer={piece.block} />
              </div>
            );
          case "changes":
            return (
              <ChangesBlock
                key={piece.key}
                block={piece.block}
                notes={piece.notes}
                selected={lineSelection}
                onSelectLine={onSelectLine}
                onComment={onComment}
                onCommentLines={onCommentLines}
                composer={
                  composer && at && at[0] === piece.index ? { ...composer, quote: at[1] } : null
                }
              />
            );
        }
      })}
    </>
  );
}
