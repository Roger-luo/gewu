import type { Explainer } from "../explainer";
import type { BlockView, Snapshot } from "../protocol";
import { changes, chunk, line, narrative, quote, snapshot } from "./builders";

function explainer(block: Explainer): BlockView {
  return { type: "explainer", ...block };
}

/**
 * `?mock=explainers`: a step from a performance pull request that teaches
 * with a claim, a layout drawn to scale, and a cost model the reader drags.
 */
export function explainersSnapshot(): Snapshot {
  return snapshot({
    review_id: "demo-explainers",
    source: { base: "0f0d781e", head: "327e0610" },
    overview: {
      intent:
        "Make predictor-corrector steps faster and lighter by storing Pauli strings once and reusing step storage.",
      changes: [
        "Index basis strings by position instead of copying each string into the index.",
        "Reuse step workspaces and the compiled parity plan across steps.",
      ],
      concepts: [],
    },
    chunks: [
      chunk({
        id: "compact-basis-index",
        index: 0,
        title: "Position table instead of copied strings",
        summary:
          "CompactBasisIndex stores 4-byte positions into the basis instead of a copy of every Pauli string.",
        kind: "concept",
        section: "Position-Indexed Basis Lookup",
        context: {
          kind: "refactor",
          before: "FxHashMap<Word<C>, u32>  // basis::build_basis_index, still used by LindbladSpec::generator",
          after: "struct CompactBasisIndex<'a, const C: usize> { basis: &'a [Word<C>], table: HashTable<u32> }",
          reason: "Drops the copied `Word<C>` key from every entry: the table keeps a `u32` position and reads the word from the borrowed `basis`.",
        },
        blocks: [
          explainer({
            kind: "claim",
            claim: "use 61–65% less peak memory than the original `main` baseline",
            result: "82-spin ladder peak RSS: 300 → 105 MiB at B = 2^17 (−65%), 940 → 367 MiB at B = 2^19 (−61%)",
            share: "about 36 MiB of the 573 MiB drop at 2^19 (≈6%), estimated",
            how:
              "Each exponential step builds an index from Pauli string to its position in the basis. The old index kept a full copy of every string beside its position; this one keeps only the position, so the index is about 8× smaller. That is a small part of the measured drop: most of it comes from steps reusing their storage and from the smaller `Word<C>`, in other sections.",
          }),
          narrative(
            "**What one entry holds.** The old `FxHashMap<Word<C>, u32>` stores a 32-byte string and a 4-byte position, padded to 40 bytes, plus one control byte per bucket. `CompactBasisIndex` stores only the position and finds the string through the borrowed `basis` slice.",
          ),
          changes("added_code", [
            quote({
              change_id: "compact-index",
              path: "crates/ppvm-lindblad/src/compact_index.rs",
              start: 9,
              end: 12,
              language: "rust",
              lines: [
                line("added", "pub(crate) struct CompactBasisIndex<'a, const C: usize> {", null, 9),
                line("added", "    basis: &'a [Word<C>],", null, 10),
                line("added", "    table: HashTable<u32>,", null, 11),
                line("added", "}", null, 12),
              ],
            }),
          ]),

          explainer({
            kind: "layout",
            title: "One index entry",
            rows: [
              {
                label: "Before",
                fields: [
                  { name: "Word<C> copy", bytes: 32, note: "X and Z planes, 2 × 64-bit words each" },
                  { name: "u32", bytes: 4 },
                  { name: "pad", bytes: 4 },
                  { name: "ctrl", bytes: 1, note: "hashbrown control byte" },
                ],
              },
              {
                label: "After",
                fields: [
                  { name: "u32", bytes: 4, note: "position into basis" },
                  { name: "ctrl", bytes: 1 },
                ],
              },
            ],
            source:
              "Word<C> is 32 bytes at C = 2, the 128-qubit width the benchmarks use (asserted by the size test in compact_word_hash_and_ord); hashbrown keeps one control byte per bucket.",
          }),
          narrative(
            "**What it adds up to.** The table grows to the next power of two above B ÷ 7/8 buckets. The PR measured B = 2^17 and 2^19.",
          ),
          explainer({
            kind: "cost_model",
            title: "Index memory at basis size B",
            params: [
              {
                id: "b",
                label: "Strings in the basis (B)",
                min: 32768,
                max: 2097152,
                default: 524288,
                scale: "log2",
                marks: [131072, 524288],
              },
            ],
            series: [
              { label: "Before", formula: "next_pow2(b * 8 / 7) * 41", unit: "bytes" },
              { label: "After", formula: "next_pow2(b * 8 / 7) * 5", unit: "bytes" },
            ],
            note:
              "An estimate of this index alone. Peak memory also falls because steps reuse their workspaces instead of allocating new ones.",
            source: "Entry sizes from the layout above; hashbrown's 7/8 load factor.",
          }),
          narrative(
            "**What it costs.** With no copy in the table, every lookup confirms a hit by comparing against `basis[i]`, shown in the next step. That reads 32 bytes the lookup was about to use anyway, and keeps the index correct when two strings share a 64-bit hash.",
          ),

        ],
      }),
    ],
  });
}
