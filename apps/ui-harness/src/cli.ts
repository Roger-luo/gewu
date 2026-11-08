#!/usr/bin/env tsx
import { writeBrief } from "./brief.js";
import { capture } from "./capture.js";
import { runJourneys } from "./journeys/index.js";
import { rank } from "./rank.js";
import { formatScoreSummary, score } from "./score.js";
import { parseViewports } from "./viewport.js";

function usage(): string {
  return `gewu-ui — capture, check, brief, score, and rank the Gewu desktop UI

Usage:
  gewu-ui capture --url <url> --out <runDir> [options]
  gewu-ui brief --run <runDir> [--judge-id <id>] [--rubric <file>]
  gewu-ui score --run <runDir> [--rubric <file>]
  gewu-ui rank --runs <dirA> <dirB> ... [--out <file.md>]
  gewu-ui journeys --out <runDir> [--gewu <bin>] [--static <dir>] [--only 1,2]
                   [--viewport 1440x900] [--color-scheme light|dark]

capture options:
  --viewports <list>     e.g. 1440x900,1024x700 (default 1440x900)
  --label <text>         label stored in manifest
  --start "<cmd>"        shell command to spawn before capture
  --wait-url <url>       URL to wait for after --start (default: --url)
  --color-scheme <mode>  light | dark | both (default light)
  --rubric <file>        rubric JSON override
`;
}

function flagValue(args: string[], name: string): string | undefined {
  const i = args.indexOf(name);
  if (i === -1) return undefined;
  const v = args[i + 1];
  if (!v || v.startsWith("--")) throw new Error(`Missing value for ${name}`);
  return v;
}

function hasFlag(args: string[], name: string): boolean {
  return args.includes(name);
}

function restAfter(args: string[], name: string): string[] {
  const i = args.indexOf(name);
  if (i === -1) return [];
  const out: string[] = [];
  for (let j = i + 1; j < args.length; j++) {
    const a = args[j]!;
    if (a.startsWith("--")) break;
    out.push(a);
  }
  return out;
}

async function main(argv: string[]): Promise<void> {
  const [cmd, ...args] = argv;
  if (!cmd || cmd === "-h" || cmd === "--help") {
    console.log(usage());
    return;
  }

  switch (cmd) {
    case "capture": {
      const url = flagValue(args, "--url");
      const out = flagValue(args, "--out");
      if (!url || !out) throw new Error("capture requires --url and --out");
      const viewports = flagValue(args, "--viewports");
      const label = flagValue(args, "--label");
      const start = flagValue(args, "--start");
      const waitUrl = flagValue(args, "--wait-url");
      const schemeRaw = flagValue(args, "--color-scheme");
      const rubricPath = flagValue(args, "--rubric");
      const colorScheme =
        schemeRaw === "light" || schemeRaw === "dark" || schemeRaw === "both"
          ? schemeRaw
          : undefined;
      const result = await capture({
        url,
        out,
        ...(viewports !== undefined ? { viewports } : {}),
        ...(label !== undefined ? { label } : {}),
        ...(start !== undefined ? { start } : {}),
        ...(waitUrl !== undefined ? { waitUrl } : {}),
        ...(colorScheme !== undefined ? { colorScheme } : {}),
        ...(rubricPath !== undefined ? { rubricPath } : {}),
      });
      console.log(`Captured ${result.manifest.pages.length} pages → ${result.out}`);
      console.log(`checks: ${result.checksPath}`);
      break;
    }
    case "brief": {
      const run = flagValue(args, "--run");
      if (!run) throw new Error("brief requires --run");
      const judgeId = flagValue(args, "--judge-id");
      const rubricPath = flagValue(args, "--rubric");
      const briefPath = await writeBrief({
        run,
        ...(judgeId !== undefined ? { judgeId } : {}),
        ...(rubricPath !== undefined ? { rubricPath } : {}),
      });
      console.log(`Wrote ${briefPath}`);
      break;
    }
    case "score": {
      const run = flagValue(args, "--run");
      if (!run) throw new Error("score requires --run");
      const rubricPath = flagValue(args, "--rubric");
      const scorecard = await score({
        run,
        ...(rubricPath !== undefined ? { rubricPath } : {}),
      });
      console.log(formatScoreSummary(scorecard));
      console.log(`Wrote ${pathJoin(run, "scorecard.json")}`);
      break;
    }
    case "rank": {
      const runs = restAfter(args, "--runs");
      if (runs.length === 0 && hasFlag(args, "--runs")) {
        throw new Error("rank requires at least one directory after --runs");
      }
      if (runs.length === 0) throw new Error("rank requires --runs <dir>...");
      const out = flagValue(args, "--out");
      const md = await rank({ runs, ...(out !== undefined ? { out } : {}) });
      console.log(md);
      break;
    }
    case "journeys": {
      const out = flagValue(args, "--out");
      if (!out) throw new Error("journeys requires --out");
      const gewu = flagValue(args, "--gewu");
      const staticDir = flagValue(args, "--static");
      const only = flagValue(args, "--only");
      const vp = flagValue(args, "--viewport");
      const scheme = flagValue(args, "--color-scheme");
      const viewport = vp ? parseViewports(vp)[0] : undefined;
      const result = await runJourneys({
        out,
        ...(gewu !== undefined ? { gewu } : {}),
        ...(staticDir !== undefined ? { staticDir } : {}),
        ...(only !== undefined ? { only: only.split(",").map(Number) } : {}),
        ...(viewport ? { viewport: { width: viewport.width, height: viewport.height } } : {}),
        ...(scheme === "dark" || scheme === "light" ? { colorScheme: scheme } : {}),
      });
      for (const journey of result.journeys) {
        const mark = journey.blocked ? "BLOCKED" : "ok";
        console.log(`${mark.padEnd(8)}${journey.story}. ${journey.title} (${journey.steps.length} steps)`);
        if (journey.blocked) console.log(`        ${journey.blocked.split("\n")[0]}`);
      }
      console.log(`Wrote ${pathJoin(out, "journeys.json")}`);
      if (result.blocked > 0) process.exitCode = 1;
      break;
    }
    default:
      throw new Error(`Unknown command "${cmd}"\n\n${usage()}`);
  }
}

function pathJoin(a: string, b: string): string {
  return `${a.replace(/\/$/, "")}/${b}`;
}

main(process.argv.slice(2)).catch((err: unknown) => {
  console.error(err instanceof Error ? err.message : err);
  process.exitCode = 1;
});
