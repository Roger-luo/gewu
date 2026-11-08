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
  const key = `${language ?? ""}\n${code}`;
  const [result, setResult] = useState<{ key: string; html: string | null } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const apply = (dark: boolean) => {
      void highlightCode(code, language, dark).then((html) => {
        if (!cancelled) {
          setResult({ key: `${language ?? ""}\n${code}`, html });
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
  if (current?.html) {
    return <div className={className} dangerouslySetInnerHTML={{ __html: current.html }} />;
  }

  return (
    <pre
      className={`code-plain ${className ?? ""}`}
      data-gewu-pending={current ? undefined : "highlight"}
    >
      {code}
    </pre>
  );
}
