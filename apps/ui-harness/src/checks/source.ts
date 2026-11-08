import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

/**
 * Flags source quotes left as one color, overview diagrams that stayed ASCII
 * art, and diagrams whose labels are drawn smaller than body text. SVG scaling
 * does not change a label's CSS font-size, so the size check measures the
 * glyphs on screen.
 */
export async function runSourceChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  // No nested functions: the harness loader wraps them in a call that does not
  // exist inside the page.
  const found = await page.evaluate((): { key: "code.unhighlighted" | "illustration.ascii" | "illustration.too_small"; selector: string; description: string }[] => {
    const out: { key: "code.unhighlighted" | "illustration.ascii" | "illustration.too_small"; selector: string; description: string }[] = [];
    const boxDrawing = /[┌┐└┘│─├┤┬┴┼╭╮╯╰]/;

    for (const el of Array.from(document.querySelectorAll("pre.code-plain, .summary-only"))) {
      if (el.classList.contains("shiki") || el.querySelector(".shiki")) continue;
      const text = el.textContent ?? "";
      const lines = text.split("\n").map((line) => line.trim()).filter((line) => line.length > 0);
      const assignments = lines.filter((line) => line.includes(" = ")).length;
      const patterns = lines.filter((line) => /^[#/*!]/.test(line) || line.endsWith("/")).length;
      if (lines.length < 2 || text.trim().length < 40 || (assignments < 1 && patterns < 2)) continue;
      const name = typeof el.className === "string" ? el.className.split(/\s+/)[0] : "";
      out.push({
        key: "code.unhighlighted",
        selector: name ? `${el.tagName.toLowerCase()}.${name}` : el.tagName.toLowerCase(),
        description: `Source shown without syntax color: "${text.trim().slice(0, 80)}"`,
      });
    }

    for (const table of Array.from(document.querySelectorAll("table.diff-table"))) {
      if (table.querySelector("code span[style*='color']")) continue;
      const text = Array.from(table.querySelectorAll("code"))
        .map((cell) => cell.textContent ?? "")
        .join("\n");
      const lines = text.split("\n").map((line) => line.trim()).filter((line) => line.length > 0);
      const assignments = lines.filter((line) => line.includes(" = ")).length;
      const patterns = lines.filter((line) => /^[#/*!]/.test(line) || line.endsWith("/")).length;
      if (lines.length < 2 || text.trim().length < 40 || (assignments < 1 && patterns < 2)) continue;
      out.push({
        key: "code.unhighlighted",
        selector: "table.diff-table",
        description: `Source shown without syntax color: "${text.trim().slice(0, 80)}"`,
      });
    }

    for (const svg of Array.from(document.querySelectorAll(".illustration svg"))) {
      if (!(svg.textContent ?? "").trim()) {
        out.push({
          key: "illustration.ascii",
          selector: ".illustration svg",
          description: "Diagram drew no labels: its boxes are empty",
        });
        continue;
      }
      const tiny = Array.from(svg.querySelectorAll("text, tspan")).find((label) => {
        const box = label.getBoundingClientRect();
        return box.height > 0 && box.height < 12;
      });
      if (tiny) {
        const height = tiny.getBoundingClientRect().height;
        out.push({
          key: "illustration.too_small",
          selector: ".illustration svg text",
          description: `Diagram label rendered ${height.toFixed(1)}px tall: "${(tiny.textContent ?? "").trim().slice(0, 60)}"`,
        });
      }
    }

    for (const el of Array.from(document.querySelectorAll(".illustration-ascii, .illustration"))) {
      if (el.classList.contains("illustration") && (el.querySelector("svg") || el.querySelector(".illustration-ascii"))) {
        continue;
      }
      const text = el.textContent ?? "";
      if (!el.classList.contains("illustration-ascii") && !boxDrawing.test(text)) continue;
      const name = typeof el.className === "string" ? el.className.split(/\s+/)[0] : "";
      out.push({
        key: "illustration.ascii",
        selector: name ? `${el.tagName.toLowerCase()}.${name}` : el.tagName.toLowerCase(),
        description: `Diagram is text, not a drawing: "${text.trim().slice(0, 80)}"`,
      });
      if (out.length >= 25) break;
    }
    return out;
  });

  return found.map((item) => ({
    key: item.key,
    page: meta.pageId,
    viewport: meta.viewport,
    selector: item.selector,
    description: item.description,
  }));
}
