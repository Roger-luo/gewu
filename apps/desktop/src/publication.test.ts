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

    expect(plan.pullRequest?.comments).toEqual([
      expect.objectContaining({ path: "src/lib.rs", startLine: 40, endLine: 42 }),
    ]);
    expect(plan.pullRequest?.summary).toContain("one review comment");
    expect(plan.pullRequest?.summary).toContain("**Naming.** The name is unclear.");
    expect(plan.codeIssues).toBeNull();
    expect(plan.gewuIssues).toBeNull();
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

    expect(plan.pullRequest).toBeNull();
    expect(plan.gewuIssues?.repository).toBe("gewu");
    expect(plan.gewuIssues?.issues).toHaveLength(2);
    expect(plan.gewuIssues?.issues[0]?.body).toContain("This is about the walkthrough Gewu wrote.");
    expect(plan.gewuIssues?.tracking?.body).toContain("2 separate notes");
    expect(plan.gewuIssues?.tracking?.body).toContain("The example never runs");
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

    expect(plan.pullRequest).toBeNull();
    expect(plan.codeIssues?.issues).toHaveLength(2);
    expect(plan.codeIssues?.issues[0]?.body).toContain("previous version");
    expect(plan.codeIssues?.issues[0]?.body).toContain("commits main..feature on demo");
    expect(plan.codeIssues?.tracking).not.toBeNull();
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
