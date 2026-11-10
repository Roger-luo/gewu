import { describe, expect, it } from "vitest";

import { assignNotes } from "./quoteNotes";
import type { QuoteView } from "./protocol";

function quote(start: number, end: number): QuoteView {
  const lines = [];
  for (let line = start; line <= end; line += 1) {
    lines.push({ kind: "added" as const, old_line: null, new_line: line, text: "code" });
  }
  return {
    change_id: `c-${start}`,
    path: "sector.rs",
    start: 1,
    end: end - start + 1,
    language: "rust",
    more_above: false,
    more_below: false,
    lines,
  };
}

const NARRATIVE =
  "The first section replaced the `PauliWord` alias in `word.rs`. " +
  "`Sector::canonicalize_phase` (lines 95–103), `Sector::orbit_size` (lines 118–121), " +
  "and `canonicalize_basis_to_rep` (line 135) pass `self.group.n_qubits()` to `to_pauli_word` " +
  "before canonicalizing and convert the representative back with `Word::from_pauli_word`.";

describe("assignNotes", () => {
  it("puts each named call site beside the view that shows its lines", () => {
    const assigned = assignNotes(NARRATIVE, [quote(95, 103), quote(118, 121), quote(135, 135)]);
    expect(assigned.lead).toBe("The first section replaced the `PauliWord` alias in `word.rs`.");
    expect(assigned.notes[0]).toContain("Sector::canonicalize_phase");
    expect(assigned.notes[1]).toContain("Sector::orbit_size");
    expect(assigned.notes[2]).toContain("canonicalize_basis_to_rep");
    for (const note of assigned.notes) {
      expect(note).toContain("to_pauli_word");
      expect(note).not.toContain("lines 95");
    }
  });

  it("keeps a sentence that cites nothing above the code", () => {
    const assigned = assignNotes("Nothing here names a line.", [quote(1, 3)]);
    expect(assigned.lead).toContain("Nothing here");
    expect(assigned.notes).toEqual([""]);
  });

  it("gives a sentence that cites one view to that view whole", () => {
    const assigned = assignNotes("Look at `orbit_size` (lines 118-121) only.", [
      quote(95, 103),
      quote(118, 121),
    ]);
    expect(assigned.notes[0]).toBe("");
    expect(assigned.notes[1]).toContain("orbit_size");
  });
});
