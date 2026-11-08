import { useEffect, useState } from "react";

import { highlightLines, prefersDark, type HighlightToken } from "../highlight";

/** Highlighted tokens for each line, or null while the grammar loads or the language is unknown. */
export function useHighlightedLines(
  code: string,
  language: string | null,
): HighlightToken[][] | null {
  const [lines, setLines] = useState<HighlightToken[][] | null>(null);

  useEffect(() => {
    let cancelled = false;
    const apply = (dark: boolean) => {
      void highlightLines(code, language, dark).then((result) => {
        if (!cancelled) {
          setLines(result);
        }
      });
    };
    apply(prefersDark());
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => apply(media.matches);
    media.addEventListener("change", onChange);
    return () => {
      cancelled = true;
      media.removeEventListener("change", onChange);
    };
  }, [code, language]);

  return lines;
}
