import { anchorInput, anchorLabel, quoteSpan } from "./anchors";
import type { QuoteLine, QuoteView } from "./protocol";

function quote(lines: QuoteLine[]): QuoteView {
  return {
    change_id: "change-1",
    path: "src/lib.rs",
    start: 1,
    end: lines.length,
    language: "rust",
    more_above: false,
    more_below: false,
    lines,
  };
}

describe("quoteSpan", () => {
  it("spans the new-side lines of a block that adds code", () => {
    const span = quoteSpan(
      quote([
        { kind: "context", old_line: 9, new_line: 9, text: "fn a() {}" },
        { kind: "added", old_line: null, new_line: 10, text: "fn b() {}" },
        { kind: "added", old_line: null, new_line: 11, text: "fn c() {}" },
      ]),
    );
    expect(span).toEqual({
      change_id: "change-1",
      path: "src/lib.rs",
      side: "new",
      start_line: 9,
      end_line: 11,
    });
  });

  it("falls back to the old side for a block of pure deletions", () => {
    const span = quoteSpan(
      quote([
        { kind: "removed", old_line: 40, new_line: null, text: "let x = 1;" },
        { kind: "removed", old_line: 41, new_line: null, text: "let y = 2;" },
      ]),
    );
    expect(span).toMatchObject({ side: "old", start_line: 40, end_line: 41 });
  });

  it("is null when the quote carries no line numbers", () => {
    expect(quoteSpan(quote([]))).toBeNull();
  });
});

describe("anchors", () => {
  it("sends the side with the span", () => {
    expect(
      anchorInput({
        change_id: "change-1",
        path: "src/lib.rs",
        side: "old",
        start_line: 40,
        end_line: 41,
      }),
    ).toEqual({ change_id: "change-1", side: "old", start_line: 40, end_line: 41 });
  });

  it("labels a span with its range and marks the old side", () => {
    expect(anchorLabel({ path: "a.rs", side: "new", start_line: 3, end_line: 3 })).toBe(
      "a.rs:3",
    );
    expect(anchorLabel({ path: "a.rs", side: "new", start_line: 3, end_line: 9 })).toBe(
      "a.rs:3-9",
    );
    expect(anchorLabel({ path: "a.rs", side: "old", start_line: 3, end_line: 9 })).toBe(
      "a.rs:3-9 (before)",
    );
  });
});
