import { useEffect, useState, type ReactNode } from "react";

import { identifierPattern, markIdentifiers } from "../contextCode";
import { highlightCode, prefersDark } from "../highlight";

export function CodeHighlight({
  code,
  language,
  className,
  marks = [],
}: {
  code: string;
  language: string | null;
  className?: string;
  /** Identifiers to flag inside the code, such as fields the new definition dropped. */
  marks?: string[];
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
    return (
      <div
        className={className}
        dangerouslySetInnerHTML={{ __html: markIdentifiers(current.html, marks) }}
      />
    );
  }

  return (
    <pre
      className={`code-plain ${className ?? ""}`}
      data-gewu-pending={current ? undefined : "highlight"}
    >
      {markedText(code, marks)}
    </pre>
  );
}

function markedText(code: string, marks: string[]): ReactNode {
  if (marks.length === 0) {
    return code;
  }
  const pattern = identifierPattern(marks);
  const parts: ReactNode[] = [];
  let last = 0;
  for (const match of code.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > last) {
      parts.push(code.slice(last, index));
    }
    parts.push(
      <span className="dropped-field" title="Dropped" key={index}>
        {match[1]}
      </span>,
    );
    last = index + match[0].length;
  }
  if (last < code.length) {
    parts.push(code.slice(last));
  }
  return parts;
}
