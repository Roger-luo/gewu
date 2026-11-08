import { mkdir, readFile, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { capture } from "../src/capture.js";
import { CHECK_KEYS, type ChecksFile, type Manifest } from "../src/types.js";
import { startFixtureServer, testRubricPath } from "./helpers.js";

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const RUN = path.join(ROOT, ".tmp-runs", "capture-one");

describe("capture", () => {
  let url: string;
  let close: () => Promise<void>;

  beforeAll(async () => {
    await rm(RUN, { recursive: true, force: true });
    await mkdir(path.dirname(RUN), { recursive: true });
    ({ url, close } = await startFixtureServer());
  });

  afterAll(async () => {
    await close();
  });

  it("writes pages, aria, manifest, and checks with expected counters", async () => {
    const result = await capture({
      url,
      out: RUN,
      viewports: "800x600",
      label: "fixture",
      rubricPath: testRubricPath,
    });

    expect(result.out).toBe(path.resolve(RUN));

    const manifest = JSON.parse(
      await readFile(path.join(RUN, "manifest.json"), "utf8"),
    ) as Manifest;
    expect(manifest.url).toBe(url);
    expect(manifest.label).toBe("fixture");
    expect(manifest.viewports).toEqual(["800x600"]);
    expect(manifest.pages.length).toBe(4);
    expect(manifest.rubric_version).toBe(1);
    expect(manifest.captured_at).toMatch(/^\d{4}-/);

    const overviewPng = path.join(RUN, "pages", "800x600", "00-overview.png");
    const overviewAria = path.join(RUN, "pages", "800x600", "00-overview.aria.yaml");
    await expect(readFile(overviewPng)).resolves.toBeInstanceOf(Buffer);
    const aria = await readFile(overviewAria, "utf8");
    expect(aria.length).toBeGreaterThan(20);

    const checks = JSON.parse(
      await readFile(path.join(RUN, "checks.json"), "utf8"),
    ) as ChecksFile;
    for (const key of CHECK_KEYS) {
      expect(checks.counters).toHaveProperty(key);
      expect(typeof checks.counters[key]).toBe("number");
    }
    expect(checks.counters["targets.too_small"]).toBeGreaterThanOrEqual(1);
    expect(checks.findings.some((f) => f.key === "targets.too_small")).toBe(true);

    const clipped = checks.findings.filter((f) => f.key === "layout.clipped_text");
    expect(clipped.some((f) => f.selector === "#clipped")).toBe(true);
    expect(clipped.some((f) => f.selector === "#scroll-pane")).toBe(false);
    expect(clipped.some((f) => f.selector === "main" || f.selector === "#main")).toBe(false);

    const markdown = checks.findings.filter((f) => f.key === "text.raw_markdown");
    expect(markdown.map((f) => f.selector)).toContain("p.raw-markdown");
    expect(markdown.some((f) => f.description.includes("literal"))).toBe(false);

    const misaligned = checks.findings.filter((f) => f.key === "layout.misaligned");
    expect(misaligned.map((f) => f.selector)).toContain("span.caption");

    const unhighlighted = checks.findings.filter((f) => f.key === "code.unhighlighted");
    expect(unhighlighted.map((f) => f.selector)).toContain("pre.code-plain");
    expect(unhighlighted.map((f) => f.selector)).toContain("table.diff-table");
    const diagrams = checks.findings.filter((f) => f.key === "illustration.ascii");
    expect(diagrams.some((f) => f.description.includes("parser"))).toBe(true);
    expect(diagrams.some((f) => f.description.includes("no labels"))).toBe(true);
    expect(manifest.interactions.length).toBeGreaterThanOrEqual(1);
  }, 90_000);
});
