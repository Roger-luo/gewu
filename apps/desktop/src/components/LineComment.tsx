/**
 * The "+" beside one line of code, shown when the row is hovered or focused.
 * It opens a comment on that line, or on the selected run when the line is in it.
 */
export function LineComment({ line, onComment }: { line: number; onComment: (line: number) => void }) {
  return (
    <button
      type="button"
      className="line-comment"
      aria-label={`Comment on line ${line}`}
      title="Comment on this line (shift-click line numbers to select a range first)"
      onClick={() => onComment(line)}
    >
      +
    </button>
  );
}
