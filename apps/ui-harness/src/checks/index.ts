import type { Page } from "playwright";
import type {
  CheckFinding,
  CheckKey,
  ChecksFile,
  InteractionShot,
  PageInfo,
} from "../types.js";
import { CHECK_KEYS } from "../types.js";
import { runAxeChecks } from "./axe.js";
import { attachConsoleCollector, type ConsoleCollector } from "./console.js";
import { runHookChecks } from "./hooks.js";
import { runKeyboardChecks } from "./keyboard.js";
import { runLayoutChecks } from "./layout.js";
import { runTargetSizeChecks } from "./targets.js";
import { runTextSizeChecks } from "./text.js";

export { attachConsoleCollector };
export type { ConsoleCollector };

export async function runPageChecks(
  page: Page,
  meta: { pageId: string; viewport: string; expected: PageInfo },
  opts: { includeKeyboard?: boolean } = {},
): Promise<{ findings: CheckFinding[]; interactionShots: InteractionShot[] }> {
  const findings: CheckFinding[] = [];
  findings.push(...(await runHookChecks(page, meta)));
  findings.push(...(await runAxeChecks(page, meta)));
  findings.push(...(await runLayoutChecks(page, meta)));
  findings.push(...(await runTextSizeChecks(page, meta)));
  findings.push(...(await runTargetSizeChecks(page, meta)));

  let interactionShots: InteractionShot[] = [];
  if (opts.includeKeyboard) {
    const kb = await runKeyboardChecks(page, meta);
    findings.push(...kb.findings);
    interactionShots = kb.interactionShots;
  }
  return { findings, interactionShots };
}

export function aggregateChecks(findings: CheckFinding[]): ChecksFile {
  const counters = Object.fromEntries(CHECK_KEYS.map((k) => [k, 0])) as Record<
    CheckKey,
    number
  >;
  for (const f of findings) {
    counters[f.key] += 1;
  }
  return { counters, findings };
}
