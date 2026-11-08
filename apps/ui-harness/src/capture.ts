import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";
import { waitReady, writeInteractionShot, writePageArtifacts } from "./artifacts.js";
import {
  aggregateChecks,
  attachConsoleCollector,
  runPageChecks,
} from "./checks/index.js";
import { gitHeadAndDirty } from "./git.js";
import { findLatestRubricPath, loadRubric, packageRoot } from "./rubric.js";
import type { CheckFinding, Manifest, PageInfo } from "./types.js";
import { parseViewports, viewportDirName } from "./viewport.js";
import { killProcessTree, spawnShell, waitForUrl } from "./wait.js";

export interface CaptureOptions {
  url: string;
  out: string;
  viewports?: string;
  label?: string;
  start?: string;
  waitUrl?: string;
  colorScheme?: "light" | "dark" | "both";
  repoRoot?: string;
  rubricPath?: string;
}

export async function capture(opts: CaptureOptions): Promise<{
  out: string;
  manifest: Manifest;
  checksPath: string;
}> {
  const viewports = parseViewports(opts.viewports ?? "1440x900");
  const schemes: ("light" | "dark")[] =
    opts.colorScheme === "both"
      ? ["light", "dark"]
      : opts.colorScheme === "dark"
        ? ["dark"]
        : ["light"];

  const outDir = path.resolve(opts.out);
  await mkdir(path.join(outDir, "pages"), { recursive: true });

  let child = opts.start ? spawnShell(opts.start) : null;
  try {
    if (opts.start) await waitForUrl(opts.waitUrl ?? opts.url);

    const repoRoot = opts.repoRoot ?? path.resolve(packageRoot(), "../..");
    const git = await gitHeadAndDirty(repoRoot);

    let rubricVersion: number | null = null;
    try {
      rubricVersion = (await loadRubric(opts.rubricPath)).version;
    } catch {
      const latest = await findLatestRubricPath();
      if (latest) {
        try {
          rubricVersion = (await loadRubric(latest)).version;
        } catch {
          rubricVersion = null;
        }
      }
    }

    const browser = await chromium.launch({
      headless: true,
      args: ["--disable-font-subpixel-positioning"],
    });
    const findings: CheckFinding[] = [];
    const interactions: string[] = [];
    let pagesList: PageInfo[] = [];

    try {
      const context = await browser.newContext({
        colorScheme: schemes[0] ?? "light",
        reducedMotion: "reduce",
      });
      const page = await context.newPage();
      const consoleCollector = attachConsoleCollector(page);

      await page.setViewportSize({
        width: viewports[0]!.width,
        height: viewports[0]!.height,
      });
      await page.goto(opts.url, { waitUntil: "domcontentloaded" });
      await waitReady(page);

      pagesList = await page.evaluate(() => {
        const api = window.__gewu;
        if (!api) throw new Error("window.__gewu missing after ready");
        return api.pages();
      });
      if (pagesList.length === 0) {
        throw new Error("window.__gewu.pages() returned no pages");
      }

      let keyboardDone = false;

      for (const scheme of schemes) {
        await page.emulateMedia({ colorScheme: scheme });
        for (const vp of viewports) {
          const vpName = viewportDirName(vp, scheme);
          await page.setViewportSize({ width: vp.width, height: vp.height });

          for (let i = 0; i < pagesList.length; i++) {
            const info = pagesList[i]!;
            consoleCollector.setContext(info.id, vpName);
            await page.evaluate(async (target) => {
              await window.__gewu!.goto(target);
            }, info.target);
            await waitReady(page);

            const { pngRel } = await writePageArtifacts(
              page,
              outDir,
              vpName,
              i,
              info.id,
            );

            const includeKeyboard =
              !keyboardDone && i === 0 && scheme === schemes[0];
            const { findings: pageFindings, interactionShots } = await runPageChecks(
              page,
              { pageId: info.id, viewport: vpName, expected: info },
              { includeKeyboard },
            );
            if (includeKeyboard) keyboardDone = true;

            for (const f of pageFindings) {
              f.evidence = pngRel;
              findings.push(f);
            }

            if (includeKeyboard) {
              let shotIdx = pagesList.length;
              for (const shot of interactionShots) {
                await page.evaluate(async (target) => {
                  await window.__gewu!.goto(target);
                }, shot.target ?? info.target);
                await waitReady(page);
                if (shot.name === "help-open") {
                  await page.keyboard.press("?");
                  await page.waitForTimeout(150);
                } else if (shot.name === "feedback-open") {
                  await page.keyboard.press("c");
                  await page.waitForTimeout(150);
                }
                interactions.push(
                  await writeInteractionShot(page, outDir, vpName, shotIdx, shot.name),
                );
                shotIdx += 1;
                await page.keyboard.press("Escape");
                await page.waitForTimeout(100);
              }
            }
          }
        }
      }

      findings.push(...consoleCollector.findings());
      consoleCollector.dispose();
      await context.close();
    } finally {
      await browser.close();
    }

    const checks = aggregateChecks(findings);
    const checksPath = path.join(outDir, "checks.json");
    await writeFile(checksPath, JSON.stringify(checks, null, 2) + "\n", "utf8");

    const manifest: Manifest = {
      url: opts.url,
      label: opts.label ?? null,
      captured_at: new Date().toISOString(),
      git,
      viewports: viewports.map((v) => v.label),
      color_schemes: schemes,
      pages: pagesList,
      rubric_version: rubricVersion,
      interactions,
    };
    await writeFile(
      path.join(outDir, "manifest.json"),
      JSON.stringify(manifest, null, 2) + "\n",
      "utf8",
    );

    return { out: outDir, manifest, checksPath };
  } finally {
    if (child) {
      await killProcessTree(child);
      child = null;
    }
  }
}
