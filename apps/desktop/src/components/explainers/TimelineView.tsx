import { motion, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";

import { plainTitle, unitFor, type Explainer, type Lane } from "../../explainer";
import { InlineMarkdown } from "../Markdown";

type Timeline = Extract<Explainer, { kind: "timeline" }>;

/** Total cost of a lane up to and including `through` steps. */
function costThrough(lane: Lane, through: number): number {
  return lane.work.reduce((sum, work) => {
    if (work.cost === undefined) return sum;
    return sum + (work.when === "once" ? (through > 0 ? work.cost : 0) : work.cost * through);
  }, 0);
}

/**
 * Work done in every step versus once: each lane is a row of steps, a
 * playhead walks across them, and the running cost of each lane counts up.
 */
export function TimelineView({ explainer }: { explainer: Timeline }) {
  // At rest the timeline shows the finished picture, so a reader who never
  // presses Play, or a screenshot, sees the true totals. Scrolling it into
  // view replays it from the first step.
  const [through, setThrough] = useState(explainer.steps);
  const [playing, setPlaying] = useState(false);
  const reduced = useReducedMotion() ?? false;
  const figure = useRef<HTMLElement>(null);
  useEffect(() => {
    const element = figure.current;
    if (!element || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          observer.disconnect();
          setThrough(0);
          setPlaying(true);
        }
      },
      { threshold: 0.5 },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const counted = explainer.lanes.some((lane) => lane.work.some((work) => work.cost !== undefined));

  // Playing ends by itself at the last step; no state to reset.
  const running = playing && through < explainer.steps;
  useEffect(() => {
    if (!running) return;
    // Reading time, kept with reduced motion: only the cells stop easing.
    const timer = setTimeout(() => setThrough((at) => at + 1), through === 0 ? 700 : 650);
    return () => clearTimeout(timer);
  }, [running, through]);

  const steps = Array.from({ length: explainer.steps }, (_, at) => at + 1);
  return (
    <figure ref={figure} className="explainer timeline" aria-label={plainTitle(explainer.title)}>
      <figcaption className="explainer-label">
        <InlineMarkdown source={explainer.title} />
      </figcaption>
      <div className="timeline-grid" style={{ "--steps": explainer.steps } as React.CSSProperties}>
        <span className="timeline-step-head">{explainer.step_label} →</span>
        {steps.map((step) => (
          <span key={step} className={step <= through ? "timeline-step reached" : "timeline-step"}>
            {step}
          </span>
        ))}
        <span className="timeline-total-head">{counted ? `Total ${explainer.unit ?? ""}` : ""}</span>
        {explainer.lanes.map((lane) => (
          <TimelineLane
            key={lane.label}
            lane={lane}
            steps={steps}
            through={through}
            running={running}
            reduced={reduced}
            counted={counted}
            unit={explainer.unit}
          />
        ))}
      </div>
      <div className="stepper-controls">
        <button
          type="button"
          className="btn small"
          aria-pressed={running}
          onClick={() => {
            if (through >= explainer.steps) {
              setThrough(0);
              setPlaying(true);
            } else {
              setPlaying(!running);
            }
          }}
        >
          {running ? "❚❚ Pause" : through >= explainer.steps ? "↻ Replay" : "▶ Play"}
        </button>
        <span className="muted timeline-legend">
          <span className="timeline-chip each" /> done every step
          <span className="timeline-chip once" /> done once
          <span className="timeline-chip reused" /> reused
        </span>
      </div>
      <p className="sr-only">
        {explainer.lanes
          .map((lane) => `${lane.label}: ${lane.work.map((work) => `${work.label} ${work.when === "once" ? "once" : "every step"}`).join(", ")}${counted ? `, ${costThrough(lane, explainer.steps)} ${explainer.unit ?? ""} over ${explainer.steps} steps` : ""}`)
          .join(". ")}
      </p>
    </figure>
  );
}

function TimelineLane({
  lane,
  steps,
  through,
  running,
  reduced,
  counted,
  unit,
}: {
  lane: Lane;
  steps: number[];
  through: number;
  running: boolean;
  reduced: boolean;
  counted: boolean;
  unit?: string;
}) {
  return (
    <>
      <span className="timeline-lane-label">
        {lane.label}
        <span className="timeline-lane-work">
          {lane.work.map((work) => (
            <span key={work.label}>
              <span className={`timeline-chip ${work.when}`} />
              {work.label}
              {work.cost !== undefined && ` · ${work.cost} ${unitFor(work.cost, unit)}`}
              {work.when === "once" ? ", once" : ", each step"}
            </span>
          ))}
        </span>
      </span>
      {steps.map((step) => (
        <span key={step} className="timeline-cell">
          {lane.work.map((work) => {
            // Work done once shows as a faint "reused" chip in later steps,
            // so the After lane reads as reuse, not as nothing.
            const reused = work.when === "once" && step > 1;
            return (
              <motion.span
                key={work.label}
                aria-hidden
                className={`timeline-work ${reused ? "reused" : work.when}`}
                title={`${work.label}${reused ? " (reused)" : ""}${work.cost !== undefined && !reused ? `: ${work.cost}` : ""}`}
                initial={false}
                animate={{
                  opacity: step <= through ? 1 : 0.18,
                  scale: running && step === through ? 1.06 : 1,
                }}
                transition={{ duration: reduced ? 0 : 0.25 }}
              />
            );
          })}
        </span>
      ))}
      <span className="timeline-total">{counted ? costThrough(lane, through) : ""}</span>
    </>
  );
}
