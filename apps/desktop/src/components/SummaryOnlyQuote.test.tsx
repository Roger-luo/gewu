import { render, screen } from "@testing-library/react";

import { SummaryOnlyQuote } from "./SummaryOnlyQuote";

describe("summary-only quote", () => {
  it("numbers each line and marks it added or removed", () => {
    render(
      <SummaryOnlyQuote
        quote={{
          change_id: "c",
          path: "notes.txt",
          start: 1,
          end: 2,
          language: null,
          more_above: false,
          more_below: false,
          lines: [
            { kind: "removed", text: "plain notes", old_line: 1, new_line: null },
            { kind: "added", text: "plain notes, updated", old_line: null, new_line: 1 },
          ],
        }}
      />,
    );
    const rows = screen.getAllByRole("row");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveClass("removed");
    expect(rows[0]).toHaveTextContent("1−removedplain notes");
    expect(rows[1]).toHaveClass("added");
    expect(rows[1]).toHaveTextContent("1+addedplain notes, updated");
  });
});
