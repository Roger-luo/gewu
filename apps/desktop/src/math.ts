import katex from "katex";
import type { MarkedExtension, Tokens } from "marked";

/**
 * TeX math in agent prose: `$…$` inline and `$$…$$` for display. An inline
 * `$` opens only before a non-space and closes only after one, and not
 * before a digit, so `$HOME`, `$PATH`, and `$5 and $10` stay text.
 *
 * KaTeX's output goes into the page after sanitizing, so DOMPurify never
 * sees (or strips) its markup: the renderer leaves a placeholder, and
 * {@link MathSlots.restore} swaps each one for its rendered math.
 */
export class MathSlots {
  private rendered: string[] = [];

  /** Rendered math for `tex`, as a placeholder that survives sanitizing. */
  slot(tex: string, display: boolean): string {
    const html = katex.renderToString(tex, {
      displayMode: display,
      throwOnError: false,
      // No \href, \url, or \htmlClass: agent prose is not trusted markup.
      trust: false,
      output: "htmlAndMathml",
    });
    this.rendered.push(html);
    return `${OPEN}${this.rendered.length - 1}${CLOSE}`;
  }

  /** `html` with every placeholder replaced, then forgotten. */
  restore(html: string): string {
    const out = html.replace(SLOT, (_, index: string) => this.rendered[Number(index)] ?? "");
    this.rendered = [];
    return out;
  }
}

// Private-use characters: text an agent will not write, kept by DOMPurify.
const OPEN = "";
const CLOSE = "";
const SLOT = /(\d+)/g;

const BLOCK = /^\$\$\n?([\s\S]+?)\n?\$\$(?:\n|$)/;
const DISPLAY = /^\$\$([\s\S]+?)\$\$/;
const INLINE = /^\$(?!\s)((?:\\.|[^\\$\n])+?)(?<!\s)\$(?!\d)/;

interface MathToken extends Tokens.Generic {
  tex: string;
}

/** The marked extension that hands math to `slots`. */
export function mathExtension(slots: MathSlots): MarkedExtension {
  return {
    extensions: [
      {
        name: "mathBlock",
        level: "block",
        start: (src) => src.match(/^\$\$/m)?.index,
        tokenizer(src) {
          const match = BLOCK.exec(src);
          return match ? { type: "mathBlock", raw: match[0], tex: match[1].trim() } : undefined;
        },
        renderer: (token) => `<div class="math-display">${slots.slot((token as MathToken).tex, true)}</div>\n`,
      },
      {
        name: "mathInline",
        level: "inline",
        start: (src) => src.indexOf("$") >= 0 ? src.indexOf("$") : undefined,
        tokenizer(src) {
          const display = DISPLAY.exec(src);
          if (display) {
            return { type: "mathInline", raw: display[0], tex: display[1].trim(), display: true };
          }
          const match = INLINE.exec(src);
          return match ? { type: "mathInline", raw: match[0], tex: match[1], display: false } : undefined;
        },
        renderer: (token) => slots.slot((token as MathToken).tex, Boolean(token.display)),
      },
    ],
  };
}
