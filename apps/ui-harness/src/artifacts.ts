import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import type { Page } from "playwright";

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

export { pad };

export async function waitReady(page: Page, timeoutMs = 15_000): Promise<void> {
  await page.waitForFunction(
    () =>
      document.documentElement.dataset.gewuReady === "true" &&
      !document.querySelector("[data-gewu-pending]"),
    undefined,
    { timeout: timeoutMs },
  );
}

/**
 * Whether anything is below the fold: the document, or a scrolling container
 * inside it such as the app's main column. Containers that scroll are
 * unrolled (marked `data-gewu-unrolled`) so a full-page shot shows them whole;
 * `restoreScrollers` puts them back.
 */
async function pageScrolls(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    let scrolls = document.documentElement.scrollHeight > window.innerHeight + 2;
    for (const element of Array.from(document.querySelectorAll<HTMLElement>("body *"))) {
      const style = getComputedStyle(element);
      const scroller = /(auto|scroll)/.test(style.overflowY);
      if (scroller && element.scrollHeight > element.clientHeight + 2) {
        element.dataset.gewuUnrolled = element.getAttribute("style") ?? "";
        element.style.overflow = "visible";
        element.style.height = "auto";
        element.style.maxHeight = "none";
        scrolls = true;
      }
    }
    if (scrolls) {
      for (const element of [document.documentElement, document.body, document.getElementById("root")]) {
        if (element) {
          element.dataset.gewuUnrolled = element.getAttribute("style") ?? "";
          element.style.height = "auto";
          element.style.overflow = "visible";
        }
      }
    }
    return scrolls;
  });
}

async function restoreScrollers(page: Page): Promise<void> {
  await page.evaluate(() => {
    for (const element of Array.from(document.querySelectorAll<HTMLElement>("[data-gewu-unrolled]"))) {
      const style = element.dataset.gewuUnrolled ?? "";
      if (style) element.setAttribute("style", style);
      else element.removeAttribute("style");
      delete element.dataset.gewuUnrolled;
    }
  });
}

export async function writePageArtifacts(
  page: Page,
  outDir: string,
  vpName: string,
  index: number,
  pageId: string,
): Promise<{ pngRel: string; ariaRel: string; fullRel: string | null }> {
  const base = `${pad(index)}-${pageId}`;
  const pngRel = `pages/${vpName}/${base}.png`;
  const ariaRel = `pages/${vpName}/${base}.aria.yaml`;
  const vpDir = path.join(outDir, "pages", vpName);
  await mkdir(vpDir, { recursive: true });

  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  await page.screenshot({
    path: path.join(outDir, pngRel),
    fullPage: false,
    animations: "disabled",
    timeout: 60_000,
  });

  let fullRel: string | null = null;
  if (await pageScrolls(page)) {
    fullRel = `pages/${vpName}/${base}-full.png`;
    await page.screenshot({
      path: path.join(outDir, fullRel),
      fullPage: true,
      animations: "disabled",
      timeout: 60_000,
    });
  }
  await restoreScrollers(page);

  const aria = await page.locator("body").ariaSnapshot();
  await writeFile(path.join(outDir, ariaRel), aria, "utf8");
  return { pngRel, ariaRel, fullRel };
}

export async function writeInteractionShot(
  page: Page,
  outDir: string,
  vpName: string,
  index: number,
  name: string,
): Promise<string> {
  const shotName = `${pad(index)}-interaction-${name}.png`;
  const shotRel = `pages/${vpName}/${shotName}`;
  await page.screenshot({
    path: path.join(outDir, shotRel),
    fullPage: false,
    animations: "disabled",
    timeout: 60_000,
  });
  return shotRel;
}
