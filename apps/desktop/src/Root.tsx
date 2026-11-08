import { useEffect, useState } from "react";

import { App } from "./App";
import { createLaunch, type Launch } from "./backend/create";
import { WorkbenchApp } from "./workbench/WorkbenchApp";

/** Ask the host which mode it serves, then open that app. */
export function Root({ launch }: { launch?: Launch }) {
  const [resolved, setResolved] = useState<Launch | null>(launch ?? null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (resolved) {
      return;
    }
    createLaunch()
      .then(setResolved)
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
  }, [resolved]);

  if (error) {
    return (
      <div className="boot-error" role="alert">
        Gewu could not start: {error}
      </div>
    );
  }
  if (!resolved) {
    return <div className="loading">Starting Gewu…</div>;
  }
  return resolved.mode === "workbench" ? (
    <WorkbenchApp backend={resolved.backend} />
  ) : (
    <App backend={resolved.backend} />
  );
}
