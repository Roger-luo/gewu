import { useId } from "react";

export function HelpOverlay({ onClose }: { onClose: () => void }) {
  const titleId = useId();
  return (
    <div className="dialog-backdrop help-overlay">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <h2 id={titleId}>Review help</h2>
        <p>
          Comments record context without blocking completion. Change requests
          stay visible until resolved and must be revisited before a final
          decision. Jumping never marks a chunk reviewed.
        </p>
        <dl>
          <dt>j / k</dt>
          <dd>Next / previous page</dd>
          <dt>o</dt>
          <dd>Overview</dd>
          <dt>s</dt>
          <dd>Summary</dd>
          <dt>r</dt>
          <dd>Toggle reviewed on the current chunk</dd>
          <dt>c</dt>
          <dd>Add a comment (note)</dd>
          <dt>x</dt>
          <dd>Request changes (issue)</dd>
          <dt>?</dt>
          <dd>Toggle this help</dd>
          <dt>Esc</dt>
          <dd>Close overlay or dialog</dd>
        </dl>
        <p className="muted">
          Select new-side line numbers in a quote to optionally anchor feedback.
        </p>
        <p className="muted">
          Navigator marks: hollow circle, not reviewed; green circle, reviewed;
          red diamond, open change request.
        </p>
        <div className="dialog-actions">
          <button type="button" className="btn primary" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
