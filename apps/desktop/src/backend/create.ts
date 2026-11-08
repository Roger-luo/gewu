import { createMockSnapshot } from "../mock";
import { HttpWorkbench, TauriWorkbench, type WorkbenchBackend } from "../workbench/backend";
import { MockWorkbench } from "../workbench/mock";
import { HttpBackend, request } from "./http";
import { MockBackend } from "./mock";
import { command, TauriBackend } from "./tauri";
import type { Backend } from "./types";

/** What the app opens: one prepared review, or the whole workbench. */
export type Launch =
  | { mode: "review"; backend: Backend }
  | { mode: "workbench"; backend: WorkbenchBackend };

function isTauri(): boolean {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

function mockNameFromQuery(): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  const params = new URLSearchParams(window.location.search);
  if (!params.has("mock")) {
    return null;
  }
  return params.get("mock") || "showcase";
}

/** `?mock=workbench` and `?mock=workbench-empty` open a simulated workbench. */
function mockLaunch(name: string): Launch {
  if (name === "workbench") {
    return { mode: "workbench", backend: MockWorkbench.seeded() };
  }
  if (name === "workbench-empty") {
    return { mode: "workbench", backend: new MockWorkbench() };
  }
  return { mode: "review", backend: new MockBackend(createMockSnapshot(name)) };
}

export async function createLaunch(): Promise<Launch> {
  const mockName = mockNameFromQuery();
  if (mockName) {
    return mockLaunch(mockName);
  }
  if (isTauri()) {
    const mode = await command<string>("mode");
    return mode === "workbench"
      ? { mode, backend: new TauriWorkbench() }
      : { mode: "review", backend: new TauriBackend() };
  }
  const health = await request<{ mode?: string }>("/api/health");
  return health.mode === "workbench"
    ? { mode: "workbench", backend: new HttpWorkbench() }
    : { mode: "review", backend: new HttpBackend() };
}
