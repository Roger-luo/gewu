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
            share: "about 52 MiB of the 573 MiB drop at 2^19 (≈9%), estimated",
            how:
              "Each exponential step builds an index from Pauli string to its position in the basis. The old index kept a full copy of every string beside its position; this one keeps only the position, so the index is about 11× smaller. That is a small part of the measured drop: most of it comes from steps reusing their storage and from the smaller `Word<C>`, in other sections.",
          }),
          narrative(
            "**What one entry holds.** The old `FxHashMap<Word<C>, u32>` stores a 48-byte word and a 4-byte position, padded to 56 bytes (the PR's own S = 56·B), plus one control byte per bucket. `CompactBasisIndex` stores only the position and finds the word through the borrowed `basis` slice.",
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
                  { name: "xbits", bytes: 16 },
                  { name: "zbits", bytes: 16 },
                  { name: "nqubits", bytes: 8 },
                  { name: "hash_cache", bytes: 8 },
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
              "PauliWord fields at the base revision (ppvm-pauli-word/src/word/data.rs:43-52), 128 qubits; padding to 8-byte alignment; hashbrown keeps one control byte per bucket.",
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
              { label: "Before", formula: "next_pow2(b * 8 / 7) * 57", unit: "bytes" },
              { label: "After", formula: "next_pow2(b * 8 / 7) * 5", unit: "bytes" },
            ],
            note:
              "An estimate of this index alone. Peak memory also falls because steps reuse their workspaces instead of allocating new ones.",
            source: "Entry sizes from the layout above; hashbrown's 7/8 load factor (from hashbrown's documentation).",
          }),
          narrative(
            "**How a lookup finds a word without a copy.** Step through `get_hashed`, which callers use when they already hold the word's hash.",
          ),
          explainer({
            kind: "stepper",
            title: "One lookup, step by step",
            language: "rust",
            source: "crates/ppvm-lindblad/src/compact_index.rs:37-41",
            start_line: 37,
            steps: [
              {
                caption: "The caller already computed `word_hash(word)` while building the row, so the lookup starts from the hash and does no hashing of its own.",
                code: "pub(crate) fn get_hashed(&self, word: &Word<C>, hash: u64) -> Option<u32> {\n    self.table\n        .find(hash, |&index| self.basis[index as usize] == *word)\n        .copied()\n}",
                highlight: "37",
                state: [
                  { name: "hash", value: "0x9e37_79b9_7f4a_7c15" },
                  { name: "index", value: "—" },
                ],
              },
              {
                caption: "`find` probes the buckets whose control byte matches the hash's top 7 bits. Each candidate holds only a `u32` position.",
                highlight: "38-39",
                state: [
                  { name: "hash", value: "0x9e37_79b9_7f4a_7c15" },
                  { name: "index", value: "41_027" },
                ],
              },
              {
                caption: "The candidate is confirmed by comparing the real word at `basis[index]`. This read replaces the copy the old map kept, and it is what keeps two words with the same 64-bit hash apart.",
                highlight: "39",
                state: [
                  { name: "hash", value: "0x9e37_79b9_7f4a_7c15" },
                  { name: "index", value: "41_027" },
                  { name: "basis[index] == *word", value: "true" },
                ],
              },
              {
                caption: "The position is returned. Callers index their arrays with it; no `Word<C>` was cloned or hashed again.",
                highlight: "40",
                state: [
                  { name: "hash", value: "0x9e37_79b9_7f4a_7c15" },
                  { name: "index", value: "41_027" },
                  { name: "returns", value: "Some(41_027)" },
                ],
              },
            ],
          }),
          narrative(
            "**What it costs.** With no copy in the table, every lookup confirms a hit by comparing against `basis[index]` (step 3 of the lookup above). That reads 32 bytes the lookup was about to use anyway, and keeps the index correct when two strings share a 64-bit hash.",
          ),

        ],
      }),
      chunk({
        id: "spec-workspaces-and-plan",
        index: 1,
        title: "Step storage and the parity plan live on the spec",
        summary:
          "LindbladSpec gains a pool of step workspaces and a parity plan compiled once in `new`.",
        kind: "integration",
        section: "Position-Indexed Basis Lookup",
        blocks: [
          narrative(
            "**Where more of the memory goes.** These two fields move work out of the step: a step takes a workspace from the pool and returns it, and the parity plan is compiled once when the model is built.",
          ),
          changes("diff", [
            quote({
              change_id: "spec-fields",
              path: "crates/ppvm-lindblad/src/spec.rs",
              start: 55,
              end: 58,
              language: "rust",
              lines: [
                line("context", "pub struct LindbladSpec<const C: usize = W_CHUNKS> {", 55, 55),
                line("added", "    pub(crate) workspaces: std::sync::Mutex<Vec<crate::PcStepWorkspace<C>>>,", null, 56),
                line("added", "    pub(crate) pauli_plan: Option<crate::pauli_plan::PauliActionPlan<C>>,", null, 57),
                line("context", "    n_qubits: usize,", 56, 58),
              ],
            }),
          ]),
          explainer({
            kind: "timeline",
            title: "Six steps, before and after",
            steps: 6,
            step_label: "step",
            lanes: [
              {
                label: "Before",
                work: [
                  { label: "allocate step workspace", when: "each" },
                  { label: "build parity tables", when: "each" },
                ],
              },
              {
                label: "After",
                work: [
                  { label: "workspace from the pool", when: "once" },
                  { label: "parity plan in new", when: "once" },
                ],
              },
            ],
          }),
        ],
      }),
    ],
  });
}
