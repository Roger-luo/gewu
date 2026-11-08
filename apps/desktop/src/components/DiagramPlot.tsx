import DOMPurify from "dompurify";
import { useEffect, useId, useRef, useState } from "react";

import { readableSvg } from "../diagram";
import { prefersDark } from "../highlight";

/**
 * Renders a Mermaid diagram. Text that is not Mermaid stays readable as source.
 * `label` names the drawing for assistive technology, which cannot read the SVG.
 */
export function DiagramPlot({ source, label }: { source: string; label: string }) {
  const reactId = useId().replace(/:/g, "");
  const serial = useRef(0);
  const [drawing, setDrawing] = useState<{ source: string; svg: string | null } | null>(null);

  useEffect(() => {
    let cancelled = false;
    serial.current += 1;
    const renderId = `gewu-${reactId}-${serial.current}`;
    void import("mermaid")
      .then(async ({ default: mermaid }) => {
        // Labels must be SVG text: the sanitizer drops the foreignObject
        // that HTML labels need, which leaves empty boxes.
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: prefersDark() ? "dark" : "neutral",
          fontFamily: "inherit",
          fontSize: 16,
          suppressErrorRendering: true,
          htmlLabels: false,
          flowchart: { htmlLabels: false },
        });
        if (!(await mermaid.parse(source, { suppressErrors: true }))) {
          throw new Error("not a Mermaid diagram");
        }
        const rendered = await mermaid.render(renderId, source).finally(() => {
          document.getElementById(`d${renderId}`)?.remove();
        });
        const clean = readableSvg(
          DOMPurify.sanitize(rendered.svg, {
            USE_PROFILES: { svg: true, svgFilters: true },
          }),
        );
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
    return (
      <div
        className="illustration-plot"
        role="img"
        aria-label={label}
        dangerouslySetInnerHTML={{ __html: ready.svg }}
      />
    );
  }
  if (ready && !ready.svg) {
    return (
      <div className="illustration-ascii">
        <p className="muted">Shown as text because this is not a Mermaid diagram.</p>
        <pre>{source}</pre>
      </div>
    );
  }
  return (
    <p className="muted" data-gewu-pending="diagram">
      Drawing the diagram…
    </p>
  );
}
