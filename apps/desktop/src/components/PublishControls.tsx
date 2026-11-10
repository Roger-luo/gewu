import { useState } from "react";

import type { GitHubAccount } from "../backend/types";
import type { Receipt } from "../publication";
import { Dialog } from "../workbench/Dialog";

/** Publish, its confirmation, and what a run sent. Nothing here sends until confirmed. */

export function PublishBar({
  blocked,
  account,
  count,
  onPublish,
}: {
  blocked: string | null;
  account: GitHubAccount | null;
  count: number;
  onPublish: () => void;
}) {
  const reason = blocked ?? (account ? null : "Sign in to GitHub at the bottom of the sidebar to publish.");
  return (
    <div className="publication-actions">
      {reason && <p className="muted">{reason}</p>}
      <button type="button" className="btn primary" disabled={reason !== null} onClick={onPublish}>
        Publish {count === 1 ? "1 item" : `${count} items`}…
      </button>
    </div>
  );
}

export function ConfirmPublish({
  account,
  paths,
  onCancel,
  onConfirm,
}: {
  account: GitHubAccount;
  paths: string[];
  onCancel: () => void;
  onConfirm: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  return (
    <Dialog title="Publish to GitHub" onClose={busy ? () => undefined : onCancel}>
      <p>
        Gewu will send these {paths.length === 1 ? "request" : `${paths.length} requests`} as{" "}
        <strong>{account.login}</strong>. Each one is visible to the repository&apos;s
        maintainers as soon as it is sent.
      </p>
      <ul className="publication-confirm-list">
        {paths.map((path, index) => (
          <li key={`${index}-${path}`}>
            <code>{path}</code>
          </li>
        ))}
      </ul>
      <div className="dialog-actions">
        <button type="button" className="btn" disabled={busy} onClick={onCancel}>
          Cancel
        </button>
        <button
          type="button"
          className="btn primary"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            void onConfirm();
          }}
        >
          {busy ? "Sending…" : "Send to GitHub"}
        </button>
      </div>
    </Dialog>
  );
}

export function ReceiptView({ receipt }: { receipt: Receipt }) {
  return (
    <div className={`publication-receipt${receipt.failure ? " failed" : ""}`} role="status">
      {receipt.posted.length > 0 && (
        <p>
          Sent {receipt.posted.length === 1 ? "1 item" : `${receipt.posted.length} items`} to
          GitHub.
        </p>
      )}
      {receipt.failure && <p className="publication-failure">{receipt.failure}</p>}
    </div>
  );
}
