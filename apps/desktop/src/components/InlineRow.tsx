import type { InlineRow } from "../composer";

/** The row holding `inline`, spanning every column of the quote's table. */
export function InlineRowView({ inline, columns }: { inline: InlineRow; columns: number }) {
  return (
    <tr className="inline-row">
      <td colSpan={columns}>{inline.node}</td>
    </tr>
  );
}
