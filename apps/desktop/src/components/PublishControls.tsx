import { useState } from "react";

import type { GitHubAccount } from "../backend/types";
import type { Receipt } from "../publication";
import { Dialog } from "../workbench/Dialog";
import { InlineMarkdown } from "./Markdown";

/** Publish, its confirmation, and what a run sent. Nothing here sends until confirmed. */

export function PublishBar({
  blocked,
  account,
  count,
  rehearsing,
  onPublish,
}: {
  blocked: string | null;
  account: GitHubAccount | null;
  count: number;
  /** Settings say to rehearse: nothing will be sent. */
  rehearsing: boolean;
  onPublish: () => void;
}) {
  const reason = blocked ?? (account ? null : "Sign in to GitHub at the bottom of the sidebar to publish.");
  return (
    <div className="publication-actions">
      {reason && (
        <p className="muted">
          <InlineMarkdown source={reason} />
        </p>
      )}
      {!reason && rehearsing && (
        <p className="muted">Rehearsing: Publish shows what it would post and sends nothing.</p>
      )}
      <button type="button" className="btn primary" disabled={reason !== null} onClick={onPublish}>
        {rehearsing ? "Rehearse publishing" : "Publish"} {count === 1 ? "1 item" : `${count} items`}…
      </button>
    </div>
  );
}

export function ConfirmPublish({
  account,
  paths,
  rehearsing,
  onCancel,
  onConfirm,
}: {
  account: GitHubAccount;
  paths: string[];
  rehearsing: boolean;
  onCancel: () => void;
  onConfirm: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  return (
    <Dialog title="Publish to GitHub" onClose={busy ? () => undefined : onCancel}>
      {rehearsing ? (
        <p>
          Rehearsal: Gewu will record these{" "}
          {paths.length === 1 ? "request" : `${paths.length} requests`} as{" "}
          <strong>{account.login}</strong> and send nothing. Turn rehearsal off in Settings to
          publish for real.
        </p>
      ) : (
        <p>
          Gewu will send these {paths.length === 1 ? "request" : `${paths.length} requests`} as{" "}
          <strong>{account.login}</strong>. Each one is visible to the repository&apos;s
          maintainers as soon as it is sent.
        </p>
      )}
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
          {rehearsing ? (busy ? "Rehearsing…" : "Rehearse") : busy ? "Sending…" : "Send to GitHub"}
        </button>
      </div>
    </Dialog>
  );
}

export function ReceiptView({ receipt }: { receipt: Receipt }) {
  return (
    <div className={`publication-receipt${receipt.failure ? " failed" : ""}`} role="status">
      {receipt.posted.length > 0 &&
        (receipt.rehearsed ? (
          <p>
            Rehearsed {receipt.posted.length === 1 ? "1 item" : `${receipt.posted.length} items`}.
            Nothing was sent to GitHub, and the notes are still waiting to be published. Turn off
            rehearsal in Settings to send them.
          </p>
        ) : (
          <p>
            Sent {receipt.posted.length === 1 ? "1 item" : `${receipt.posted.length} items`} to
            GitHub.
          </p>
        ))}
      {receipt.failure && <p className="publication-failure">{receipt.failure}</p>}
      {receipt.failure?.includes("OAuth App access restrictions") && (
        <p>
          This organization has not approved Gewu&apos;s GitHub app. Press Switch on
          the account at the bottom of the sidebar and use the GitHub CLI, or ask an
          owner to approve Gewu. Nothing more was sent, so publishing again picks up
          where this stopped.
        </p>
      )}
    </div>
  );
}
