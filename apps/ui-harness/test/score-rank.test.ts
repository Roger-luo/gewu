import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it } from "vitest";
import { writeBrief } from "../src/brief.js";
import { capture } from "../src/capture.js";
import { rank } from "../src/rank.js";
import { formatScoreSummary, score } from "../src/score.js";
import type { ChecksFile, JudgeOutput } from "../src/types.js";
import { startFixtureServer, testRubricPath } from "./helpers.js";

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const RUN_A = path.join(ROOT, ".tmp-runs", "score-a");
const RUN_B = path.join(ROOT, ".tmp-runs", "score-b");

async function seedJudge(
  runDir: string,
  id: string,
  scores: { orientation: number; a11y: number },
  evidenceFile: string,
): Promise<void> {
  const judge: JudgeOutput = {
    rubric_version: 1,
    judge: id,
    dimensions: [
      {
        id: "orientation",
        score: scores.orientation,
        rationale: "Navigator and titles are visible.",
        evidence: [{ file: evidenceFile, note: "Overview shows title and nav" }],
      },
      {
        id: "a11y",
        score: scores.a11y,
        rationale: "Landmarks present; one tiny control.",
        evidence: [{ file: evidenceFile, note: "ARIA snapshot has main and nav" }],
      },
    ],
    findings: [
      {
        severity: "major",
        dimension: "a11y",
        page: evidenceFile,
        description: "Tiny header button fails 24x24 target size",
        suggestion: "Enlarge the tiny action control to at least 24x24 CSS px",
      },
    ],
  };
  await mkdir(path.join(runDir, "judge"), { recursive: true });
  await writeFile(
    path.join(runDir, "judge", `${id}.json`),
    JSON.stringify(judge, null, 2),
    "utf8",
  );
}

describe("brief, score, and rank", () => {
  beforeAll(async () => {
    await rm(path.join(ROOT, ".tmp-runs"), { recursive: true, force: true });
    await mkdir(path.join(ROOT, ".tmp-runs"), { recursive: true });

    const { url, close } = await startFixtureServer();
    try {
      await capture({
        url,
        out: RUN_A,
        viewports: "800x600",
        label: "run-a",
        rubricPath: testRubricPath,
      });
      // Clone artifacts for run B with modified checks (simulate improved targets)
      await capture({
        url,
        out: RUN_B,
        viewports: "800x600",
        label: "run-b",
        rubricPath: testRubricPath,
      });
    } finally {
      await close();
    }

    const evidence = "pages/800x600/00-overview.png";
    await seedJudge(RUN_A, "a", { orientation: 2, a11y: 1 }, evidence);
    await seedJudge(RUN_B, "a", { orientation: 3, a11y: 2 }, evidence);

    // Simulate run B fixing tiny targets for gate comparison
    const checksB = JSON.parse(
      await readFile(path.join(RUN_B, "checks.json"), "utf8"),
    ) as ChecksFile;
    checksB.counters["targets.too_small"] = 0;
    checksB.findings = checksB.findings.filter((f) => f.key !== "targets.too_small");
    await writeFile(
      path.join(RUN_B, "checks.json"),
      JSON.stringify(checksB, null, 2),
      "utf8",
    );

    // Clear B findings related to tiny button for rank "fixed" detection
    const judgeB = JSON.parse(
      await readFile(path.join(RUN_B, "judge", "a.json"), "utf8"),
    ) as JudgeOutput;
    judgeB.findings = [
      {
        severity: "minor",
        dimension: "orientation",
        page: evidence,
        description: "Summary could echo remaining open issues more clearly",
        suggestion: "Add a short unresolved-issues line on the summary",
      },
    ];
    await writeFile(
      path.join(RUN_B, "judge", "a.json"),
      JSON.stringify(judgeB, null, 2),
      "utf8",
    );
  }, 180_000);

  it("writes a judge brief", async () => {
    const briefPath = await writeBrief({
      run: RUN_A,
      judgeId: "b",
      rubricPath: testRubricPath,
    });
    const text = await readFile(briefPath, "utf8");
    expect(text).toContain("Judge a Gewu desktop UI capture");
    expect(text).toContain("orientation");
    expect(text).toContain("judge/b.json");
    expect(text).toContain("Exact output JSON schema");
  });

  it("scores with gates and evidence validation", async () => {
    const scorecard = await score({ run: RUN_A, rubricPath: testRubricPath });
    expect(scorecard.judges).toContain("a");
    expect(scorecard.total).toBeLessThanOrEqual(70); // capped by tiny targets
    expect(scorecard.gates.some((g) => g.id === "target-size-cap" && !g.passed)).toBe(
      true,
    );
    expect(scorecard.dimensions.find((d) => d.id === "orientation")?.mean).toBe(2);

    const summary = formatScoreSummary(scorecard);
    expect(summary).toContain("Total:");
    expect(summary).toMatch(/Gates/);

    // Invalid evidence zeros a dimension
    const bad: JudgeOutput = {
      rubric_version: 1,
      judge: "bad",
      dimensions: [
        {
          id: "orientation",
          score: 3,
          rationale: "invented",
          evidence: [{ file: "pages/missing.png", note: "nope" }],
        },
        {
          id: "a11y",
          score: 3,
          rationale: "ok",
          evidence: [{ file: "pages/800x600/00-overview.png", note: "ok" }],
        },
      ],
      findings: [],
    };
    await writeFile(
      path.join(RUN_A, "judge", "bad.json"),
      JSON.stringify(bad, null, 2),
      "utf8",
    );
    const rescored = await score({ run: RUN_A, rubricPath: testRubricPath });
    const orientation = rescored.dimensions.find((d) => d.id === "orientation");
    expect(orientation?.scores).toContain(0);
    expect(
      rescored.validation.some((v) => v.judge === "bad" && v.issues.length > 0),
    ).toBe(true);
  });

  it("ranks two runs with deltas and finding changes", async () => {
    await score({ run: RUN_A, rubricPath: testRubricPath });
    await score({ run: RUN_B, rubricPath: testRubricPath });
    const md = await rank({
      runs: [RUN_A, RUN_B],
      out: path.join(ROOT, ".tmp-runs", "rank.md"),
    });
    expect(md).toContain("UI harness rank");
    expect(md).toContain("Dimension deltas");
    expect(md).toContain("Fixed");
    expect(md).toContain("New");
    expect(md).toMatch(/score-b/);
  });
});
