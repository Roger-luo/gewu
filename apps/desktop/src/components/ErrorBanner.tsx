import { InlineMarkdown } from "./Markdown";

export function ErrorBanner({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  return (
    <div className="error-banner" role="alert">
      <span>
        <InlineMarkdown source={message} />
      </span>
      <button type="button" onClick={onDismiss} aria-label="Dismiss error">
        Dismiss
      </button>
    </div>
  );
}
