import { useState } from "react";

import type { Receipt } from "../publication";
import type { PromptReport } from "./protocol";

/** Where a report goes: the same public repository walkthrough notes go to. */
const GEWU_REPOSITORY = "Roger-luo/gewu";

/**
 * Tells Gewu what is wrong with one prompt. Sending files a public issue on
 * Gewu's repository, so it takes a second, explicit confirmation, and the
 * issue carries only what the reader wrote plus which prompt it is about.
 */
export function PromptReportForm({
  promptId,
  promptTitle,
  backend,
  backendLabel,
  onSend,
}: {
  promptId: string;
  promptTitle: string;
  backend: string | null;
  backendLabel: string | null;
  onSend: (report: PromptReport) => Promise<Receipt>;
}) {
  const [body, setBody] = useState("");
  const [step, setStep] = useState<"write" | "confirm" | "sending">("write");
  const [receipt, setReceipt] = useState<Receipt | null>(null);
  const [error, setError] = useState<string | null>(null);

  const send = async () => {
    setStep("sending");
    setError(null);
    try {
      const sent = await onSend({ prompt_id: promptId, backend, body: body.trim() });
      setReceipt(sent);
      if (!sent.failure) setBody("");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
    setStep("write");
  };

  const about = backendLabel ? `the ${promptTitle} prompt as sent to ${backendLabel}` : `the ${promptTitle} prompt`;

  return (
    <div className="prompt-report" role="group" aria-label="Report a problem with this prompt">
      <label className="field">
        <span>Something wrong or unclear in it?</span>
        <textarea
          value={body}
          placeholder="What it gets wrong, and what you expected instead"
          onChange={(event) => {
            setBody(event.target.value);
            setStep("write");
            setReceipt(null);
          }}
        />
      </label>
      <p className="muted field-hint">
        Sending opens a public issue on <code>{GEWU_REPOSITORY}</code> about {about}, with only what
        you write here. Describe the prompt, not private code.
      </p>
      {step === "confirm" ? (
        <div className="prompt-report-actions" role="group" aria-label="Confirm the report">
          <span>Open a public issue on {GEWU_REPOSITORY}?</span>
          <button type="button" className="btn" onClick={() => setStep("write")}>
            Cancel
          </button>
          <button type="button" className="btn primary" onClick={() => void send()}>
            Send
          </button>
        </div>
      ) : (
        <div className="prompt-report-actions">
          <button
            type="button"
            className="btn"
            disabled={!body.trim() || step === "sending"}
            onClick={() => setStep("confirm")}
          >
            {step === "sending" ? "Sending…" : "Report a problem"}
          </button>
        </div>
      )}
      {error && (
        <p className="warning" role="alert">
          Not sent: {error}
        </p>
      )}
      {receipt && (
        <p className={receipt.failure ? "warning" : "muted"} role="status">
          {receipt.failure
            ? `Not sent: ${receipt.failure}`
            : receipt.posted.map((posted) => (
                <span key={posted.url}>
                  {receipt.rehearsed ? "Rehearsed, nothing sent: " : "Sent: "}
                  <a href={posted.url} target="_blank" rel="noreferrer">
                    {posted.label}
                  </a>
                </span>
              ))}
        </p>
      )}
    </div>
  );
}
