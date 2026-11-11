import { ShikiMagicMove } from "@shikijs/magic-move/react";
import "@shikijs/magic-move/style.css";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useEffect, useMemo, useRef, useState } from "react";

import { lineRuns, plainTitle, stepCodes, type Explainer } from "../../explainer";
import { loadHighlighter, prefersDark } from "../../highlight";
import { InlineMarkdown, Markdown } from "../Markdown";

type Stepper = Extract<Explainer, { kind: "stepper" }>;
type Loaded = Awaited<ReturnType<typeof loadHighlighter>>;

/** How long Play holds each step: reading time, kept even with reduced motion. */
const HOLD_MS = 2600;

/**
 * An algorithm one state at a time: the code morphs between steps, a band
 * glides to the lines that run now, and values that changed are marked. The
 * gutter uses the file's own line numbers (`start_line`). Back, Next, and
 * Play move through it; nothing moves until the reader asks.
 */
export function StepperView({ explainer }: { explainer: Stepper }) {
  const [index, setIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [loaded, setLoaded] = useState<Loaded>(null);
  const reduced = useReducedMotion() ?? false;
  const back = useRef<HTMLButtonElement>(null);
  const play = useRef<HTMLButtonElement>(null);
  const next = useRef<HTMLButtonElement>(null);
  const box = useRef<HTMLDivElement>(null);
  // Whether code runs past the box's right edge, for the fade that says so.
  const [clipped, setClipped] = useState(false);
  useEffect(() => {
    const element = box.current;
    if (!element || typeof ResizeObserver === "undefined") return;
    const measure = () =>
      setClipped(element.scrollWidth - element.scrollLeft - element.clientWidth > 2);
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    element.addEventListener("scroll", measure, { passive: true });
    return () => {
      observer.disconnect();
      element.removeEventListener("scroll", measure);
    };
  }, []);
  const codes = useMemo(() => stepCodes(explainer.steps), [explainer.steps]);
  const first = explainer.start_line ?? 1;
  const step = explainer.steps[index];
  const code = codes[index];
  const runs = lineRuns(step.highlight);
  const last = explainer.steps.length - 1;
  const previous = index > 0 ? (explainer.steps[index - 1].state ?? []) : [];
  const lineCount = code.split("\n").length;

  useEffect(() => {
    let live = true;
    void loadHighlighter(explainer.language).then((result) => live && setLoaded(result));
    return () => {
      live = false;
    };
  }, [explainer.language]);

  // Playing ends by itself on the last step.
  const running = playing && index < last;
  useEffect(() => {
    if (!running) return;
    const timer = setTimeout(() => setIndex((at) => Math.min(at + 1, last)), HOLD_MS);
    return () => clearTimeout(timer);
  }, [running, index, last]);

  /** Moves to `to`; when the pressed button becomes disabled, focus moves on. */
  const go = (to: number) => {
    setPlaying(false);
    setIndex(to);
    if (to === last) requestAnimationFrame(() => play.current?.focus());
    if (to === 0) requestAnimationFrame(() => next.current?.focus());
  };

  return (
    <figure className="explainer stepper" aria-label={plainTitle(explainer.title)}>
      <figcaption className="explainer-label">
        <InlineMarkdown source={explainer.title} />
      </figcaption>
      <div className="stepper-body">
        <div className="stepper-main">
          <div
            ref={box}
            className={clipped ? "stepper-code clipped" : "stepper-code"}
            // Long lines scroll sideways; the region must be reachable by keyboard.
            tabIndex={0}
            role="region"
            aria-label={`Code for ${plainTitle(explainer.title)}`}
          >
            {explainer.source && <div className="stepper-source">{explainer.source}</div>}
            <div className="stepper-lines">
              <div className="stepper-gutter" aria-hidden>
                {Array.from({ length: lineCount }, (_, at) => (
                  <span key={at}>{first + at}</span>
                ))}
              </div>
              <div className="stepper-text">
                <div className="stepper-bands" aria-hidden>
                  {runs.map((run, at) => (
                    <motion.span
                      key={at}
                      className="stepper-band"
                      initial={false}
                      animate={{
                        top: `calc(${run.start - first} * var(--line-height))`,
                        height: `calc(${run.length} * var(--line-height))`,
                      }}
                      transition={{ duration: reduced ? 0 : 0.35, ease: "easeInOut" }}
                    />
                  ))}
                </div>
                {loaded ? (
                  <ShikiMagicMove
                    highlighter={loaded.highlighter}
                    lang={loaded.lang}
                    theme={prefersDark() ? loaded.dark : loaded.light}
                    code={code}
                    options={{ duration: reduced ? 0 : 700, stagger: reduced ? 0 : 0.3, lineNumbers: false }}
                    className="stepper-shiki"
                  />
                ) : (
                  <pre className="stepper-plain">{code}</pre>
                )}
              </div>
            </div>
          </div>
          <div className="stepper-controls">
            <button ref={back} type="button" className="btn small" disabled={index === 0} onClick={() => go(index - 1)}>
              ← Back
            </button>
            <button
              ref={play}
              type="button"
              className="btn small"
              aria-pressed={running}
              onClick={() => {
                if (index >= last) {
                  setIndex(0);
                  setPlaying(true);
                } else {
                  setPlaying(!running);
                }
              }}
            >
              {running ? "❚❚ Pause" : index >= last ? "↻ Play again" : "▶ Play"}
            </button>
            <button
              ref={next}
              type="button"
              className="btn small primary"
              disabled={index === last}
              onClick={() => go(index + 1)}
            >
              Next →
            </button>
            <span className="stepper-dots" aria-hidden>
              {explainer.steps.map((_, at) => (
                <span
                  key={at}
                  className={at === index ? "stepper-dot current" : at < index ? "stepper-dot seen" : "stepper-dot"}
                />
              ))}
            </span>
          </div>
        </div>
        <div className="stepper-side">
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={index}
              className="stepper-caption"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: reduced ? 0 : 0.2 }}
              aria-live="polite"
            >
              <p className="stepper-count muted">
                Step {index + 1} of {explainer.steps.length}
                {step.highlight ? ` · ${/[-,]/.test(step.highlight) ? "lines" : "line"} ${step.highlight}` : ""}
              </p>
              <Markdown source={step.caption} />
            </motion.div>
          </AnimatePresence>
          {(step.state?.length ?? 0) > 0 && (
            <table className="stepper-state">
              <tbody>
                {step.state?.map((value) => {
                  const changed = previous.find((old) => old.name === value.name)?.value !== value.value;
                  return (
                    <tr key={value.name}>
                      <th scope="row">
                        <code>{value.name}</code>
                      </th>
                      <td>
                        <code className={changed ? "stepper-changed" : undefined}>{value.value}</code>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </figure>
  );
}
