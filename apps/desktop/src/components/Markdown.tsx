import { renderInlineMarkdown, renderMarkdown } from "../markdown";

/** Inline agent prose inside a heading, paragraph, or list item. */
export function InlineMarkdown({ source }: { source: string }) {
  return (
    <span
      className="inline-markdown"
      dangerouslySetInnerHTML={{ __html: renderInlineMarkdown(source) }}
    />
  );
}

export function Markdown({
  source,
  className,
}: {
  source: string;
  className?: string;
}) {
  return (
    <div
      className={className ? `markdown ${className}` : "markdown"}
      dangerouslySetInnerHTML={{ __html: renderMarkdown(source) }}
    />
  );
}
