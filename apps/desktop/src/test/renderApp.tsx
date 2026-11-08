import { render } from "@testing-library/react";

import { App } from "../App";
import { MockBackend } from "../backend";
import { createMockSnapshot } from "../mock";
import type { Snapshot } from "../protocol";

export function renderApp(snapshotName = "showcase", snapshot?: Snapshot) {
  const backend = new MockBackend(snapshot ?? createMockSnapshot(snapshotName));
  const view = render(<App backend={backend} />);
  return { backend, ...view };
}

export async function waitForReady(): Promise<void> {
  await vi.waitFor(() => {
    expect(document.documentElement.dataset.gewuReady).toBe("true");
  });
}
