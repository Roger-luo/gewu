import { access, readFile, readdir } from "node:fs/promises";
import path from "node:path";
import type { JudgeFinding, JudgeOutput, Rubric } from "./types.js";

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

async function exists(p: string): Promise<boolean> {
  try {
    await access(p);
    return true;
  } catch {
    return false;
  }
}

export function parseJudgeOutput(raw: unknown, source: string): JudgeOutput {
  if (!isRecord(raw)) throw new Error(`${source}: root must be object`);
  if (typeof raw.rubric_version !== "number") {
    throw new Error(`${source}: rubric_version required`);
  }
  if (typeof raw.judge !== "string") throw new Error(`${source}: judge required`);
  if (!Array.isArray(raw.dimensions)) throw new Error(`${source}: dimensions required`);
  if (!Array.isArray(raw.findings)) throw new Error(`${source}: findings required`);

  const dimensions = raw.dimensions.map((d, i) => {
    const ctx = `${source}.dimensions[${i}]`;
    if (!isRecord(d)) throw new Error(`${ctx}: must be object`);
    if (typeof d.id !== "string") throw new Error(`${ctx}: id required`);
    if (typeof d.score !== "number") throw new Error(`${ctx}: score required`);
    if (typeof d.rationale !== "string") throw new Error(`${ctx}: rationale required`);
    if (!Array.isArray(d.evidence)) throw new Error(`${ctx}: evidence required`);
    const evidence = d.evidence.map((e, j) => {
      if (!isRecord(e) || typeof e.file !== "string" || typeof e.note !== "string") {
        throw new Error(`${ctx}.evidence[${j}]: file and note required`);
      }
      return { file: e.file, note: e.note };
    });
    return { id: d.id, score: d.score, rationale: d.rationale, evidence };
  });

  const findings: JudgeFinding[] = raw.findings.map((f, i) => {
    const ctx = `${source}.findings[${i}]`;
    if (!isRecord(f)) throw new Error(`${ctx}: must be object`);
    const severity = f.severity;
    if (severity !== "blocker" && severity !== "major" && severity !== "minor") {
      throw new Error(`${ctx}: severity must be blocker|major|minor`);
    }
    return {
      severity,
      dimension: String(f.dimension ?? ""),
      page: String(f.page ?? ""),
      description: String(f.description ?? ""),
      suggestion: String(f.suggestion ?? ""),
    };
  });

  return {
    rubric_version: raw.rubric_version,
    judge: raw.judge,
    dimensions,
    findings,
  };
}

export async function loadJudges(
  runDir: string,
): Promise<{ file: string; judge: JudgeOutput }[]> {
  const judgeDir = path.join(runDir, "judge");
  let names: string[];
  try {
    names = await readdir(judgeDir);
  } catch {
    return [];
  }
  const out: { file: string; judge: JudgeOutput }[] = [];
  for (const name of names.filter((n) => n.endsWith(".json"))) {
    const file = path.join(judgeDir, name);
    const raw = JSON.parse(await readFile(file, "utf8")) as unknown;
    out.push({ file: name, judge: parseJudgeOutput(raw, name) });
  }
  return out;
}

export async function validateJudgeAgainstRubric(
  judge: JudgeOutput,
  rubric: Rubric,
  runDir: string,
): Promise<{ scores: Map<string, number>; issues: string[] }> {
  const issues: string[] = [];
  const scores = new Map<string, number>();

  if (judge.rubric_version !== rubric.version) {
    issues.push(
      `rubric_version ${judge.rubric_version} != loaded rubric ${rubric.version}`,
    );
  }

  const byId = new Map(judge.dimensions.map((d) => [d.id, d]));
  for (const dim of rubric.dimensions) {
    const entry = byId.get(dim.id);
    if (!entry) {
      issues.push(`missing dimension ${dim.id}`);
      scores.set(dim.id, 0);
      continue;
    }
    let score = entry.score;
    if (score < rubric.scale.min || score > rubric.scale.max) {
      issues.push(`${dim.id}: score ${score} out of scale`);
      score = 0;
    }
    let valid = 0;
    for (const ev of entry.evidence) {
      if (await exists(path.join(runDir, ev.file))) valid += 1;
      else issues.push(`${dim.id}: evidence missing file ${ev.file}`);
    }
    if (valid === 0) {
      issues.push(`${dim.id}: no valid evidence; score counts as 0`);
      score = 0;
    }
    scores.set(dim.id, score);
  }
  return { scores, issues };
}
