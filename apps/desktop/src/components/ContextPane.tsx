import {
  useLayoutEffect,
  useState,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import { droppedIdentifiers, layoutCode } from "../contextCode";
import type { ChunkContext } from "../protocol";
import { CodeHighlight } from "./CodeHighlight";
import { InlineMarkdown } from "./Markdown";

const WIDTH = { default: 560, min: 360, max: 840 };
const WIDTH_KEY = "gewu.context-width";

/** `language` is the language of the chunk's code, used for its examples. */
export function ContextPane({
  context,
  language,
}: {
  context: ChunkContext;
  language: string | null;
}) {
  const width = useContextWidth();
  const before = context.kind === "refactor" ? layoutCode(context.before) : "";
  const after = context.kind === "refactor" ? layoutCode(context.after) : "";
  const dropped =
    context.kind === "refactor" ? droppedIdentifiers(context.reason, before, after) : [];

  return (
    <aside className="context-pane" aria-label="Context">
      <div
        className="context-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Context pane width"
        aria-valuemin={WIDTH.min}
        aria-valuemax={WIDTH.max}
        aria-valuenow={width.value}
        tabIndex={0}
        onPointerDown={width.startDrag}
        onKeyDown={width.onKeyDown}
      />
      <h3>{context.kind === "concept" ? "Concept" : "Refactor"}</h3>
      {context.kind === "concept" ? (
        <>
          <div className="field">
            <div className="field-label">Definition</div>
            <div>
              <InlineMarkdown source={context.definition} />
            </div>
          </div>
          <div className="field">
            <div className="field-label">Example</div>
            <CodeHighlight code={context.example} language={language} className="context-code" />
          </div>
          <div className="field">
            <div className="field-label">Relation</div>
            <div>
              <InlineMarkdown source={context.relation} />
            </div>
          </div>
        </>
      ) : (
        <>
          <div className="field">
            <div className="field-label">Reason</div>
            <div>
              <InlineMarkdown source={context.reason} />
            </div>
          </div>
          <div className="field">
            <div className="field-label">Before</div>
            {dropped.length > 0 && (
              <p className="dropped-hint">Marked fields are not in the new definition.</p>
            )}
            <CodeHighlight
              code={before}
              language={language}
              className="context-code"
              marks={dropped}
            />
          </div>
          <div className="field">
            <div className="field-label">After</div>
            <CodeHighlight code={after} language={language} className="context-code" />
          </div>
        </>
      )}
    </aside>
  );
}

function useContextWidth() {
  const [value, setValue] = useState(readWidth);

  useLayoutEffect(() => {
    document.documentElement.style.setProperty("--context-width", `${value}px`);
    try {
      localStorage.setItem(WIDTH_KEY, String(value));
    } catch {
      // A private window can refuse storage; the drag still applies this visit.
    }
  }, [value]);

  const setWidth = (next: number) => setValue(clampWidth(next));

  return {
    value,
    startDrag(event: ReactPointerEvent<HTMLDivElement>) {
      event.preventDefault();
      const handle = event.currentTarget;
      handle.setPointerCapture(event.pointerId);
      const startX = event.clientX;
      const startWidth = value;
      const pair = centeredPair(handle);
      // Where on the handle it was grabbed, so the handle stays under the
      // pointer instead of jumping to it.
      const grab = startX - handle.getBoundingClientRect().left;
      const move = (moveEvent: PointerEvent) => {
        setWidth(
          pair
            ? pair.widthFor(moveEvent.clientX - grab)
            : startWidth - (moveEvent.clientX - startX),
        );
      };
      const stop = () => {
        handle.removeEventListener("pointermove", move);
        handle.removeEventListener("pointerup", stop);
      };
      handle.addEventListener("pointermove", move);
      handle.addEventListener("pointerup", stop);
    },
    onKeyDown(event: KeyboardEvent) {
      const step = event.shiftKey ? 48 : 16;
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        setWidth(value + step);
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        setWidth(value - step);
      }
    },
  };
}

function readWidth(): number {
  try {
    const stored = localStorage.getItem(WIDTH_KEY);
    if (stored) {
      return clampWidth(Number(stored));
    }
  } catch {
    // Fall through to the default.
  }
  return WIDTH.default;
}

function clampWidth(width: number): number {
  if (!Number.isFinite(width)) {
    return WIDTH.default;
  }
  return Math.min(WIDTH.max, Math.max(WIDTH.min, Math.round(width)));
}

/**
 * The pane's width that puts its left edge at `edge`, when the reading
 * column and the pane are centered as a pair. Widening the pane re-centers
 * the pair, so the edge moves half as far as the width changes; solving for
 * the width keeps the handle under the pointer. Null outside the layout.
 */
function centeredPair(handle: HTMLElement): { widthFor: (edge: number) => number } | null {
  const body = handle.closest(".app-body");
  const main = body?.querySelector<HTMLElement>("main[data-gewu-screen]");
  const column = main?.firstElementChild;
  if (!body || !main || !column) {
    return null;
  }
  const mainStyle = getComputedStyle(main);
  const reading =
    parseFloat(getComputedStyle(column).maxWidth) +
    parseFloat(mainStyle.paddingLeft) +
    parseFloat(mainStyle.paddingRight);
  if (!Number.isFinite(reading)) {
    return null;
  }
  const { left, width } = body.getBoundingClientRect();
  return {
    widthFor(edge: number) {
      const offset = edge - left;
      // Past the reading column the outer margins are open, and half of each
      // change in width comes out of the left margin.
      return offset > reading ? width + reading - 2 * offset : width - offset;
    },
  };
}
