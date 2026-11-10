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

  it("v3 scores review flow and caps raw Markdown and misaligned captions", async () => {
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v3.json"));
    expect(rubric.dimensions.map((d) => d.id)).toContain("review_flow");
    const caps = Object.fromEntries(rubric.gates.map((g) => [g.check, g.cap]));
    expect(caps["text.raw_markdown"]).toBe(70);
    expect(caps["layout.misaligned"]).toBe(80);
    expect(caps["code.unhighlighted"]).toBe(70);
    expect(caps["illustration.ascii"]).toBe(75);
  });

  it("v9 scores a new structure against the one it replaces", async () => {
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v9.json"));
    expect(rubric.version).toBe(9);
    const contrast = rubric.dimensions.find((dimension) => dimension.id === "representation_contrast");
    expect(contrast?.weight).toBe(3);
    expect(contrast?.anchors["1"]).toContain("previous structure");
    expect(contrast?.look_for.join(" ")).toContain("Refactor");
    const caps = Object.fromEntries(rubric.gates.map((gate) => [gate.check, gate.cap]));
    expect(caps["layout.column_drift"]).toBe(80);
  });

  it("v8 caps a list whose status labels drift sideways", async () => {
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v8.json"));
    const caps = Object.fromEntries(rubric.gates.map((gate) => [gate.check, gate.cap]));
    expect(caps["layout.column_drift"]).toBe(80);
    expect(caps["preview.backdrop"]).toBe(60);
    const hierarchy = rubric.dimensions.find((dimension) => dimension.id === "visual_hierarchy");
    expect(hierarchy?.look_for.join(" ")).toContain("layout.column_drift");
    const reviews = rubric.dimensions.find((dimension) => dimension.id === "multi_review_navigation");
    expect(reviews?.look_for.join(" ")).toContain("layout.column_drift");
    expect(reviews?.anchors["2"]).toContain("drift");
  });

  it("v7 caps a page fill smaller than the drawing", async () => {
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v7.json"));
    const caps = Object.fromEntries(rubric.gates.map((gate) => [gate.check, gate.cap]));
    expect(caps["preview.backdrop"]).toBe(60);
    const shown = rubric.dimensions.find((dimension) => dimension.id === "shown_result");
    expect(shown?.look_for.join(" ")).toContain("preview.backdrop");
    expect(shown?.anchors["1"]).toContain("page fill");
  });

  it("v6 caps a preview that cuts off the drawing", async () => {
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v6.json"));
    const caps = Object.fromEntries(rubric.gates.map((gate) => [gate.check, gate.cap]));
    expect(caps["preview.clipped"]).toBe(60);
    expect(caps["preview.blank"]).toBe(60);
    const shown = rubric.dimensions.find((dimension) => dimension.id === "shown_result");
    expect(shown?.anchors["1"]).toContain("cropped");
    expect(shown?.look_for.join(" ")).toContain("preview.clipped");
  });

  it("v5 caps a blank preview of what the code draws", async () => {
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v5.json"));
    expect(rubric.dimensions.map((dimension) => dimension.id)).toContain("shown_result");
    const caps = Object.fromEntries(rubric.gates.map((gate) => [gate.check, gate.cap]));
    expect(caps["preview.blank"]).toBe(60);
    const shown = rubric.dimensions.find((dimension) => dimension.id === "shown_result");
    expect(shown?.anchors["0"]).toContain("empty rendered panel");
  });

  it("v4 caps a diagram whose labels are too small to read", async () => {
    const rubric = await loadRubric(path.join(import.meta.dirname, "../rubric/v4.json"));
    const caps = Object.fromEntries(rubric.gates.map((g) => [g.check, g.cap]));
    expect(caps["illustration.too_small"]).toBe(70);
    const overview = rubric.dimensions.find((dimension) => dimension.id === "overview_intent");
    expect(overview?.look_for.join(" ")).toContain("illustration.too_small");
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
