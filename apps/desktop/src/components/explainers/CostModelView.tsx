import { motion, useReducedMotion } from "motion/react";
import { useEffect, useId, useMemo, useRef, useState } from "react";

import {
  compileFormula,
  formatValue,
  paramSteps,
  plainTitle,
  type Explainer,
  type Param,
} from "../../explainer";
import { InlineMarkdown, Markdown } from "../Markdown";

type CostModel = Extract<Explainer, { kind: "cost_model" }>;

/**
 * A cost the reviewer can recompute: drag a parameter and every series is
 * evaluated again from its formula, so the claim's arithmetic is checkable.
 */
export function CostModelView({ explainer }: { explainer: CostModel }) {
  const reduced = useReducedMotion() ?? false;
  const [values, setValues] = useState(() => explainer.params.map((param) => param.default));
  const compiled = useMemo(() => {
    const names = explainer.params.map((param) => param.id);
    return explainer.series.map((series) => {
      try {
        return compileFormula(series.formula, names);
      } catch {
        return () => NaN;
      }
    });
  }, [explainer]);
  const results = compiled.map((expr) => expr(values));
  const sweep = useSweep(explainer.params[0], (value) =>
    setValues((current) => current.map((old, at) => (at === 0 ? value : old))),
  );
  // Bars share one scale across the sizes worth comparing — the default and
  // the measured marks — so dragging between them grows and shrinks the
  // bars; going past them stretches the scale with it.
  const largest = useMemo(() => {
    const first = explainer.params[0];
    const sizes = [first?.default, ...(first?.marks ?? [])].filter((v): v is number => v !== undefined);
    const points = sizes.map((size) => explainer.params.map((p, at) => (at === 0 ? size : p.default)));
    const all = compiled.flatMap((expr) => [...points.map((point) => expr(point)), expr(values)]);
    return Math.max(...all.filter(Number.isFinite), 1);
  }, [compiled, explainer.params, values]);
  const setParam = (index: number, value: number) => {
    sweep.stop();
    setValues((current) => current.map((old, at) => (at === index ? value : old)));
  };

  return (
    <figure className="explainer cost-model" aria-label={plainTitle(explainer.title)}>
      <figcaption className="explainer-label">
        <InlineMarkdown source={explainer.title} />
      </figcaption>
      <div className="cost-params">
        {explainer.params.map((param, index) => (
          <ParamSlider
            key={param.id}
            param={param}
            value={values[index]}
            onChange={(value) => setParam(index, value)}
          />
        ))}
      </div>
      {sweep.available && (
        <div className="cost-sweep">
          <button type="button" className="btn small" aria-pressed={sweep.playing} onClick={sweep.toggle}>
            {sweep.playing ? "❚❚ Pause" : "▶ Play through the sizes"}
          </button>
        </div>
      )}
      <div className="cost-series" role="list" aria-label="Results">
        {explainer.series.map((series, index) => (
          <div className="cost-row" role="listitem" key={series.label}>
            <span className="cost-label">{series.label}</span>
            <div className="cost-track" aria-hidden>
              <motion.div
                className={`cost-bar series-${index}`}
                animate={{ width: `${Math.max(0.5, (results[index] / largest) * 100)}%` }}
                transition={reduced ? { duration: 0 } : { type: "spring", stiffness: 260, damping: 30 }}
              />
            </div>
            <output className="cost-value">{formatValue(results[index], series.unit)}</output>
          </div>
        ))}
      </div>
      {results.length === 2 && results[1] > 0 && Number.isFinite(results[0] / results[1]) && (
        <p className="explainer-verdict" aria-live="polite">
          {explainer.series[1].label}: <strong>{ratioText(results[0], results[1])}</strong> than{" "}
          {explainer.series[0].label.toLowerCase()}.
        </p>
      )}
      <details className="cost-formulas">
        <summary>How it is computed</summary>
        <ul>
          {explainer.series.map((series) => (
            <li key={series.label}>
              {series.label}: <code>{series.formula}</code>
            </li>
          ))}
        </ul>
      </details>
      {explainer.note && (
        <div className="explainer-note muted">
          <Markdown source={explainer.note} />
        </div>
      )}
      <p className="explainer-source muted">Source: {explainer.source}</p>
    </figure>
  );
}

/**
 * Steps the first parameter from its minimum to its maximum so the bars grow
 * in front of the reader, pausing longer on its marks (the measured sizes).
 */
function useSweep(param: Param | undefined, set: (value: number) => void) {
  const [playing, setPlaying] = useState(false);
  const setter = useRef(set);
  useEffect(() => {
    setter.current = set;
  });
  const values = useMemo(() => {
    if (!param) return [];
    const steps = paramSteps(param);
    if (steps) return steps;
    const count = 12;
    return Array.from({ length: count + 1 }, (_, i) =>
      Math.round(param.min + ((param.max - param.min) * i) / count),
    );
  }, [param]);

  useEffect(() => {
    if (!playing || !param) return;
    let index = 0;
    let timer: ReturnType<typeof setTimeout>;
    const step = () => {
      const value = values[index];
      setter.current(value);
      index += 1;
      if (index >= values.length) {
        setPlaying(false);
        return;
      }
      const pause = (param.marks ?? []).includes(value) ? 1400 : 550;
      // Reading time, kept with reduced motion: only the bars stop gliding.
      timer = setTimeout(step, pause);
    };
    step();
    return () => clearTimeout(timer);
  }, [playing, param, values]);

  return {
    available: values.length > 2,
    playing,
    toggle: () => setPlaying((on) => !on),
    stop: () => setPlaying(false),
  };
}

function ratioText(before: number, after: number): string {
  if (after < before) {
    const less = (1 - after / before) * 100;
    return `${less.toFixed(0)}% less (${(before / after).toFixed(1)}× smaller)`;
  }
  return `${((after / before - 1) * 100).toFixed(0)}% more`;
}

function ParamSlider({
  param,
  value,
  onChange,
}: {
  param: Param;
  value: number;
  onChange: (value: number) => void;
}) {
  const id = useId();
  const steps = paramSteps(param);
  const position = steps ? Math.max(0, steps.indexOf(value)) : value;
  const label = (v: number) => (steps ? `2^${Math.log2(v)}` : formatValue(v));
  const shown = steps ? `2^${Math.log2(value)} = ${formatValue(value)}` : formatValue(value);
  const marks = param.marks ?? [];
  return (
    <div className="cost-param">
      <label htmlFor={id}>{param.label}</label>
      <input
        id={id}
        type="range"
        min={steps ? 0 : param.min}
        max={steps ? steps.length - 1 : param.max}
        step={1}
        value={position}
        // A log2 slider moves through step positions; say the value itself.
        aria-valuemin={param.min}
        aria-valuemax={param.max}
        aria-valuenow={value}
        aria-valuetext={shown}
        onChange={(event) => {
          const raw = Number(event.target.value);
          onChange(steps ? (steps[raw] ?? param.default) : raw);
        }}
      />
      <output htmlFor={id} className="cost-param-value">
        {shown}
      </output>
      {marks.length > 0 && (
        <div className="cost-marks">
          <span className="muted">Measured at</span>
          {marks.map((mark) => (
            <button
              type="button"
              key={mark}
              className="btn small cost-mark"
              aria-pressed={mark === value}
              onClick={() => onChange(mark)}
            >
              {label(mark)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
