import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { CheckKey, Rubric, RubricGate } from "./types.js";
import { CHECK_KEYS } from "./types.js";

const PACKAGE_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export function packageRoot(): string {
  return PACKAGE_ROOT;
}

export function defaultRubricDir(): string {
  return path.join(PACKAGE_ROOT, "rubric");
}

export async function findLatestRubricPath(dir = defaultRubricDir()): Promise<string | null> {
  let entries: string[];
  try {
    entries = await readdir(dir);
  } catch {
    return null;
  }
  const versions = entries
    .map((name) => {
      const m = /^v(\d+)\.json$/.exec(name);
      return m ? { n: Number(m[1]), name } : null;
    })
    .filter((v): v is { n: number; name: string } => v !== null)
    .sort((a, b) => b.n - a.n);
  const latest = versions[0];
  return latest ? path.join(dir, latest.name) : null;
}

export async function loadRubric(file?: string): Promise<Rubric> {
  const resolved =
    file ??
    (await findLatestRubricPath()) ??
    (() => {
      throw new Error(
        `No rubric found under ${defaultRubricDir()}. Pass --rubric <file> or add rubric/vN.json.`,
      );
    })();
  const raw = JSON.parse(await readFile(resolved, "utf8")) as unknown;
  return validateRubric(raw, resolved);
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function requireString(obj: Record<string, unknown>, key: string, ctx: string): string {
  const v = obj[key];
  if (typeof v !== "string" || v.length === 0) {
    throw new Error(`${ctx}: missing string "${key}"`);
  }
  return v;
}

function requireNumber(obj: Record<string, unknown>, key: string, ctx: string): number {
  const v = obj[key];
  if (typeof v !== "number" || Number.isNaN(v)) {
    throw new Error(`${ctx}: missing number "${key}"`);
  }
  return v;
}

function requireStringArray(obj: Record<string, unknown>, key: string, ctx: string): string[] {
  const v = obj[key];
  if (!Array.isArray(v) || !v.every((x) => typeof x === "string")) {
    throw new Error(`${ctx}: missing string[] "${key}"`);
  }
  return v;
}

function requireAnchors(
  obj: Record<string, unknown>,
  key: string,
  ctx: string,
  min: number,
  max: number,
): Record<string, string> {
  const v = obj[key];
  if (!isRecord(v)) throw new Error(`${ctx}: missing anchors object`);
  const out: Record<string, string> = {};
  for (let i = min; i <= max; i++) {
    const a = v[String(i)];
    if (typeof a !== "string") {
      throw new Error(`${ctx}: anchors must include "${i}"`);
    }
    out[String(i)] = a;
  }
  return out;
}

export function validateRubric(raw: unknown, source = "rubric"): Rubric {
  if (!isRecord(raw)) throw new Error(`${source}: root must be an object`);
  const version = requireNumber(raw, "version", source);
  const title = requireString(raw, "title", source);
  if (!isRecord(raw.scale)) throw new Error(`${source}: scale required`);
  const scaleMin = requireNumber(raw.scale, "min", `${source}.scale`);
  const scaleMax = requireNumber(raw.scale, "max", `${source}.scale`);
  const scaleAnchors = requireAnchors(raw.scale, "anchors", `${source}.scale`, scaleMin, scaleMax);

  if (!Array.isArray(raw.dimensions) || raw.dimensions.length === 0) {
    throw new Error(`${source}: dimensions required`);
  }
  const dimensions = raw.dimensions.map((d, i) => {
    const ctx = `${source}.dimensions[${i}]`;
    if (!isRecord(d)) throw new Error(`${ctx}: must be object`);
    return {
      id: requireString(d, "id", ctx),
      name: requireString(d, "name", ctx),
      weight: requireNumber(d, "weight", ctx),
      question: requireString(d, "question", ctx),
      look_for: requireStringArray(d, "look_for", ctx),
      anchors: requireAnchors(d, "anchors", ctx, scaleMin, scaleMax),
      evidence: requireStringArray(d, "evidence", ctx),
      sources: requireStringArray(d, "sources", ctx),
    };
  });

  const ids = new Set(dimensions.map((d) => d.id));
  if (ids.size !== dimensions.length) throw new Error(`${source}: duplicate dimension ids`);

  const gates: RubricGate[] = Array.isArray(raw.gates)
    ? raw.gates.map((g, i) => {
        const ctx = `${source}.gates[${i}]`;
        if (!isRecord(g)) throw new Error(`${ctx}: must be object`);
        const check = requireString(g, "check", ctx) as CheckKey;
        if (!CHECK_KEYS.includes(check)) {
          throw new Error(`${ctx}: unknown check key "${check}"`);
        }
        const effect = requireString(g, "effect", ctx);
        if (effect !== "fail" && effect !== "cap") {
          throw new Error(`${ctx}: effect must be fail|cap`);
        }
        const gate: RubricGate = {
          id: requireString(g, "id", ctx),
          check,
          max: requireNumber(g, "max", ctx),
          effect,
          description: requireString(g, "description", ctx),
        };
        if (effect === "cap") {
          gate.cap = requireNumber(g, "cap", ctx);
        }
        return gate;
      })
    : [];

  const sources = Array.isArray(raw.sources)
    ? raw.sources.map((s, i) => {
        const ctx = `${source}.sources[${i}]`;
        if (!isRecord(s)) throw new Error(`${ctx}: must be object`);
        return {
          id: requireString(s, "id", ctx),
          title: requireString(s, "title", ctx),
          url: requireString(s, "url", ctx),
        };
      })
    : [];

  return {
    version,
    title,
    scale: { min: scaleMin, max: scaleMax, anchors: scaleAnchors },
    dimensions,
    gates,
    sources,
  };
}

export { renderRubricText } from "./rubric-text.js";
