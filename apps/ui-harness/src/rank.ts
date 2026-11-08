import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import type { JudgeFinding, Scorecard } from "./types.js";

export interface RankOptions {
  runs: string[];
  out?: string;
}

interface RunRow {
  dir: string;
  name: string;
  scorecard: Scorecard;
  findings: JudgeFinding[];
}

async function loadRun(dir: string): Promise<RunRow> {
  const abs = path.resolve(dir);
  const scorecard = JSON.parse(
    await readFile(path.join(abs, "scorecard.json"), "utf8"),
  ) as Scorecard;
  const findings = scorecard.top_findings ?? [];
  return { dir: abs, name: path.basename(abs), scorecard, findings };
}

function tokenSet(s: string): Set<string> {
  return new Set(
    s
      .toLowerCase()
      .replace(/[^a-z0-9\s]/g, " ")
      .split(/\s+/)
      .filter((t) => t.length > 2),
  );
}

function similar(a: JudgeFinding, b: JudgeFinding): boolean {
  if (a.dimension === b.dimension && a.page === b.page) {
    const ta = tokenSet(a.description);
    const tb = tokenSet(b.description);
    if (ta.size === 0 || tb.size === 0) return a.description === b.description;
    let inter = 0;
    for (const t of ta) if (tb.has(t)) inter++;
    const union = new Set([...ta, ...tb]).size;
    return inter / union >= 0.4;
  }
  if (a.dimension === b.dimension) {
    const ta = tokenSet(a.description);
    const tb = tokenSet(b.description);
    let inter = 0;
    for (const t of ta) if (tb.has(t)) inter++;
    return inter >= 3;
  }
  return false;
}

export async function rank(opts: RankOptions): Promise<string> {
  if (opts.runs.length < 1) throw new Error("rank requires at least one run");
  const rows = await Promise.all(opts.runs.map(loadRun));
  const baseline = rows[0]!;

  const ordered = [...rows].sort((a, b) => b.scorecard.total - a.scorecard.total);

  const lines: string[] = [];
  lines.push(`# UI harness rank`, ``);
  lines.push(`Baseline: \`${baseline.name}\``, ``);
  lines.push(`## Totals`, ``);
  lines.push(`| Rank | Run | Total | Gates |`);
  lines.push(`| --- | --- | --- | --- |`);
  ordered.forEach((r, i) => {
    const gates =
      r.scorecard.gates_failed.length === 0
        ? "pass"
        : `FAIL: ${r.scorecard.gates_failed.join(", ")}`;
    lines.push(`| ${i + 1} | ${r.name} | ${r.scorecard.total} | ${gates} |`);
  });
  lines.push(``);

  const dimIds = baseline.scorecard.dimensions.map((d) => d.id);
  lines.push(`## Dimension deltas vs ${baseline.name}`, ``);
  lines.push(`| Dimension | ${rows.map((r) => r.name).join(" | ")} |`);
  lines.push(`| --- | ${rows.map(() => "---").join(" | ")} |`);
  for (const id of dimIds) {
    const baseMean = baseline.scorecard.dimensions.find((d) => d.id === id)?.mean ?? 0;
    const cells = rows.map((r) => {
      const mean = r.scorecard.dimensions.find((d) => d.id === id)?.mean ?? 0;
      if (r === baseline) return mean.toFixed(2);
      const delta = mean - baseMean;
      const sign = delta > 0 ? "+" : "";
      return `${mean.toFixed(2)} (${sign}${delta.toFixed(2)})`;
    });
    lines.push(`| ${id} | ${cells.join(" | ")} |`);
  }
  lines.push(``);

  for (const r of rows.slice(1)) {
    lines.push(`## Findings: ${r.name} vs ${baseline.name}`, ``);
    const fixed: JudgeFinding[] = [];
    const retained: JudgeFinding[] = [];
    for (const f of baseline.findings) {
      if (r.findings.some((g) => similar(f, g))) retained.push(f);
      else fixed.push(f);
    }
    const added = r.findings.filter(
      (g) => !baseline.findings.some((f) => similar(f, g)),
    );
    lines.push(`### Fixed (${fixed.length})`);
    for (const f of fixed) lines.push(`- [${f.severity}] ${f.dimension}: ${f.description}`);
    if (!fixed.length) lines.push(`- (none)`);
    lines.push(``, `### New (${added.length})`);
    for (const f of added) lines.push(`- [${f.severity}] ${f.dimension}: ${f.description}`);
    if (!added.length) lines.push(`- (none)`);
    lines.push(``, `### Still open (${retained.length})`);
    for (const f of retained) lines.push(`- [${f.severity}] ${f.dimension}: ${f.description}`);
    if (!retained.length) lines.push(`- (none)`);
    lines.push(``);
  }

  const md = lines.join("\n");
  if (opts.out) {
    await writeFile(path.resolve(opts.out), md, "utf8");
  }
  return md;
}
