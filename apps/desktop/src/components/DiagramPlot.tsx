import DOMPurify from "dompurify";
import { useEffect, useId, useRef, useState } from "react";

import { prefersDark } from "../highlight";

/** Renders a Mermaid diagram. Text that is not Mermaid stays readable as source. */
export function DiagramPlot({ source }: { source: string }) {
  const reactId = useId().replace(/:/g, "");
  const serial = useRef(0);
  const [drawing, setDrawing] = useState<{ source: string; svg: string | null } | null>(null);

  useEffect(() => {
    let cancelled = false;
    serial.current += 1;
    const renderId = `gewu-${reactId}-${serial.current}`;
    void import("mermaid")
      .then(async ({ default: mermaid }) => {
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: prefersDark() ? "dark" : "neutral",
          fontFamily: "inherit",
        });
        const rendered = await mermaid.render(renderId, source);
        const clean = DOMPurify.sanitize(rendered.svg, {
          USE_PROFILES: { svg: true, svgFilters: true },
        });
        if (!cancelled) {
          setDrawing({ source, svg: clean });
        }
      })
      .catch(() => {
        if (!cancelled) {
          setDrawing({ source, svg: null });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [reactId, source]);

  const ready = drawing?.source === source ? drawing : null;
  if (ready?.svg) {
    return <div className="illustration-plot" dangerouslySetInnerHTML={{ __html: ready.svg }} />;
  }
  if (ready && !ready.svg) {
    return (
      <div className="illustration-ascii">
        <p className="muted">Shown as text because this is not a Mermaid diagram.</p>
        <pre>{source}</pre>
      </div>
    );
  }
  return <p className="muted">Drawing the diagram…</p>;
}
