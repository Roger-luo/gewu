/**
 * Explainers as the app receives them, and the formula arithmetic cost
 * models are written in. The grammar matches `gewu_review::explainer::formula`
 * in Rust, which validates every formula before it reaches the app.
 */

export type Unit = "bytes" | "count" | "seconds";

export interface LayoutField {
  name: string;
  bytes: number;
  note?: string;
}

export interface Param {
  id: string;
  label: string;
  min: number;
  max: number;
  default: number;
  scale?: "linear" | "log2";
  /** Values worth landing on, such as the sizes a benchmark measured. */
  marks?: number[];
}

export interface Series {
  label: string;
  formula: string;
  unit?: Unit;
}

export interface Step {
  caption: string;
  code?: string;
  highlight?: string;
  state?: { name: string; value: string }[];
}

export interface Work {
  label: string;
  when: "once" | "each";
  cost?: number;
}

export interface Lane {
  label: string;
  work: Work[];
}

export type Explainer =
  | { kind: "claim"; claim: string; result?: string; share?: string; how: string }
  | { kind: "layout"; title: string; rows: { label: string; fields: LayoutField[] }[]; source: string }
  | {
      kind: "cost_model";
      title: string;
      params: Param[];
      series: Series[];
      note?: string;
      source: string;
    }
  | {
      kind: "stepper";
      title: string;
      language?: string;
      source?: string;
      start_line?: number;
      steps: Step[];
    }
  | { kind: "timeline"; title: string; steps: number; step_label: string; lanes: Lane[]; unit?: string };

/** "3", "3-5", or "2,6-7" as a set of 1-based line numbers. */
export function lineSet(text: string | undefined): Set<number> {
  const lines = new Set<number>();
  for (const part of (text ?? "").split(",")) {
    const [start, end] = part.split("-").map((n) => Number(n.trim()));
    if (!Number.isInteger(start) || start < 1) continue;
    for (let line = start; line <= (Number.isInteger(end) ? end : start); line += 1) lines.add(line);
  }
  return lines;
}

/** "1 allocations" reads wrong: drop a plural `s` when the count is one. */
export function unitFor(count: number, unit: string | undefined): string {
  if (!unit) return "";
  return count === 1 && unit.endsWith("s") ? unit.slice(0, -1) : unit;
}

/** "38-40,42" as contiguous runs of lines: one highlight band each. */
export function lineRuns(text: string | undefined): { start: number; length: number }[] {
  const lines = [...lineSet(text)].sort((a, b) => a - b);
  const runs: { start: number; length: number }[] = [];
  for (const line of lines) {
    const last = runs[runs.length - 1];
    if (last && last.start + last.length === line) last.length += 1;
    else runs.push({ start: line, length: 1 });
  }
  return runs;
}

/** Each step's code: a step without `code` keeps the previous step's. */
export function stepCodes(steps: Step[]): string[] {
  let code = "";
  return steps.map((step) => (code = step.code ?? code));
}

type Expr = (values: number[]) => number;

const FUNCTIONS: Record<string, [number, (a: number, b: number) => number]> = {
  log2: [1, (a) => Math.log2(a)],
  ceil: [1, (a) => Math.ceil(a)],
  floor: [1, (a) => Math.floor(a)],
  min: [2, (a, b) => Math.min(a, b)],
  max: [2, (a, b) => Math.max(a, b)],
  next_pow2: [1, (a) => 2 ** Math.max(0, Math.ceil(Math.log2(a)))],
};

/** Compiles `text`; `params` are the only names it may use. Throws on a bad formula. */
export function compileFormula(text: string, params: string[]): Expr {
  const chars = [...text].filter((c) => !/\s/.test(c));
  let at = 0;
  const peek = () => chars[at];
  const eat = (c: string) => (peek() === c ? (at += 1, true) : false);
  const fail = (message: string): never => {
    throw new Error(message);
  };

  const sum = (): Expr => {
    let left = product();
    while (peek() === "+" || peek() === "-") {
      const op = chars[at++];
      const [l, r] = [left, product()];
      left = op === "+" ? (v) => l(v) + r(v) : (v) => l(v) - r(v);
    }
    return left;
  };
  const product = (): Expr => {
    let left = unary();
    while (peek() === "*" || peek() === "/") {
      const op = chars[at++];
      const [l, r] = [left, unary()];
      left = op === "*" ? (v) => l(v) * r(v) : (v) => l(v) / r(v);
    }
    return left;
  };
  const unary = (): Expr => {
    if (eat("-")) {
      const inner = unary();
      return (v) => -inner(v);
    }
    return power();
  };
  const power = (): Expr => {
    const base = primary();
    if (eat("^")) {
      const exponent = unary();
      return (v) => base(v) ** exponent(v);
    }
    return base;
  };
  const primary = (): Expr => {
    const c = peek();
    if (c === undefined) return fail("the formula ends too early");
    if (eat("(")) {
      const inner = sum();
      if (!eat(")")) fail("a `(` is never closed");
      return inner;
    }
    if (/[0-9.]/.test(c)) {
      let digits = "";
      while (peek() !== undefined && /[0-9.]/.test(peek() as string)) digits += chars[at++];
      const value = Number(digits);
      if (Number.isNaN(value)) fail(`\`${digits}\` is not a number`);
      return () => value;
    }
    if (/[A-Za-z_]/.test(c)) {
      let name = "";
      while (peek() !== undefined && /[A-Za-z0-9_]/.test(peek() as string)) name += chars[at++];
      if (eat("(")) {
        const known = FUNCTIONS[name];
        if (!known) return fail(`unknown function \`${name}\``);
        const args: Expr[] = [];
        if (!eat(")")) {
          for (;;) {
            args.push(sum());
            if (eat(")")) break;
            if (!eat(",")) fail(`\`${name}(\` is never closed`);
          }
        }
        if (args.length !== known[0]) fail(`\`${name}\` takes ${known[0]} argument(s), got ${args.length}`);
        const [fn] = [known[1]];
        const [a, b] = args;
        return (v) => fn(a(v), b ? b(v) : NaN);
      }
      const index = params.indexOf(name);
      if (index < 0) fail(`\`${name}\` is not a declared parameter`);
      return (v) => v[index] ?? NaN;
    }
    return fail(`unexpected \`${c}\``);
  };

  if (chars.length === 0) fail("the formula is empty");
  const expr = sum();
  if (at < chars.length) fail(`unexpected \`${chars[at]}\``);
  return expr;
}

/** A value in its unit: bytes in KiB, MiB, or GiB; counts with separators. */
export function formatValue(value: number, unit: Unit = "count"): string {
  if (!Number.isFinite(value)) return "not computable";
  if (unit === "seconds") return `${value.toFixed(2)} s`;
  if (unit === "count") return Math.round(value).toLocaleString("en-US");
  let amount = value;
  for (const suffix of ["B", "KiB", "MiB", "GiB"]) {
    if (amount < 1024 || suffix === "GiB") {
      return suffix === "B" ? `${Math.round(amount)} B` : `${amount.toFixed(1)} ${suffix}`;
    }
    amount /= 1024;
  }
  return `${amount.toFixed(1)} GiB`;
}

/** The slider positions a parameter offers: every integer, or powers of two. */
export function paramSteps(param: Param): number[] | null {
  if (param.scale !== "log2") return null;
  const steps: number[] = [];
  for (let value = 2 ** Math.ceil(Math.log2(param.min)); value <= param.max; value *= 2) {
    steps.push(value);
  }
  return steps;
}

/** A title as plain text, for accessible names: Markdown code marks removed. */
export function plainTitle(title: string): string {
  return title.replace(/`/g, "");
}
