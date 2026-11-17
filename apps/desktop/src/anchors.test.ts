import { anchorInput, anchorLabel, lineSpan, narrowSpan, quoteSpan, selectLine } from "./anchors";
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

describe("selectLine", () => {
  it("picks one line, extends from it with shift, and clears on a repeat click", () => {
    const one = selectLine(null, "c", 10, false);
    expect([...(one?.lines ?? [])]).toEqual([10]);
    const run = selectLine(one, "c", 7, true);
    expect([...(run?.lines ?? [])].sort((a, b) => a - b)).toEqual([7, 8, 9, 10]);
    expect(selectLine(run, "c", 12, true)?.lines.size).toBe(3);
    expect(selectLine(one, "c", 10, false)).toBeNull();
  });

  it("starts over in another change even with shift", () => {
    const other = selectLine(selectLine(null, "a", 3, false), "b", 9, true);
    expect(other?.changeId).toBe("b");
    expect([...(other?.lines ?? [])]).toEqual([9]);
  });
});

describe("line spans", () => {
  const block = { change_id: "c", path: "p", side: "new" as const, start_line: 5, end_line: 20 };

  it("comments on the selection when the line is in it, else on that line", () => {
    expect(lineSpan(block, new Set([8, 9]), 9)).toMatchObject({ start_line: 8, end_line: 9 });
    expect(lineSpan(block, new Set([8, 9]), 15)).toMatchObject({ start_line: 15, end_line: 15 });
  });

  it("narrows a block only to selected lines inside it", () => {
    expect(narrowSpan(block, new Set([2, 30]))).toEqual(block);
    expect(narrowSpan(block, new Set([6, 7]))).toMatchObject({ start_line: 6, end_line: 7 });
  });
});
