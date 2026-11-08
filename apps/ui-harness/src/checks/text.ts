import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

export async function runTextSizeChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  const small = await page.evaluate((): { selector: string; px: number; text: string }[] => {
    const out: { selector: string; px: number; text: string }[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    let node = walker.nextNode();
    while (node) {
      const text = (node.textContent || "").trim();
      const parent = node.parentElement;
      node = walker.nextNode();
      if (!text || !parent) continue;
      const style = getComputedStyle(parent);
      if (style.display === "none" || style.visibility === "hidden") continue;
      const px = parseFloat(style.fontSize);
      if (!(px < 12)) continue;
      const r = parent.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) continue;
      out.push({
        selector: parent.id ? `#${CSS.escape(parent.id)}` : parent.tagName.toLowerCase(),
        px,
        text: text.slice(0, 60),
      });
      if (out.length >= 25) break;
    }
    return out;
  });

  return small.map((s) => ({
    key: "text.too_small" as const,
    page: meta.pageId,
    viewport: meta.viewport,
    selector: s.selector,
    description: `Text rendered at ${s.px.toFixed(1)}px: "${s.text}"`,
  }));
}
