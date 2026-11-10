import { describe, expect, it } from "vitest";

import { droppedIdentifiers, layoutCode, markIdentifiers } from "./contextCode";

const BEFORE =
  "pub struct PauliWord<A: PauliStorage, S, const R: bool> { pub xbits: BitArray<A>, pub zbits: BitArray<A>, nqubits: usize, hash_cache: u64, _phantom: PhantomData<S> }";
const AFTER =
  "pub struct Word<const C: usize = W_CHUNKS> { pub xbits: BitArray<Chunk, C>, pub zbits: BitArray<Chunk, C> }";
const REASON =
  "Drops `nqubits`, `hash_cache`, and `_phantom` so `Word<2>` shrinks from 48 to 32 bytes.";

describe("context code", () => {
  it("lays a one-line struct out one field per line and keeps generics intact", () => {
    const laid = layoutCode(BEFORE);
    expect(laid).toBe(
      [
        "pub struct PauliWord<A: PauliStorage, S, const R: bool> {",
        "    pub xbits: BitArray<A>,",
        "    pub zbits: BitArray<A>,",
        "    nqubits: usize,",
        "    hash_cache: u64,",
        "    _phantom: PhantomData<S>",
        "}",
      ].join("\n"),
    );
    expect(layoutCode("    already\n    laid out")).toBe("    already\n    laid out");
  });

  it("names only the fields the reason cites that the new definition dropped", () => {
    expect(droppedIdentifiers(REASON, layoutCode(BEFORE), layoutCode(AFTER))).toEqual([
      "nqubits",
      "hash_cache",
      "_phantom",
    ]);
  });

  it("marks those names inside highlighted markup", () => {
    const html = markIdentifiers(
      '<pre class="shiki"><code><span style="color:#f00">nqubits</span>: usize</code></pre>',
      ["nqubits"],
    );
    expect(html).toContain('class="dropped-field"');
    expect(html).toContain(">nqubits<");
    expect(html).toContain("color:#f00");
    expect(markIdentifiers("<pre><code>nqubits_len</code></pre>", ["nqubits"])).not.toContain(
      "dropped-field",
    );
  });
});
