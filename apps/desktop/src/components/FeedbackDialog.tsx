import { useEffect, useId, useRef, useState } from "react";

import type { FeedbackKind, FeedbackTarget } from "../protocol";

export function FeedbackDialog({
  kind,
  anchorSummary,
  onCancel,
  onSubmit,
}: {
  kind: FeedbackKind;
  anchorSummary: string | null;
  onCancel: () => void;
  onSubmit: (body: string, target: FeedbackTarget) => void;
}) {
  const titleId = useId();
  const [body, setBody] = useState("");
  const [target, setTarget] = useState<FeedbackTarget>("code");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const title = kind === "note" ? "Add comment" : "Request changes";

  return (
    <div className="dialog-backdrop">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <h2 id={titleId}>{title}</h2>
        <label>
          <span>Applies to</span>
          <select
            value={target}
            onChange={(event) =>
              setTarget(event.target.value as FeedbackTarget)
            }
          >
            <option value="code">The code under review</option>
            <option value="explanation">The walkthrough explanation</option>
          </select>
        </label>
        <p className="anchor-status" role="status">
          {anchorSummary
            ? `Anchored to ${anchorSummary}`
            : target === "code"
              ? "No line selected — applies to the whole chunk. Select new-side line numbers in a quote to anchor."
              : "Targets the explanation, not a specific line."}
        </p>
        <label>
          <span>Body</span>
          <textarea
            ref={textareaRef}
            value={body}
            onChange={(event) => setBody(event.target.value)}
          />
        </label>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="btn primary"
            disabled={!body.trim()}
            onClick={() => onSubmit(body.trim(), target)}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
