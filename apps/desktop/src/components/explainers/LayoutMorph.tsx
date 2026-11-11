import { animate, motion, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";

import type { LayoutField } from "../../explainer";

type Row = { label: string; fields: LayoutField[] };

/** Seconds one change takes: mark the dropped fields, then collapse them. */
const STEP = 1.8;
/** Share of `STEP` spent marking before anything moves. */
const MARK = 0.4;
const DROPPED = "color-mix(in srgb, var(--danger) 55%, var(--bg-elevated))";
/** Resting fills by phase and position: every animation ends on one of these. */
const REST = {
  before: ["var(--before-fill)", "var(--before-fill-alt)"],
  after: ["var(--after-fill)", "var(--after-fill-alt)"],
};
/** How long Before stays on screen after Replay before it changes. */
const HOLD_MS = 1700;

/** Every field of every row, in first-seen order, so each keeps one element. */
function union(rows: Row[]): string[] {
  const names: string[] = [];
  for (const row of rows) {
    for (const field of row.fields) if (!names.includes(field.name)) names.push(field.name);
  }
  return names;
}

/**
 * One record that changes in front of the reader. Every field of every row
 * stays mounted and animates its own width, so replaying never remounts or
 * jumps: fields the next row drops are marked, then collapse; fields both
 * rows keep stay at their exact width. Widths are shares of the largest
 * record, so every frame is drawn to scale.
 */
export function LayoutMorph({ rows, widest }: { rows: Row[]; widest: number }) {
  // The row shown and the row it came from, so a field knows it is leaving.
  const [{ phase, from }, setState] = useState({ phase: 0, from: 0 });
  const reduced = useReducedMotion() ?? false;
  const names = union(rows);
  const total = (index: number) => rows[index].fields.reduce((sum, field) => sum + field.bytes, 0);
  const [shown, setShown] = useState(total(0));
  const counted = useRef(total(0));
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const goTo = (next: number) => setState((state) => ({ phase: next, from: state.phase }));

  // Play once on arrival: hold "before" long enough to read, then change.
  useEffect(() => {
    if (rows.length < 2) return;
    timer.current = setTimeout(() => goTo(rows.length - 1), 1400);
    return () => clearTimeout(timer.current);
  }, [rows.length]);

  useEffect(() => {
    const to = total(phase);
    const shrinking = to < counted.current;
    const controls = animate(counted.current, to, {
      duration: reduced ? 0 : STEP * (shrinking ? 1 - MARK : 0.6),
      delay: reduced || !shrinking ? 0 : STEP * MARK,
      ease: "easeInOut",
      onUpdate: (value) => {
        counted.current = value;
        setShown(Math.round(value));
      },
    });
    return () => controls.stop();
    // `total` reads `rows`, which is stable for one explainer.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, reduced]);

  const replay = () => {
    clearTimeout(timer.current);
    goTo(0);
    timer.current = setTimeout(() => goTo(rows.length - 1), HOLD_MS);
  };
  const current = rows[phase];
  const before = rows[from];

  return (
    <div className="layout-morph">
      <div className="layout-morph-controls" role="group" aria-label="Show record">
        {rows.map((row, index) => (
          <button
            key={row.label}
            type="button"
            className="btn small"
            aria-pressed={phase === index}
            onClick={() => {
              clearTimeout(timer.current);
              goTo(index);
            }}
          >
            {row.label}
          </button>
        ))}
        <button type="button" className="btn small" onClick={replay}>
          ↻ Replay
        </button>
      </div>
      <div className={`layout-morph-stage phase-${phase === 0 ? "before" : "after"}`} aria-hidden>
        <div className="layout-morph-track">
          {names.map((name, position) => {
            const rest = REST[phase === 0 ? "before" : "after"][position % 2];
            const field = current.fields.find((item) => item.name === name);
            const was = before.fields.find((item) => item.name === name);
            const width = field ? (field.bytes / widest) * 100 : 0;
            const leaving = !field && was !== undefined && !reduced;
            const roomy = (field ?? was) !== undefined && ((field ?? was)!.bytes / widest) >= 0.15;
            return (
              <motion.span
                key={name}
                className="layout-field"
                initial={false}
                animate={
                  leaving
                    ? {
                        width: [`${(was.bytes / widest) * 100}%`, `${(was.bytes / widest) * 100}%`, "0%"],
                        backgroundColor: [DROPPED, DROPPED, DROPPED],
                      }
                    : { width: `${width}%`, backgroundColor: rest }
                }
                transition={{
                  duration: reduced ? 0 : leaving ? STEP : STEP * 0.6,
                  times: leaving ? [0, MARK, 1] : undefined,
                  ease: "easeInOut",
                }}
                title={`${name}${field ? `: ${field.bytes} B` : ""}`}
              >
                {roomy && (
                  <motion.span
                    className="layout-field-name"
                    initial={false}
                    // The label leaves before the field narrows, so no
                    // clipped fragment is left inside a sliver.
                    // A label leaves before its field narrows, and returns only
                    // once a growing field has room, so no fragment shows.
                    animate={{ opacity: leaving ? [1, 1, 0] : field ? [0, 0, 1] : 0 }}
                    transition={{
                      duration: reduced ? 0 : leaving ? STEP * (MARK + 0.1) : STEP * 0.7,
                      times: leaving ? [0, 0.8, 1] : field ? [0, 0.85, 1] : undefined,
                    }}
                  >
                    {name}
                    {leaving && <span className="layout-dropped-mark visible"> dropped</span>}
                  </motion.span>
                )}
              </motion.span>
            );
          })}
        </div>
        <span className="layout-morph-total">{shown} B</span>
      </div>
      <p className="sr-only" aria-live="polite">
        {current.label}: {total(phase)} bytes, {current.fields.map((field) => field.name).join(", ")}
      </p>
    </div>
  );
}
