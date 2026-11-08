import {
  changes,
  chunk,
  line,
  narrative,
  quote,
  snapshot,
} from "./builders";
import type { Snapshot } from "../protocol";

/** Smaller fixture mirroring a self-review shape for ?mock=self-review. */
export function selfReviewSnapshot(): Snapshot {
  return snapshot({
    review_id: "demo-self-review",
    source: { base: "abc1234", head: "def5678" },
    overview: {
      intent: "Harden the desktop snapshot protocol and UI test hooks.",
      changes: [
        "Expose window.__gewu for the harness.",
        "Keep data-gewu-ready in sync with navigation.",
        "Render quotes with path gutters and line numbers.",
      ],
      illustration: {
        kind: "flow",
        steps: [
          "Load snapshot",
          "Render screen",
          "Set data-gewu-ready",
          "Harness captures page",
        ],
      },
      concepts: ["test hook"],
    },
    generation: { remaining_work_items: 0, complete: true },
    chunks: [
      chunk({
        id: "hooks",
        index: 0,
        title: "Ready flag and test API",
        summary:
          "The document element carries data-gewu-ready once the current screen has settled.",
        kind: "integration",
        section: "Harness",
        blocks: [
          narrative(
            "The harness waits on `document.documentElement.dataset.gewuReady` before capturing.",
          ),
          changes("added_code", [
            quote({
              change_id: "c-hooks",
              path: "apps/desktop/src/hooks/useReady.ts",
              start: 1,
              end: 5,
              language: "typescript",
              lines: [
                line("added", "export function setReady(ready: boolean) {", null, 1),
                line(
                  "added",
                  '  document.documentElement.dataset.gewuReady = ready ? "true" : "false";',
                  null,
                  2,
                ),
                line("added", "}", null, 3),
              ],
            }),
          ]),
        ],
      }),
      chunk({
        id: "example-diagram",
        index: 1,
        title: "Overview illustrations",
        summary: "Example and diagram kinds round out the overview illustration set.",
        kind: "other",
        section: "Docs",
        blocks: [
          narrative("Illustrations clarify intent without repeating the bullet list."),
          changes("diff", [
            quote({
              change_id: "c-illust",
              path: "docs/design.md",
              start: 1,
              end: 3,
              language: "markdown",
              lines: [
                line("context", "### Overview", 40, 40),
                line("added", "- Example: input → output", null, 41),
                line("added", "- Diagram: ASCII connection sketch", null, 42),
              ],
            }),
          ]),
        ],
      }),
    ],
  });
}
