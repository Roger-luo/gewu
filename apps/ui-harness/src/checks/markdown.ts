import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

/**
 * Flags Markdown syntax that reached the screen as literal characters, e.g. a
 * title printed as `foo` instead of rendering a code span. Text inside code,
 * pre, and form fields is source by design and is skipped.
 */
export async function runRawMarkdownChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  const raw = await page.evaluate((): { selector: string; text: string; syntax: string }[] => {
    const patterns: [string, RegExp][] = [
      ["code span", /`[^`\s][^`]*`/],
      ["bold", /\*\*[^*\s][^*]*\*\*/],
      ["link", /\[[^\]]+\]\([^)\s]+\)/],
      ["heading", /^#{1,6}\s+\S/],
    ];
    const skip = "code, pre, kbd, samp, textarea, input, script, style, [aria-hidden=true] svg";
    const out: { selector: string; text: string; syntax: string }[] = [];
    const seen = new Set<Element>();
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const parent = node.parentElement;
      if (!parent || seen.has(parent) || parent.closest(skip)) continue;
      const style = getComputedStyle(parent);
      if (style.display === "none" || style.visibility === "hidden") continue;
      const rect = parent.getBoundingClientRect();
      if (rect.width < 1 || rect.height < 1) continue;
      const text = node.textContent ?? "";
      const hit = patterns.find(([, pattern]) => pattern.test(text.trim()));
      if (!hit) continue;
      seen.add(parent);
      out.push({
        selector: parent.className
          ? `${parent.tagName.toLowerCase()}.${String(parent.className).split(/\s+/)[0]}`
          : parent.tagName.toLowerCase(),
        text: text.trim().slice(0, 80),
        syntax: hit[0],
      });
      if (out.length >= 25) break;
    }
    return out;
  });

  return raw.map((item) => ({
    key: "text.raw_markdown" as const,
    page: meta.pageId,
    viewport: meta.viewport,
    selector: item.selector,
    description: `Unrendered Markdown ${item.syntax}: "${item.text}"`,
  }));
}
