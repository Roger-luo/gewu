import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import path from "node:path";
import { chromium } from "playwright";
import { aggregateChecks, attachConsoleCollector } from "../checks/index.js";
import { gitHeadAndDirty } from "../git.js";
import { packageRoot } from "../rubric.js";
import type { CheckFinding } from "../types.js";
import { killProcessTree, waitForUrl } from "../wait.js";
import { createFixture } from "./fixture.js";
import { Journey, type JourneyRecord } from "./runner.js";
import { STORIES } from "./stories.js";

export interface JourneyOptions {
  out: string;
  /** The `gewu` binary; default `target/debug/gewu`. */
  gewu?: string;
  /** The built web UI; default `apps/desktop/dist`. */
  staticDir?: string;
  /** Story numbers to run; default all. Later stories build on earlier ones. */
  only?: number[];
  viewport?: { width: number; height: number };
  colorScheme?: "light" | "dark";
}

export interface JourneysFile {
  captured_at: string;
  git: { head: string | null; dirty: boolean };
  viewport: string;
  color_scheme: "light" | "dark";
  journeys: JourneyRecord[];
  blocked: number;
}

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => (typeof address === "object" && address ? resolve(address.port) : reject(new Error("no port"))));
    });
  });
}

/** Run the user-story journeys against a real `gewu ui serve` and a throwaway fixture. */
export async function runJourneys(opts: JourneyOptions): Promise<JourneysFile> {
  const repoRoot = path.resolve(packageRoot(), "../..");
  const gewu = path.resolve(opts.gewu ?? path.join(repoRoot, "target/debug/gewu"));
  const staticDir = path.resolve(opts.staticDir ?? path.join(repoRoot, "apps/desktop/dist"));
  const outDir = path.resolve(opts.out);
  const viewport = opts.viewport ?? { width: 1440, height: 900 };
  const scheme = opts.colorScheme ?? "light";
  const viewportName = `${viewport.width}x${viewport.height}${scheme === "dark" ? "-dark" : ""}`;
  await mkdir(outDir, { recursive: true });

  const fixture = await createFixture(repoRoot);
  const port = await freePort();
  const url = `http://127.0.0.1:${port}/`;
  const server = spawn(gewu, ["ui", "serve", "--static", staticDir, "--port", String(port)], {
    env: { ...process.env, ...fixture.env },
    stdio: ["ignore", "pipe", "pipe"],
    detached: true,
  });
  let serverLog = "";
  server.stdout?.on("data", (chunk: Buffer) => (serverLog += chunk.toString()));
  server.stderr?.on("data", (chunk: Buffer) => (serverLog += chunk.toString()));

  const findings: CheckFinding[] = [];
  const journeys: JourneyRecord[] = [];
  try {
    await waitForUrl(`${url}api/health`, { timeoutMs: 30_000 });
    const browser = await chromium.launch({ headless: true });
    try {
      const context = await browser.newContext({ colorScheme: scheme, reducedMotion: "reduce", viewport });
      const page = await context.newPage();
      const console = attachConsoleCollector(page);
      await page.goto(url, { waitUntil: "domcontentloaded" });
      for (const story of STORIES) {
        if (opts.only && !opts.only.includes(story.meta.story)) {
          continue;
        }
        console.setContext(story.meta.id, viewportName);
        const journey = new Journey(page, outDir, viewportName, story.meta);
        await story.run(journey, fixture);
        if (journey.blocked) {
          findings.push({
            key: "journey.blocked",
            page: story.meta.id,
            viewport: viewportName,
            selector: "",
            description: journey.record.blocked ?? "",
            evidence: journey.record.steps.at(-1)?.shot ?? "",
          });
        }
        findings.push(...journey.findings);
        journeys.push(journey.record);
        if (journey.blocked) {
          // Later stories assume this one's end state; start them from Home.
          await page.goto(url, { waitUntil: "domcontentloaded" });
        }
      }
      findings.push(...console.findings());
      console.dispose();
      await context.close();
    } finally {
      await browser.close();
    }
  } finally {
    await killProcessTree(server);
    await writeFile(path.join(outDir, "server.log"), serverLog, "utf8");
    await fixture.dispose();
  }

  const result: JourneysFile = {
    captured_at: new Date().toISOString(),
    git: await gitHeadAndDirty(repoRoot),
    viewport: viewportName,
    color_scheme: scheme,
    journeys,
    blocked: journeys.filter((journey) => journey.blocked).length,
  };
  await writeFile(path.join(outDir, "journeys.json"), JSON.stringify(result, null, 2) + "\n", "utf8");
  await writeFile(path.join(outDir, "journey-checks.json"), JSON.stringify(aggregateChecks(findings), null, 2) + "\n", "utf8");
  return result;
}
