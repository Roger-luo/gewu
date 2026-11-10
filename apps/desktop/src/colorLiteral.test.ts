import { describe, expect, it } from "vitest";

import { typstColors } from "./colorLiteral";

describe("typst colors", () => {
  it("reads a quoted hex, a percentage, and luma", () => {
    expect(typstColors('#let ink = rgb("243746")')).toEqual(["#243746"]);
    expect(typstColors('#let muted = rgb("#526575")')).toEqual(["#526575"]);
    expect(typstColors("#let paper = luma(96%)")).toEqual(["#f5f5f5"]);
    expect(typstColors("#let shade = rgb(14%, 22%, 28%)")).toHaveLength(1);
  });
});
