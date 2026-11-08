import DOMPurify from "dompurify";
import { marked, type Tokens } from "marked";

marked.setOptions({ gfm: true, breaks: false });

marked.use({
  renderer: {
    heading({ tokens, depth }: Tokens.Heading) {
      // Keep the chunk title as the only page-level h2; start document headings at h3.
      const level = Math.min(depth + 2, 6);
      const text = this.parser.parseInline(tokens);
      return `<h${level}>${text}</h${level}>\n`;
    },
  },
});

export function renderMarkdown(source: string): string {
  const html = marked.parse(source, { async: false }) as string;
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
  });
}
