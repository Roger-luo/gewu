import DOMPurify from "dompurify";
import "katex/dist/katex.min.css";
import { marked, type Tokens } from "marked";

import { MathSlots, mathExtension } from "./math";

const math = new MathSlots();

marked.setOptions({ gfm: true, breaks: false });
marked.use(mathExtension(math));

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
  return math.restore(
    DOMPurify.sanitize(html, {
      USE_PROFILES: { html: true },
    }),
  );
}

/** One line of agent prose (a title or summary): code spans and emphasis, no blocks. */
export function renderInlineMarkdown(source: string): string {
  const html = marked.parseInline(source, { async: false }) as string;
  return math.restore(
    DOMPurify.sanitize(html, {
      ALLOWED_TAGS: ["code", "em", "strong", "del", "a"],
      ALLOWED_ATTR: ["href"],
    }),
  );
}

/** The same line as plain text, for accessible names and tooltips. */
export function plainInline(source: string): string {
  const doc = new DOMParser().parseFromString(renderInlineMarkdown(source), "text/html");
  return doc.body.textContent ?? source;
}
