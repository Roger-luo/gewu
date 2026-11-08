import type { AgentPreset, RepositoryRefs } from "./protocol";

export const AGENTS: AgentPreset[] = [
  {
    id: "none",
    label: "No agent (raw diff, file order)",
    kind: "none",
    command: "",
    available: true,
    detail: null,
  },
  {
    id: "claude",
    label: "Claude Code",
    kind: "acp",
    command: "npx -y @zed-industries/claude-code-acp",
    available: true,
    detail: null,
  },
  {
    id: "codex",
    label: "Codex",
    kind: "acp",
    command: "npx -y @zed-industries/codex-acp",
    available: true,
    detail: null,
  },
  {
    id: "cursor",
    label: "Cursor Agent",
    kind: "acp",
    command: "agent acp",
    available: true,
    detail: null,
  },
  {
    id: "gemini",
    label: "Gemini CLI",
    kind: "acp",
    command: "gemini --acp",
    available: false,
    detail: "`gemini` was not found on PATH",
  },
  {
    id: "custom",
    label: "Custom ACP command",
    kind: "acp",
    command: "",
    available: true,
    detail: null,
  },
];

export const REFS: RepositoryRefs = {
  branch: "main",
  dirty: true,
  commits: [
    { sha: "4f2a9c1e", subject: "feat(review): stable session keys", author: "Ada", date: "2026-11-06", has_parent: true },
    { sha: "9b1d03aa", subject: "fix(harness): keep progress on append", author: "Lin", date: "2026-11-05", has_parent: true },
    { sha: "c07e5512", subject: "docs: describe batch-scoped decisions", author: "Ada", date: "2026-11-04", has_parent: true },
    { sha: "1aa4be90", subject: "test: cover orphaned events", author: "Mo", date: "2026-11-03", has_parent: false },
  ],
  branches: ["main", "feature/session-keys"],
  pull_requests: [
    { number: 42, title: "Stable session keys", author: "ada", head_ref: "feature/session-keys" },
    { number: 37, title: "Render markdown chunks", author: "lin", head_ref: "feature/markdown" },
  ],
  directories: ["crates", "crates/gewu-review", "crates/gewu-git", "docs"],
};
