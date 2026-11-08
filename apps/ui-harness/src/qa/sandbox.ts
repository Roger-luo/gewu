import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright";
import { createFixture } from "../journeys/fixture.js";
import { packageRoot } from "../rubric.js";
import { freePort, killProcessTree, waitForUrl } from "../wait.js";

/** What a QA agent needs to drive the sandbox, written to `<dir>/sandbox.json`. */
export interface SandboxFile {
  url: string;
  /** Chrome DevTools endpoint the `act` command attaches to. */
  cdp: string;
  repositories: { shop: string; notes: string } & Record<string, string>;
  /** Custom ACP commands: one that writes batches, one that fails. */
  agents: { batches: string; slow_batches: string; failing: string };
  pid: number;
}

export interface SandboxOptions {
  dir: string;
  gewu?: string;
  staticDir?: string;
  /** Extra repositories to offer, `name=path`. */
  repositories?: Record<string, string>;
  viewport?: { width: number; height: number };
}

/**
 * Serve the real app against throwaway repositories and keep one browser open,
 * so a QA agent can act as a user one step at a time. Runs until interrupted.
 */
export async function runSandbox(opts: SandboxOptions): Promise<void> {
  const repoRoot = path.resolve(packageRoot(), "../..");
  const gewu = path.resolve(opts.gewu ?? path.join(repoRoot, "target/debug/gewu"));
  const staticDir = path.resolve(opts.staticDir ?? path.join(repoRoot, "apps/desktop/dist"));
  const dir = path.resolve(opts.dir);
  const viewport = opts.viewport ?? { width: 1440, height: 900 };
  await mkdir(path.join(dir, "shots"), { recursive: true });

  const fixture = await createFixture(repoRoot);
  const port = await freePort();
  const cdpPort = await freePort();
  const url = `http://127.0.0.1:${port}/`;
  const server = spawn(gewu, ["ui", "serve", "--static", staticDir, "--port", String(port)], {
    env: { ...process.env, ...fixture.env },
    stdio: ["ignore", "pipe", "pipe"],
    detached: true,
  });
  let serverLog = "";
  server.stdout?.on("data", (chunk: Buffer) => (serverLog += chunk.toString()));
  server.stderr?.on("data", (chunk: Buffer) => (serverLog += chunk.toString()));

  const context = await chromium.launchPersistentContext(path.join(fixture.root, "browser"), {
    headless: true,
    viewport,
    reducedMotion: "reduce",
    args: [`--remote-debugging-port=${cdpPort}`],
  });
  const stop = async () => {
    await context.close().catch(() => undefined);
    await killProcessTree(server);
    await writeFile(path.join(dir, "server.log"), serverLog, "utf8");
    await fixture.dispose();
  };
  try {
    await waitForUrl(`${url}api/health`, { timeoutMs: 30_000 });
    const page = context.pages()[0] ?? (await context.newPage());
    await page.goto(url, { waitUntil: "domcontentloaded" });
    const file: SandboxFile = {
      url,
      cdp: `http://127.0.0.1:${cdpPort}`,
      repositories: { shop: fixture.shop, notes: fixture.notes, ...opts.repositories },
      agents: {
        batches: fixture.agent("harness", 0.4),
        slow_batches: fixture.agent("harness", 3),
        failing: fixture.agent("fail"),
      },
      pid: process.pid,
    };
    await writeFile(path.join(dir, "sandbox.json"), JSON.stringify(file, null, 2) + "\n", "utf8");
    console.log(`Sandbox ready at ${url}; drive it with: gewu-ui act --sandbox ${dir} look`);
  } catch (error) {
    await stop();
    throw error;
  }
  await new Promise<void>((resolve) => {
    process.once("SIGINT", resolve);
    process.once("SIGTERM", resolve);
  });
  await stop();
}
