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

async function pageScrolls(page: Page): Promise<boolean> {
  return page.evaluate(
    () => document.documentElement.scrollHeight > window.innerHeight + 2,
  );
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
