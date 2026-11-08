import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

/**
 * Flags a caption or control that shares a row with controls but does not sit
 * on their center line, e.g. a "Leave feedback" caption stuck to the top of a
 * row of taller buttons. Captions are measured by their glyphs, not their box,
 * because a stretched box hides a top-aligned caption.
 */
export async function runAlignmentChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  // No named helpers inside evaluate: tsx wraps them in a `__name` call that
  // does not exist in the page.
  const misaligned = await page.evaluate((): { selector: string; text: string; offset: number }[] => {
    const TOLERANCE = 3;
    const control = "button, a[href], input, select, [role=button], [role=tab]";
    const out: { selector: string; text: string; offset: number }[] = [];

    for (const row of Array.from(document.querySelectorAll<HTMLElement>("body *"))) {
      const style = getComputedStyle(row);
      if (!style.display.includes("flex") || style.flexDirection !== "row") continue;
      const items: { el: HTMLElement; rect: DOMRect }[] = [];
      for (const el of Array.from(row.children)) {
        if (!(el instanceof HTMLElement)) continue;
        const own = getComputedStyle(el);
        if (own.display === "none" || own.visibility === "hidden" || own.position === "absolute") continue;
        let rect = el.getBoundingClientRect();
        if (!el.matches(control)) {
          // Only captions are compared; a block of content beside a button
          // (a title and its eyebrow) is laid out on purpose.
          if (el.children.length > 0 || (el.textContent ?? "").trim() === "") continue;
          const range = document.createRange();
          range.selectNodeContents(el);
          rect = range.getBoundingClientRect();
        }
        if (rect.width > 0 && rect.height > 0) items.push({ el, rect });
      }
      const anchor = items.find((item) => item.el.matches(control));
      if (!anchor || items.length < 2) continue;
      const center = anchor.rect.top + anchor.rect.height / 2;
      for (const item of items) {
        if (item === anchor) continue;
        const overlaps = item.rect.top < anchor.rect.bottom && item.rect.bottom > anchor.rect.top;
        if (!overlaps) continue;
        const offset = item.rect.top + item.rect.height / 2 - center;
        if (Math.abs(offset) <= TOLERANCE) continue;
        const name = typeof item.el.className === "string" ? item.el.className.split(/\s+/)[0] : "";
        out.push({
          selector: name ? `${item.el.tagName.toLowerCase()}.${name}` : item.el.tagName.toLowerCase(),
          text: (item.el.textContent ?? "").trim().slice(0, 60),
          offset,
        });
      }
      if (out.length >= 25) break;
    }
    return out;
  });

  return misaligned.map((item) => ({
    key: "layout.misaligned" as const,
    page: meta.pageId,
    viewport: meta.viewport,
    selector: item.selector,
    description: `"${item.text}" sits ${Math.abs(item.offset).toFixed(0)}px ${item.offset < 0 ? "above" : "below"} the center line of the controls beside it`,
  }));
}
