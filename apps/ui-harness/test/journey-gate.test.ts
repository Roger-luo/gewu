import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { loadRubric } from "../src/rubric.js";
import { loadChecks } from "../src/score.js";
import { CHECK_KEYS, type ChecksFile } from "../src/types.js";

function checks(overrides: Partial<ChecksFile["counters"]> = {}): ChecksFile {
  const counters = Object.fromEntries(CHECK_KEYS.map((key) => [key, 0])) as ChecksFile["counters"];
  return { counters: { ...counters, ...overrides }, findings: [] };
}

describe("journey gate", () => {
  let dir: string | null = null;
  afterEach(async () => {
    if (dir) await rm(dir, { recursive: true, force: true });
    dir = null;
  });

  it("counts a v2 run without journeys as blocked", async () => {
    dir = await mkdtemp(path.join(tmpdir(), "gewu-ui-gate-"));
    await writeFile(path.join(dir, "checks.json"), JSON.stringify(checks()));
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v2.json"));
    expect((await loadChecks(dir, rubric)).counters["journey.blocked"]).toBe(1);
  });

  it("adds journey counters to page counters", async () => {
    dir = await mkdtemp(path.join(tmpdir(), "gewu-ui-gate-"));
    await writeFile(path.join(dir, "checks.json"), JSON.stringify(checks({ "axe.serious": 1 })));
    await writeFile(path.join(dir, "journey-checks.json"), JSON.stringify(checks({ "axe.serious": 2 })));
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v2.json"));
    const merged = await loadChecks(dir, rubric);
    expect(merged.counters["axe.serious"]).toBe(3);
    expect(merged.counters["journey.blocked"]).toBe(0);
  });
});
