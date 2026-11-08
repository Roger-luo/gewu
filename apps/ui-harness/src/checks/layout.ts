import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

interface LayoutProbe {
  horizontalOverflow: boolean;
  clipped: { selector: string; text: string }[];
  overlaps: { a: string; b: string }[];
}

export async function runLayoutChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  const probe = await page.evaluate((): LayoutProbe => {
    const docEl = document.documentElement;
    const horizontalOverflow = docEl.scrollWidth > docEl.clientWidth + 1;

    const clipped: { selector: string; text: string }[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT);
    let node = walker.nextNode();
    while (node) {
      const el = node as HTMLElement;
      node = walker.nextNode();
      if (!el.offsetParent && getComputedStyle(el).position !== "fixed") continue;
      if (el.getAttribute("aria-hidden") === "true") continue;
      const style = getComputedStyle(el);
      // Screen-reader-only patterns hide text on purpose.
      if (style.clipPath !== "none" || style.clip !== "auto") continue;
      if (style.textOverflow === "ellipsis" || el.title) continue;
      // `text-overflow: clip` is the CSS default, so only `overflow: hidden`
      // on an axis that actually overflows hides content; auto/scroll panes
      // keep it reachable.
      const clipsX =
        (style.overflowX === "hidden" || style.overflowX === "clip") &&
        el.scrollWidth > el.clientWidth + 1;
      const clipsY =
        (style.overflowY === "hidden" || style.overflowY === "clip") &&
        el.scrollHeight > el.clientHeight + 1;
      if (!clipsX && !clipsY) continue;
      const text = (el.innerText || "").trim().slice(0, 80);
      if (!text) continue;
      clipped.push({
        selector: cssPath(el),
        text,
      });
    }

    const modal = document.querySelector(".dialog-backdrop .dialog[role='dialog']");
    const interactive = (
      Array.from(
        document.querySelectorAll(
          'a[href], button, input, select, textarea, [role="button"], [tabindex]:not([tabindex="-1"])',
        ),
      ) as HTMLElement[]
    ).filter((el) => {
      if (el.closest("[inert], [aria-hidden='true']")) return false;
      // A modal's backdrop covers the page; only score controls inside it.
      if (modal && !modal.closest(".dialog-backdrop")?.contains(el)) return false;
      return true;
    });
    const boxes = interactive
      .map((el) => {
        const style = getComputedStyle(el);
        if (style.visibility === "hidden" || style.pointerEvents === "none") return null;
        if (Number(style.opacity) === 0) return null;
        const r = el.getBoundingClientRect();
        if (r.width < 1 || r.height < 1) return null;
        return { el, r, sel: cssPath(el) };
      })
      .filter((x): x is NonNullable<typeof x> => x !== null);

    const overlaps: { a: string; b: string }[] = [];
    for (let i = 0; i < boxes.length; i++) {
      for (let j = i + 1; j < boxes.length; j++) {
        const a = boxes[i]!;
        const b = boxes[j]!;
        if (rectsOverlap(a.r, b.r) && !a.el.contains(b.el) && !b.el.contains(a.el)) {
          overlaps.push({ a: a.sel, b: b.sel });
        }
      }
    }

    return { horizontalOverflow, clipped: clipped.slice(0, 20), overlaps: overlaps.slice(0, 20) };

    function rectsOverlap(a: DOMRect, b: DOMRect): boolean {
      return !(a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top);
    }

    function cssPath(el: Element): string {
      if (el.id) return `#${CSS.escape(el.id)}`;
      const tag = el.tagName.toLowerCase();
      const parent = el.parentElement;
      if (!parent) return tag;
      const siblings = Array.from(parent.children).filter((c) => c.tagName === el.tagName);
      if (siblings.length > 1) {
        return `${tag}:nth-of-type(${siblings.indexOf(el) + 1})`;
      }
      return tag;
    }
  });

  const findings: CheckFinding[] = [];
  if (probe.horizontalOverflow) {
    findings.push({
      key: "layout.horizontal_overflow",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: "html",
      description: "Document scrolls horizontally",
    });
  }
  for (const c of probe.clipped) {
    findings.push({
      key: "layout.clipped_text",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: c.selector,
      description: `Clipped text without ellipsis/title: "${c.text}"`,
    });
  }
  for (const o of probe.overlaps) {
    findings.push({
      key: "layout.overlap",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: `${o.a} ∩ ${o.b}`,
      description: "Interactive elements overlap",
    });
  }
  return findings;
}
