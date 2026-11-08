import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { loadRubric, packageRoot, renderRubricText } from "./rubric.js";
import type { JourneysFile } from "./journeys/index.js";
import type { Manifest, Rubric } from "./types.js";

/** Each story's goal and steps, so the judge can follow the screenshots in order. */
function journeyLines(file: JourneysFile | null): string[] {
  if (!file) {
    return ["No journeys.json in this run: score every journey dimension 0 and cite `manifest.json`."];
  }
  const lines = [`Viewport ${file.viewport}. Stories run in order against one workbench; later stories see earlier stories' reviews.`, ``];
  for (const journey of file.journeys) {
    lines.push(`### ${journey.story}. ${journey.title}${journey.blocked ? " — BLOCKED" : ""}`, ``, `Goal: ${journey.goal}`, ``);
    for (const step of journey.steps) {
      lines.push(`${step.index}. ${step.ok ? "" : "**failed** "}${step.name} → \`${step.shot}\` (screen: ${step.screen ?? "none"})${step.error ? ` — ${step.error.split("\n")[0]}` : ""}`);
    }
    lines.push(``);
  }
  return lines;
}

export interface BriefOptions {
  run: string;
  judgeId?: string;
  rubricPath?: string;
}

async function listArtifacts(runDir: string): Promise<string[]> {
  const out: string[] = [];
  async function walk(rel: string): Promise<void> {
    const abs = path.join(runDir, rel);
    let entries;
    try {
      entries = await readdir(abs, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      const child = rel ? `${rel}/${e.name}` : e.name;
      if (e.isDirectory()) {
        if (e.name === "judge") continue;
        await walk(child);
      } else {
        out.push(child);
      }
    }
  }
  await walk("");
  return out.sort();
}

function judgeSchema(rubric: Rubric, judgeId: string): string {
  const dims = rubric.dimensions
    .map(
      (d) =>
        `    { "id": "${d.id}", "score": <${rubric.scale.min}..${rubric.scale.max}>, "rationale": "...", "evidence": [{ "file": "pages/...", "note": "..." }] }`,
    )
    .join(",\n");
  return `{
  "rubric_version": ${rubric.version},
  "judge": "${judgeId}",
  "dimensions": [
${dims}
  ],
  "findings": [
    {
      "severity": "blocker|major|minor",
      "dimension": "<id>",
      "page": "pages/...",
      "description": "...",
      "suggestion": "..."
    }
  ]
}`;
}

export async function writeBrief(opts: BriefOptions): Promise<string> {
  const runDir = path.resolve(opts.run);
  const judgeId = opts.judgeId ?? "a";
  const rubric = await loadRubric(opts.rubricPath);

  let manifest: Manifest | null = null;
  try {
    manifest = JSON.parse(await readFile(path.join(runDir, "manifest.json"), "utf8")) as Manifest;
  } catch {
    manifest = null;
  }

  const judgePrompt = await readFile(
    path.join(packageRoot(), "prompts", "judge.md"),
    "utf8",
  );
  const artifacts = await listArtifacts(runDir);
  const checksPreview = await readFile(path.join(runDir, "checks.json"), "utf8").catch(
    () => "(checks.json missing)",
  );
  const journeyChecks = await readFile(path.join(runDir, "journey-checks.json"), "utf8").catch(
    () => null,
  );
  const journeys = await readFile(path.join(runDir, "journeys.json"), "utf8")
    .then((text) => JSON.parse(text) as JourneysFile)
    .catch(() => null);

  const lines: string[] = [
    judgePrompt.trim(),
    ``,
    `---`,
    ``,
    `## This run`,
    ``,
    `- Run directory: \`${runDir}\``,
    `- Judge id: \`${judgeId}\``,
    `- Write output to: \`judge/${judgeId}.json\``,
    manifest
      ? `- Captured: ${manifest.captured_at} · URL: ${manifest.url} · label: ${manifest.label ?? "(none)"}`
      : `- manifest.json missing`,
    ``,
    `## Rubric`,
    ``,
    renderRubricText(rubric),
    ``,
    `## Artifacts in this run`,
    ``,
    ...artifacts.map((a) => `- \`${a}\``),
    ``,
    `## User-story journeys`,
    ``,
    ...journeyLines(journeys),
    ``,
    `## Mechanical checks (excerpt)`,
    ``,
    "```json",
    checksPreview.length > 4000 ? checksPreview.slice(0, 4000) + "\n…" : checksPreview,
    "```",
    ...(journeyChecks
      ? [``, `Journey checks:`, ``, "```json", journeyChecks.length > 3000 ? journeyChecks.slice(0, 3000) + "\n…" : journeyChecks, "```"]
      : []),
    ``,
    `## Exact output JSON schema`,
    ``,
    "```json",
    judgeSchema(rubric, judgeId),
    "```",
    ``,
  ];

  const judgeDir = path.join(runDir, "judge");
  await mkdir(judgeDir, { recursive: true });
  const briefPath = path.join(judgeDir, `${judgeId}.brief.md`);
  await writeFile(briefPath, lines.join("\n"), "utf8");
  return briefPath;
}
