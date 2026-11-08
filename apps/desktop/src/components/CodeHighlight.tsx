import { useEffect, useState } from "react";

import { highlightCode, prefersDark } from "../highlight";

export function CodeHighlight({
  code,
  language,
  className,
}: {
  code: string;
  language: string | null;
  className?: string;
}) {
  const [html, setHtml] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const dark = prefersDark();
    void highlightCode(code, language, dark).then((result) => {
      if (!cancelled) {
        setHtml(result);
      }
    });
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      void highlightCode(code, language, media.matches).then((result) => {
        if (!cancelled) {
          setHtml(result);
        }
      });
    };
    media.addEventListener("change", onChange);
    return () => {
      cancelled = true;
      media.removeEventListener("change", onChange);
    };
  }, [code, language]);

  if (html) {
    return (
      <div
        className={className}
        dangerouslySetInnerHTML={{ __html: html }}
      />
    );
  }

  return <pre className={`code-plain ${className ?? ""}`}>{code}</pre>;
}
