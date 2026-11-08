import { useEffect, useState } from "react";

import { highlightLines, prefersDark, type HighlightToken } from "../highlight";

/**
 * Highlighted tokens for each line. `lines` is null while the grammar loads or
 * when the language is unknown; `pending` tells the two apart.
 */
export function useHighlightedLines(
  code: string,
  language: string | null,
): { lines: HighlightToken[][] | null; pending: boolean } {
  const key = `${language ?? ""}\n${code}`;
  const [result, setResult] = useState<{ key: string; lines: HighlightToken[][] | null } | null>(
    null,
  );

  useEffect(() => {
    let cancelled = false;
    const apply = (dark: boolean) => {
      void highlightLines(code, language, dark).then((lines) => {
        if (!cancelled) {
          setResult({ key: `${language ?? ""}\n${code}`, lines });
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

  const current = result?.key === key ? result : null;
  return { lines: current?.lines ?? null, pending: current === null };
}
