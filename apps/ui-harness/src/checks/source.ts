import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

/**
 * Flags source quotes left as one color, and overview diagrams that stayed
 * ASCII art. A drawn diagram is an svg, so a box-drawing picture or the
 * "shown as text" fallback is the defect.
 */
export async function runSourceChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  // No nested functions: the harness loader wraps them in a call that does not
  // exist inside the page.
  const found = await page.evaluate((): { key: "code.unhighlighted" | "illustration.ascii"; selector: string; description: string }[] => {
    const out: { key: "code.unhighlighted" | "illustration.ascii"; selector: string; description: string }[] = [];
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
