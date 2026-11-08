import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { loadJudges, validateJudgeAgainstRubric } from "./judge.js";
import { loadRubric } from "./rubric.js";
import type {
  CheckKey,
  ChecksFile,
  JudgeFinding,
  Rubric,
  Scorecard,
  ScorecardDimension,
  ScorecardGateResult,
} from "./types.js";

export interface ScoreOptions {
  run: string;
  rubricPath?: string;
}

function applyGates(
  rubric: Rubric,
  checks: ChecksFile,
): { gates: ScorecardGateResult[]; failed: string[]; cap: number | null } {
  const gates: ScorecardGateResult[] = [];
  const failed: string[] = [];
  let cap: number | null = null;

  for (const g of rubric.gates) {
    const count = checks.counters[g.check as CheckKey] ?? 0;
    const passed = count <= g.max;
    gates.push({
      id: g.id,
      check: g.check,
      count,
      max: g.max,
      effect: g.effect,
      passed,
      description: g.description,
    });
    if (!passed) {
      if (g.effect === "fail") failed.push(g.id);
      if (g.effect === "cap" && typeof g.cap === "number") {
        cap = cap === null ? g.cap : Math.min(cap, g.cap);
      }
    }
  }
  return { gates, failed, cap };
}

async function readChecks(file: string): Promise<ChecksFile | null> {
  try {
    return JSON.parse(await readFile(file, "utf8")) as ChecksFile;
  } catch {
    return null;
  }
}

/**
 * Page checks plus journey checks. A rubric that gates on journeys counts a
 * run without `journeys.json` as blocked, so skipping them cannot pass.
 */
export async function loadChecks(runDir: string, rubric: Rubric): Promise<ChecksFile> {
  const pages = await readChecks(path.join(runDir, "checks.json"));
  const journeys = await readChecks(path.join(runDir, "journey-checks.json"));
  if (!pages && !journeys) {
    throw new Error(`No checks.json or journey-checks.json in ${runDir}`);
  }
  const merged: ChecksFile = {
    counters: { ...(pages ?? journeys)!.counters },
    findings: [...(pages?.findings ?? []), ...(journeys?.findings ?? [])],
  };
  if (pages && journeys) {
    for (const [key, count] of Object.entries(journeys.counters)) {
      const k = key as CheckKey;
      merged.counters[k] = (merged.counters[k] ?? 0) + count;
    }
  }
  const gatesJourneys = rubric.gates.some((gate) => gate.check === "journey.blocked");
  if (gatesJourneys && !journeys) {
    merged.counters["journey.blocked"] = Math.max(1, merged.counters["journey.blocked"] ?? 0);
  }
  return merged;
}

export async function score(opts: ScoreOptions): Promise<Scorecard> {
  const runDir = path.resolve(opts.run);
  const rubric = await loadRubric(opts.rubricPath);
  const checks = await loadChecks(runDir, rubric);

  const loaded = await loadJudges(runDir);
  if (loaded.length === 0) {
    throw new Error(`No judge JSON files in ${path.join(runDir, "judge")}`);
  }

  const validation: Scorecard["validation"] = [];
  const perDimScores = new Map<string, number[]>();
  for (const dim of rubric.dimensions) perDimScores.set(dim.id, []);

  const allFindings: JudgeFinding[] = [];

  for (const { judge } of loaded) {
    const { scores, issues } = await validateJudgeAgainstRubric(judge, rubric, runDir);
    validation.push({ judge: judge.judge, issues });
    for (const dim of rubric.dimensions) {
      perDimScores.get(dim.id)!.push(scores.get(dim.id) ?? 0);
    }
    allFindings.push(...judge.findings);
  }

  const dimensions: ScorecardDimension[] = rubric.dimensions.map((dim) => {
    const scores = perDimScores.get(dim.id)!;
    const mean = scores.reduce((a, b) => a + b, 0) / scores.length;
    const disagreement = Math.max(...scores) - Math.min(...scores) >= 2;
    return { id: dim.id, mean, scores, disagreement, weight: dim.weight };
  });

  const weightSum = rubric.dimensions.reduce((a, d) => a + d.weight, 0);
  const scaleMax = rubric.scale.max;
  const raw =
    weightSum === 0
      ? 0
      : dimensions.reduce((a, d) => a + d.mean * d.weight, 0) / weightSum;
  let total = (raw / scaleMax) * 100;

  const { gates, failed, cap } = applyGates(rubric, checks);
  const capped_total = cap !== null ? Math.min(total, cap) : null;
  if (capped_total !== null) total = capped_total;

  const severityRank = { blocker: 0, major: 1, minor: 2 };
  const top_findings = [...allFindings]
    .sort((a, b) => severityRank[a.severity] - severityRank[b.severity])
    .slice(0, 10);

  const scorecard: Scorecard = {
    rubric_version: rubric.version,
    judges: loaded.map((l) => l.judge.judge),
    total: Math.round(total * 10) / 10,
    capped_total,
    gates_failed: failed,
    dimensions,
    gates,
    validation,
    top_findings,
  };

  await writeFile(
    path.join(runDir, "scorecard.json"),
    JSON.stringify(scorecard, null, 2) + "\n",
    "utf8",
  );
  return scorecard;
}

export function formatScoreSummary(scorecard: Scorecard): string {
  const lines: string[] = [];
  lines.push(`Total: ${scorecard.total}/100`);
  if (scorecard.gates_failed.length) {
    lines.push(`Gates failed: ${scorecard.gates_failed.join(", ")}`);
  } else {
    lines.push(`Gates: all passed`);
  }
  const weakest = [...scorecard.dimensions].sort((a, b) => a.mean - b.mean).slice(0, 3);
  lines.push(
    `Weakest: ${weakest.map((d) => `${d.id}=${d.mean.toFixed(2)}${d.disagreement ? "*" : ""}`).join(", ")}`,
  );
  if (scorecard.top_findings.length) {
    lines.push(`Top findings:`);
    for (const f of scorecard.top_findings.slice(0, 5)) {
      lines.push(`  [${f.severity}] ${f.dimension}: ${f.description}`);
    }
  }
  for (const v of scorecard.validation) {
    if (v.issues.length) {
      lines.push(`Judge ${v.judge} validation: ${v.issues.length} issue(s)`);
    }
  }
  return lines.join("\n");
}
