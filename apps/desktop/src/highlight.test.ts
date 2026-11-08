import { describe, expect, it } from "vitest";

import { mermaidFlow } from "./diagram";
import { highlightCode } from "./highlight";

describe("syntax highlighting", () => {
  it("colors a TOML lock file", async () => {
    const html = await highlightCode(
      '[[skill]]\nname = "writing"\nkind = "local"\n',
      "toml",
      true,
    );
    expect(html).toContain("shiki");
    expect(new Set(html?.match(/color:[^;"]+/g))).not.toHaveLength(1);
  });

  it("colors a gitignore comment differently from a pattern", async () => {
    const html = await highlightCode("# generated\n/build/\n*.synctex.gz\n", "gitignore", true);
    expect(html).toContain("shiki");
    expect(new Set(html?.match(/color:[^;"]+/g)).size).toBeGreaterThan(1);
  });
});

describe("diagrams", () => {
  it("turns a sequence into a flowchart", () => {
    expect(mermaidFlow(["read the lock", 'write "Ion.lock"'])).toBe(
      ['flowchart TD', '  s0["read the lock"]', '  s1["write \'Ion.lock\'"]', "  s0 --> s1"].join("\n"),
    );
  });
});
