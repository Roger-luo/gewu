import DOMPurify from "dompurify";

import type { QuotePreview as Preview } from "../protocol";

/** The picture of what a quote produces, beside its source. */
export function QuotePreview({ preview }: { preview: Preview }) {
  const svg =
    preview.media === "image/svg+xml"
      ? DOMPurify.sanitize(preview.data, { USE_PROFILES: { svg: true, svgFilters: true } })
      : null;
  return (
    <aside className="quote-preview" aria-label={preview.label}>
      <p className="quote-preview-label">{preview.label}</p>
      {svg != null ? (
        <div className="quote-preview-svg" dangerouslySetInnerHTML={{ __html: svg }} />
      ) : (
        <img alt={preview.label} src={`data:${preview.media};base64,${preview.data}`} />
      )}
    </aside>
  );
}
