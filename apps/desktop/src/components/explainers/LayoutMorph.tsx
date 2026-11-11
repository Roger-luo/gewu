import { animate, AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";

import type { LayoutField } from "../../explainer";

type Row = { label: string; fields: LayoutField[] };

/** Seconds one change takes: mark the dropped fields, then collapse them. */
const STEP = 1.8;
const DROPPED = "color-mix(in srgb, var(--danger) 55%, var(--bg-elevated))";

/**
 * One record that changes in front of the reader: fields the next row drops
 * collapse out, fields both rows keep stay at their exact width, and the byte
 * total counts to the new size. Widths are shares of the largest record, so
 * every frame is drawn to scale. Plays once on arrival; Replay and the row
 * buttons step through it again.
 */
export function LayoutMorph({ rows, widest }: { rows: Row[]; widest: number }) {
  const [phase, setPhase] = useState(0);
  const reduced = useReducedMotion();
  const total = (index: number) => rows[index].fields.reduce((sum, field) => sum + field.bytes, 0);
  const [shown, setShown] = useState(total(0));
  const counted = useRef(total(0));

  // Play once on arrival: hold "before" long enough to read, then change.
  useEffect(() => {
    if (rows.length < 2) return;
    const timer = setTimeout(() => setPhase(rows.length - 1), reduced ? 0 : 1200);
    return () => clearTimeout(timer);
  }, [rows.length, reduced]);

  useEffect(() => {
    const to = total(phase);
    const controls = animate(counted.current, to, {
      duration: reduced ? 0 : STEP * 0.6,
      delay: reduced ? 0 : STEP * 0.4,
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
    setPhase(0);
    setTimeout(() => setPhase(rows.length - 1), reduced ? 0 : 1200);
  };
  const current = rows[phase];

  return (
    <div className="layout-morph">
      <div className="layout-morph-controls" role="group" aria-label="Show record">
        {rows.map((row, index) => (
          <button
            key={row.label}
            type="button"
            className="btn small"
            aria-pressed={phase === index}
            onClick={() => setPhase(index)}
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
          <AnimatePresence initial={false}>
            {current.fields.map((field) => {
              const width = `${(field.bytes / widest) * 100}%`;
              return (
                <motion.span
                  key={field.name}
                  layout
                  className="layout-field"
                  initial={{ width: 0, opacity: 0 }}
                  animate={{ width, opacity: 1 }}
                  // Dropped fields are marked first, then collapse, so the
                  // reader sees what went before seeing the record shrink.
                  exit={{
                    width: [width, width, "0%"],
                    opacity: [1, 1, 0],
                    backgroundColor: [DROPPED, DROPPED, DROPPED],
                  }}
                  transition={{
                    duration: STEP,
                    times: [0, 0.4, 1],
                    ease: "easeInOut",
                    layout: { duration: STEP * 0.6, delay: STEP * 0.4, ease: "easeInOut" },
                  }}
                  title={`${field.name}: ${field.bytes} B`}
                >
                  {field.bytes / widest >= 0.15 && (
                    <>
                      <span className="layout-field-name">{field.name}</span>
                      <span className="layout-dropped-mark" aria-hidden>
                        dropped
                      </span>
                    </>
                  )}
                </motion.span>
              );
            })}
          </AnimatePresence>
        </div>
        <span className="layout-morph-total">{shown} B</span>
      </div>
      <p className="sr-only" aria-live="polite">
        {current.label}: {total(phase)} bytes, {current.fields.map((field) => field.name).join(", ")}
      </p>
    </div>
  );
}
