import type { Source, SourceType } from "./protocol";

/** Field values for every source kind, so switching tabs keeps them. */
export interface SourceDraft {
  base: string;
  head: string;
  workingBase: string;
  number: string;
  directory: string;
  ref: string;
}

export const EMPTY_DRAFT: SourceDraft = {
  base: "HEAD~1",
  head: "HEAD",
  workingBase: "HEAD",
  number: "",
  directory: "",
  ref: "HEAD",
};

export function draftSource(type: SourceType, draft: SourceDraft): Source | string {
  switch (type) {
    case "commit_range":
      return draft.base.trim() && draft.head.trim()
        ? { type, base: draft.base.trim(), head: draft.head.trim() }
        : "Enter a base and a head commit.";
    case "working_tree":
      return draft.workingBase.trim()
        ? { type, base: draft.workingBase.trim() }
        : "Enter the commit to compare against, usually HEAD.";
    case "pull_request": {
      const number = Number.parseInt(draft.number.replace(/^#/, ""), 10);
      return Number.isInteger(number) && number > 0
        ? { type, number, remote: "origin" }
        : "Pick a pull request or type its number.";
    }
    case "directory":
      return draft.ref.trim()
        ? { type, path: draft.directory.trim(), ref: draft.ref.trim() }
        : "Enter the ref to read the directory at.";
  }
}
