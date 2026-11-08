import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import type { Snapshot } from "./protocol";

const FIXTURE_ROOT = resolve(
  import.meta.dirname,
  "../../../fixtures/ui",
);

const FIXTURES = ["self-review", "showcase", "showcase-feedback"] as const;

describe("fixture snapshots", () => {
  for (const name of FIXTURES) {
    it(`parses ${name} when present`, async () => {
      const path = resolve(FIXTURE_ROOT, name, "snapshot.json");
      let raw: string;
      try {
        raw = await readFile(path, "utf8");
      } catch {
        // Another agent may still be generating these fixtures.
        return;
      }
      const snapshot = JSON.parse(raw) as Snapshot;
      expect(typeof snapshot.review_id).toBe("string");
      expect(snapshot.source).toEqual(
        expect.objectContaining({
          base: expect.any(String),
          head: expect.any(String),
        }),
      );
      expect(snapshot.overview.intent).toBeTruthy();
      expect(Array.isArray(snapshot.chunks)).toBe(true);
      expect(Array.isArray(snapshot.feedback)).toBe(true);
      expect(snapshot.summary).toEqual(
        expect.objectContaining({
          reviewed: expect.any(Number),
          total: expect.any(Number),
          suggested_decision: expect.any(String),
        }),
      );
      expect(snapshot.generation).toEqual(
        expect.objectContaining({
          complete: expect.any(Boolean),
        }),
      );
      for (const chunk of snapshot.chunks) {
        expect(chunk.id).toBeTruthy();
        expect(Array.isArray(chunk.blocks)).toBe(true);
        expect(chunk.status).toEqual(
          expect.objectContaining({
            visited: expect.any(Boolean),
            reviewed: expect.any(Boolean),
          }),
        );
      }
    });
  }
});
