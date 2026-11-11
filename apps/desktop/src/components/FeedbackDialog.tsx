import { useEffect, useId, useRef, useState } from "react";

import type { Contributor } from "../backend/types";
import type { FeedbackKind, FeedbackTarget } from "../protocol";
import { MentionTextarea } from "./MentionTextarea";

export function FeedbackDialog({
  kind,
  anchorSummary,
  initialTarget = "code",
  contributors = [],
  onCancel,
  onSubmit,
}: {
  kind: FeedbackKind;
  anchorSummary: string | null;
  /** The target the reviewer opened the dialog on, e.g. a note's explanation. */
  initialTarget?: FeedbackTarget;
  /** Who the body can `@`-mention. */
  contributors?: Contributor[];
  onCancel: () => void;
  onSubmit: (body: string, target: FeedbackTarget) => void;
}) {
  const titleId = useId();
  const [body, setBody] = useState("");
  const [target, setTarget] = useState<FeedbackTarget>(initialTarget);
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
              ? "No line selected — applies to the whole chunk. To anchor it, cancel, click new-side line numbers in a quote, then open this again."
              : "Targets the explanation, not a specific line."}
        </p>
        <label>
          <span>
            Body
            {contributors.length > 0 && (
              <span className="muted mention-hint"> · type @ to mention a contributor</span>
            )}
          </span>
          <MentionTextarea
            ref={textareaRef}
            value={body}
            onChange={setBody}
            contributors={contributors}
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
