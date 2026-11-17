import { plainInline, renderInlineMarkdown, renderMarkdown } from "./markdown";

describe("math in agent prose", () => {
  it("typesets inline and display TeX", () => {
    const html = renderMarkdown("A gate acts as $G^\\dagger P G$.\n\n$$\nXX \\cdot ZZ = -YY\n$$\n");
    expect(html).toContain('class="katex"');
    expect(html).toContain("katex-display");
    expect(html).not.toMatch(/|\$/);
  });

  it("typesets math in a title or summary", () => {
    expect(renderInlineMarkdown("Maps $X$ to $-Y$")).toContain('class="katex"');
  });

  it("leaves dollar signs that are not math as text", () => {
    for (const text of ["Set $HOME and $PATH", "costs $5 and $10", "a `$x$` code span"]) {
      expect(renderMarkdown(text)).not.toContain("katex");
    }
    expect(renderMarkdown("escaped \\$x\\$ stays")).toContain("$x$");
  });

  it("does not let TeX inject markup", () => {
    const html = renderMarkdown("$\\href{javascript:alert(1)}{x}$ and $<img src=x onerror=alert(1)>$");
    // KaTeX keeps the source as escaped text in a MathML annotation; what
    // matters is that none of it became an element or a live link.
    const doc = new DOMParser().parseFromString(html, "text/html");
    expect(doc.querySelector("img, [onerror], a[href^='javascript']")).toBeNull();
  });

  it("reads math as its source in plain text", () => {
    expect(plainInline("Maps $X$ to Y")).toContain("X");
  });
});
