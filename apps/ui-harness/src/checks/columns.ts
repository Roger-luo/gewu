import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

/**
 * Flags a list whose status labels do not start on one shared x position, for
 * example rows where a button beside the status pushes it left in some rows
 * and not in others. A reader scans the status as a column, so it must line up.
 */
export async function runColumnChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  // No named helpers inside evaluate: tsx wraps them in a `__name` call that
  // does not exist in the page.
  const drifting = await page.evaluate((): { selector: string; spread: number; count: number }[] => {
    const TOLERANCE = 2;
    const out: { selector: string; spread: number; count: number }[] = [];
    const lists = new Map<Element, number[]>();
    for (const pill of Array.from(document.querySelectorAll(".status-text"))) {
      const list = pill.closest("ul, ol, [role=list]");
      const rect = pill.getBoundingClientRect();
      if (!list || rect.width === 0 || rect.height === 0) continue;
      const lefts = lists.get(list) ?? [];
      lefts.push(rect.left);
      lists.set(list, lefts);
    }
    for (const [list, lefts] of Array.from(lists.entries())) {
      if (lefts.length < 2) continue;
      const spread = Math.max(...lefts) - Math.min(...lefts);
      if (spread <= TOLERANCE) continue;
      const name = typeof list.className === "string" ? list.className.split(/\s+/)[0] : "";
      out.push({
        selector: name ? `${list.tagName.toLowerCase()}.${name}` : list.tagName.toLowerCase(),
        spread,
        count: lefts.length,
      });
    }
    return out.slice(0, 25);
  });

  return drifting.map((item) => ({
    key: "layout.column_drift" as const,
    page: meta.pageId,
    viewport: meta.viewport,
    selector: item.selector,
    description: `Status labels in this list of ${item.count} start ${item.spread.toFixed(0)}px apart, so the status is not one column`,
  }));
}
