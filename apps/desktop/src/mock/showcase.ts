import {
  changes,
  chunk,
  line,
  narrative,
  quote,
  snapshot,
} from "./builders";
import type { Snapshot } from "../protocol";

export function showcaseSnapshot(): Snapshot {
  return snapshot({
    review_id: "demo-showcase",
    source: { base: "main", head: "feature/session-keys" },
    overview: {
      intent:
        "Introduce stable session keys so appending chunks never orphans reviewer progress.",
      changes: [
        "Stamp a durable review_id when a harness run starts.",
        "Derive session paths from review_id instead of a content hash.",
        "Keep generation status on the snapshot for incomplete batches.",
        "Surface confidence labels when explanations are uncertain.",
      ],
      concepts: ["session key", "batch-scoped decision"],
      illustration: {
        kind: "before_after",
        before: "session_key = hash(spec content)\nappend chunk → new hash → orphaned events",
        after:
          "session_key = review_id (stamped once)\nappend chunk → same key → progress kept",
      },
    },
    generation: { remaining_work_items: 1, complete: false },
    chunks: [
      chunk({
        id: "session-key",
        index: 0,
        title: "Session key identity",
        summary:
          "A `review_id` is stamped once and used as the session key so later edits cannot move the event log.",
        kind: "concept",
        section: "Identity via `review_id`",
        context: {
          kind: "concept",
          definition:
            "A session key is the directory name under .gewu/reviews/ where events.jsonl lives.",
          example: 'session_key() → "run-2026-03-14-a1"',
          relation:
            "This chunk introduces the key that later chunks store visited and reviewed state against.",
        },
        blocks: [
          narrative(
            "Previously the session path was derived from a content hash of the whole spec. Appending a chunk changed the hash, so the UI looked for events in a new empty directory.",
          ),
          changes("added_code", [
            quote({
              change_id: "c-session",
              path: "crates/gewu-review/src/spec.rs",
              start: 1,
              end: 8,
              language: "rust",
              lines: [
                line("added", "pub fn session_key(&self) -> String {", null, 220),
                line("added", "    if self.review_id.is_empty() {", null, 221),
                line("added", "        self.id()", null, 222),
                line("added", "    } else {", null, 223),
                line("added", "        self.review_id.clone()", null, 224),
                line("added", "    }", null, 225),
                line("added", "}", null, 226),
              ],
            }),
          ]),
        ],
      }),
      chunk({
        id: "event-path",
        index: 1,
        title: "Event path wiring via `session_key()`",
        summary:
          "The app service resolves the event log from `session_key()`, keeping TUI and desktop sessions interchangeable.",
        kind: "integration",
        section: "Identity via `review_id`",
        confidence: "Path joins follow the existing TUI layout; verify `session_key()` on resume.",
        blocks: [
          narrative(
            "Look at how the path is built. The `reviews/<key>/events.jsonl` layout is unchanged — only the key source moves.",
          ),
          changes("diff", [
            quote({
              change_id: "c-path",
              path: "crates/gewu-app/src/session.rs",
              start: 4,
              end: 10,
              language: "rust",
              more_above: true,
              more_below: true,
              lines: [
                line("context", "fn events_path(root: &Path, key: &str) -> PathBuf {", 40, 40),
                line("removed", "    root.join(\".gewu/reviews\").join(spec.id())", 41, null),
                line(
                  "added",
                  "    root.join(\".gewu/reviews\").join(key)",
                  null,
                  41,
                ),
                line("context", "        .join(\"events.jsonl\")", 42, 42),
                line("context", "}", 43, 43),
              ],
            }),
          ]),
        ],
      }),
      chunk({
        id: "docs-note",
        index: 2,
        title: "Document batch-scoped decisions",
        summary:
          "README guidance explains that a decision on an incomplete batch does not discard pending generation.",
        kind: "other",
        section: "Docs",
        blocks: [
          narrative("The docs change is the reviewer-facing explanation of batch scope."),
          changes("rendered_markdown", [
            quote({
              change_id: "c-docs",
              path: "docs/desktop.md",
              start: 1,
              end: 6,
              language: "markdown",
              lines: [
                line("added", "## Batch-scoped decisions", null, 210),
                line("added", "", null, 211),
                line(
                  "added",
                  "When generation is incomplete, label the decision as",
                  null,
                  212,
                ),
                line(
                  "added",
                  "batch-scoped. Recording it never discards pending work.",
                  null,
                  213,
                ),
                line("added", "", null, 214),
                line("added", "```text", null, 215),
                line("added", "decision = approve (batch)", null, 216),
                line("added", "```", null, 217),
              ],
            }),
          ]),
        ],
      }),
      chunk({
        id: "generated-fixture",
        index: 3,
        title: "Summarize generated fixture",
        summary:
          "Large generated JSON is summarized rather than shown in full; reviewers can still see that coverage exists.",
        kind: "test",
        section: "Fixtures",
        oversized_reason: "Generated fixture exceeds the viewport budget as a unit.",
        blocks: [
          narrative(
            "The harness emitted a large fixture. Its contents are not useful to inspect line by line.",
          ),
          changes("summary_only", [
            quote({
              change_id: "c-fixture",
              path: "fixtures/generated/session.json",
              start: 1,
              end: 1,
              language: "json",
              lines: [
                line(
                  "added",
                  "… 240 lines of generated session fixture omitted …",
                  null,
                  1,
                ),
              ],
            }),
          ]),
        ],
      }),
      chunk({
        id: "rename-helper",
        index: 4,
        title: "Rename path helper",
        summary:
          "What changed is the helper name; behavior is intended to stay the same.",
        kind: "refactor",
        section: "Cleanup",
        context: {
          kind: "refactor",
          before: "events_file(root, spec.id())",
          after: "events_path(root, spec.session_key())",
          reason: "Name matches PathBuf return and accepts the stable key.",
        },
        blocks: [
          narrative(
            "A pure rename with a parameter swap. Confirm call sites were updated together.",
          ),
          changes("diff", [
            quote({
              change_id: "c-rename",
              path: "crates/gewu-app/src/lib.rs",
              start: 1,
              end: 4,
              language: "rust",
              lines: [
                line("removed", "fn events_file(root: &Path, id: &str) -> PathBuf {", 10, null),
                line(
                  "added",
                  "fn events_path(root: &Path, key: &str) -> PathBuf {",
                  null,
                  10,
                ),
                line("context", "    root.join(\".gewu/reviews\").join(key)", 11, 11),
                line("context", "}", 12, 12),
              ],
            }),
          ]),
        ],
      }),
    ],
  });
}
