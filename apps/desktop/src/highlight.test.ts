import { describe, expect, it } from "vitest";

import { diagramLabel, mermaidFlow, readableSvg } from "./diagram";
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

  it("keeps a diagram at its own size instead of scaling the labels away", () => {
    const svg = readableSvg(
      '<svg viewBox="0 0 480.4 220.2" width="100%" style="max-width: 200px"><text>read the lock</text></svg>',
    );
    expect(svg).toContain('width="481"');
    expect(svg).toContain('height="221"');
    expect(svg).not.toContain("100%");
    expect(svg).not.toContain("max-width");
  });

  it("names a diagram by its node labels", () => {
    expect(diagramLabel('flowchart TD\n  A["review pr N"] --> B[GenerateArgs::run]\n  B --> C(fetch)')).toBe(
      "Diagram of review pr N, GenerateArgs::run, fetch",
    );
  });
});
