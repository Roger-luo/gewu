import { draftPlan, planIsEmpty, type Venue } from "./publication";
import type { Feedback } from "./protocol";

function feedback(partial: Partial<Feedback> & Pick<Feedback, "id" | "target" | "body">): Feedback {
  return {
    chunk_id: "chunk",
    kind: "issue",
    anchor: null,
    resolved: false,
    ...partial,
  };
}

const chunks = [
  { id: "path", title: "Event path" },
  { id: "names", title: "Naming" },
];

const pull: Venue = { type: "pull_request", repository: "demo", number: 12 };

describe("publication drafts", () => {
  it("keeps a line note on the pull request and summarizes a note with no line", () => {
    const plan = draftPlan(pull, "Session keys", chunks, [
      feedback({
        id: "line",
        chunk_id: "path",
        target: "code",
        body: "Confirm the path join.",
        anchor: {
          change_id: "c",
          path: "src/lib.rs",
          side: "new",
          start_line: 40,
          end_line: 42,
          patch_sha256: "hash",
        },
      }),
      feedback({
        id: "whole",
        chunk_id: "names",
        target: "code",
        body: "The name is unclear.",
      }),
    ]);

    expect(plan.pull_request?.comments).toEqual([
      expect.objectContaining({ path: "src/lib.rs", start_line: 40, end_line: 42 }),
    ]);
    expect(plan.pull_request?.summary).toContain("one review comment");
    expect(plan.pull_request?.summary).toContain("**Naming.** The name is unclear.");
    expect(plan.code_issues).toBeUndefined();
    expect(plan.gewu_issues).toBeUndefined();
  });

  it("files walkthrough notes on gewu, one issue each, plus a tracking issue", () => {
    const plan = draftPlan(pull, "Session keys", chunks, [
      feedback({ id: "a", chunk_id: "path", target: "explanation", body: "The example never runs." }),
      feedback({ id: "b", chunk_id: "names", target: "explanation", body: "Define the term first." }),
      feedback({
        id: "c",
        chunk_id: "path",
        target: "explanation",
        body: "Already handled.",
        resolved: true,
      }),
    ]);

    expect(plan.pull_request).toBeUndefined();
    expect(plan.gewu_issues?.repository).toBe("Roger-luo/gewu");
    expect(plan.gewu_issues?.issues).toHaveLength(2);
    expect(plan.gewu_issues?.issues[0]?.body).toContain("This is about the walkthrough Gewu wrote.");
    expect(plan.gewu_issues?.tracking?.body).toContain("2 separate notes");
    expect(plan.gewu_issues?.tracking?.body).toContain("The example never runs");
  });

  it("turns notes on commits into separate repository issues", () => {
    const plan = draftPlan(
      { type: "commits", repository: "demo", base: "main", head: "feature" },
      "Paths",
      chunks,
      [
        feedback({
          id: "one",
          chunk_id: "path",
          target: "code",
          body: "The helper returns the wrong root.",
          anchor: {
            change_id: "c",
            path: "src/lib.rs",
            side: "old",
            start_line: 8,
            end_line: 9,
            patch_sha256: "hash",
          },
        }),
        feedback({ id: "two", chunk_id: "names", target: "code", body: "Add a test for the empty path." }),
      ],
    );

    expect(plan.pull_request).toBeUndefined();
    expect(plan.code_issues?.issues).toHaveLength(2);
    expect(plan.code_issues?.issues[0]?.body).toContain("previous version");
    expect(plan.code_issues?.issues[0]?.body).toContain("commits main..feature on demo");
    expect(plan.code_issues?.tracking).toBeDefined();
  });

  it("is empty when every note is resolved", () => {
    const plan = draftPlan(
      { type: "working_tree", repository: "demo", base: "main" },
      "Work",
      chunks,
      [feedback({ id: "done", target: "code", body: "Done.", resolved: true })],
    );
    expect(planIsEmpty(plan)).toBe(true);
  });
});
