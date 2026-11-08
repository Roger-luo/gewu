import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

/** WCAG 2.5.8 Target Size (Minimum): 24×24 CSS px; inline links in text exempt. */
export async function runTargetSizeChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  const small = await page.evaluate((): { selector: string; w: number; h: number }[] => {
    const selectors =
      'a[href], button, input, select, textarea, [role="button"], [role="link"], [tabindex]:not([tabindex="-1"])';
    const out: { selector: string; w: number; h: number }[] = [];
    for (const el of Array.from(document.querySelectorAll(selectors)) as HTMLElement[]) {
      if (isInlineLinkInText(el)) continue;
      const r = el.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) continue;
      if (r.width >= 24 && r.height >= 24) continue;
      out.push({
        selector: el.id ? `#${CSS.escape(el.id)}` : describe(el),
        w: Math.round(r.width * 10) / 10,
        h: Math.round(r.height * 10) / 10,
      });
    }
    return out.slice(0, 30);

    function isInlineLinkInText(el: HTMLElement): boolean {
      if (el.tagName !== "A") return false;
      const parent = el.parentElement;
      if (!parent) return false;
      const display = getComputedStyle(el).display;
      if (display !== "inline" && display !== "inline-block" && display !== "contents") {
        return false;
      }
      return Array.from(parent.childNodes).some(
        (n) => n.nodeType === Node.TEXT_NODE && (n.textContent || "").trim().length > 0,
      );
    }

    function describe(el: HTMLElement): string {
      const name = el.getAttribute("aria-label") || el.getAttribute("name") || el.textContent?.trim();
      const tag = el.tagName.toLowerCase();
      return name ? `${tag}("${name.slice(0, 40)}")` : tag;
    }
  });

  return small.map((s) => ({
    key: "targets.too_small" as const,
    page: meta.pageId,
    viewport: meta.viewport,
    selector: s.selector,
    description: `Interactive target ${s.w}×${s.h}px (< 24×24)`,
  }));
}
