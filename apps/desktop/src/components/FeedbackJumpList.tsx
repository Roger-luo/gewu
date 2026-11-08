import type { Feedback, ReviewTarget, Snapshot } from "../protocol";

/** Feedback items, each a link back to the chunk it was left on. */
export function FeedbackJumpList({
  items,
  chunks,
  empty,
  onNavigate,
}: {
  items: Feedback[];
  chunks: Snapshot["chunks"];
  empty: string;
  onNavigate: (target: ReviewTarget) => void;
}) {
  if (items.length === 0) {
    return <p className="muted">{empty}</p>;
  }
  return (
    <ul className="jump-list">
      {items.map((item) => {
        const index = chunks.findIndex((chunk) => chunk.id === item.chunk_id);
        return (
          <li key={item.id}>
            <button type="button" onClick={() => index >= 0 && onNavigate({ screen: "chunk", index })}>
              {item.body}
            </button>
          </li>
        );
      })}
    </ul>
  );
}
