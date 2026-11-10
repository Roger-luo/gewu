import { changes, chunk, line, narrative, quote, snapshot } from "./builders";
import type { Snapshot } from "../protocol";

/** Three call sites named in one paragraph, so each note sits beside its diff. */
export function pairedSnapshot(): Snapshot {
  const row = (text: string, number: number) => line("added", text, null, number);
  return snapshot({
    review_id: "demo-paired",
    overview: {
      intent: "Show a note beside each call site.",
      changes: ["Three conversions in sector.rs."],
    },
    chunks: [
      chunk({
        id: "sector",
        index: 0,
        title: "Momentum-Sector Word Conversions in sector.rs",
        summary:
          "Converts `Word<C>` around `TranslationGroup` calls in `canonicalize_phase`, `orbit_size`, and `canonicalize_basis_to_rep`.",
        kind: "integration",
        blocks: [
          narrative(
            "The first section replaced the `PauliWord` alias in `word.rs` with `Word<C>`. " +
              "`Sector::canonicalize_phase` (lines 95–103), `Sector::orbit_size` (lines 118–121), " +
              "and `canonicalize_basis_to_rep` (line 135) pass `self.group.n_qubits()` to `to_pauli_word` " +
              "before canonicalizing and convert the representative back with `Word::from_pauli_word`.",
          ),
          changes("diff", [
            quote({
              change_id: "c-phase",
              path: "crates/ppvm-lindblad/src/sector.rs",
              start: 1,
              end: 4,
              language: "rust",
              lines: [
                row("let (rep, idx, orbit_size) = self", 95),
                row("    .group", 96),
                row("    .canonicalize_in_sector_indexed(q, &self.characters);", 97),
                row("let word = Word::from_pauli_word(rep);", 100),
              ],
            }),
            quote({
              change_id: "c-orbit",
              path: "crates/ppvm-lindblad/src/sector.rs",
              start: 1,
              end: 3,
              language: "rust",
              lines: [
                row(".canonicalize_in_sector_indexed(", 118),
                row("    to_pauli_word(self.group.n_qubits()),", 119),
                row("    &self.characters,", 120),
              ],
            }),
            quote({
              change_id: "c-basis",
              path: "crates/ppvm-lindblad/src/sector.rs",
              start: 1,
              end: 2,
              language: "rust",
              lines: [
                row("w = group.canonicalize(w);", 134),
                row("w = Word::from_pauli_word(group.canonicalize(w), group.n_qubits());", 135),
              ],
            }),
          ]),
        ],
      }),
    ],
  });
}
