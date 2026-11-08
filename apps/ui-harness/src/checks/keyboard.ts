import type { Page } from "playwright";
import type { CheckFinding, InteractionShot, Target } from "../types.js";

async function visibleFocus(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return false;
    const style = getComputedStyle(el);
    const outline = style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0;
    const ring =
      style.boxShadow.includes("rgb") ||
      style.boxShadow.includes("#") ||
      style.outlineOffset !== "0px";
    const border =
      (style.borderStyle !== "none" && parseFloat(style.borderWidth) > 0) ||
      el.classList.contains("focus-visible") ||
      el.matches(":focus-visible");
    return outline || ring || border;
  });
}

export async function runKeyboardChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<{ findings: CheckFinding[]; interactionShots: InteractionShot[] }> {
  const findings: CheckFinding[] = [];
  const interactionShots: InteractionShot[] = [];

  const fail = (description: string, selector = "keyboard") => {
    findings.push({
      key: "keyboard.failures",
      page: meta.pageId,
      viewport: meta.viewport,
      selector,
      description,
    });
  };

  const start = await page.evaluate(() => window.__gewu?.state() ?? null);
  if (!start) {
    fail("window.__gewu unavailable for keyboard probes");
    return { findings, interactionShots };
  }

  // j / k page navigation
  await page.keyboard.press("j");
  await page.waitForTimeout(100);
  const afterJ = await page.evaluate(() => window.__gewu?.state() ?? null);
  await page.keyboard.press("k");
  await page.waitForTimeout(100);
  const afterK = await page.evaluate(() => window.__gewu?.state() ?? null);
  if (!afterJ || !afterK) {
    fail("j/k did not leave a readable state()");
  } else if (start.chunk_count > 1 || start.screen !== "overview") {
    // At least one of j/k should change state when there is somewhere to go,
    // or return to start after a round trip from overview.
    const changed = JSON.stringify(afterJ) !== JSON.stringify(start);
    const restored =
      afterK.screen === start.screen && afterK.chunk_index === start.chunk_index;
    if (!changed && start.screen === "overview" && start.chunk_count > 0) {
      fail("j did not move from overview to next page");
    }
    if (changed && !restored && start.screen !== "summary") {
      // k should undo j when not at the end edge oddly — soft check
    }
  }

  // ? opens dialog, Esc closes
  await page.keyboard.press("?");
  await page.waitForTimeout(150);
  const helpOpen = await page.evaluate(() => {
    const dialog = document.querySelector('[role="dialog"]');
    const state = window.__gewu?.state();
    return { hasDialog: !!dialog, overlay: state?.overlay ?? null };
  });
  if (!helpOpen.hasDialog && helpOpen.overlay !== "help") {
    fail("? did not open a role=dialog (help)");
  } else {
    interactionShots.push({ name: "help-open" });
  }
  await page.keyboard.press("Escape");
  await page.waitForTimeout(150);
  const helpClosed = await page.evaluate(() => {
    const dialog = document.querySelector('[role="dialog"]');
    const state = window.__gewu?.state();
    return { hasDialog: !!dialog, overlay: state?.overlay ?? null };
  });
  if (helpClosed.hasDialog || helpClosed.overlay === "help") {
    fail("Esc did not close the help dialog");
  }

  // Tab reaches interactive with visible focus
  await page.keyboard.press("Tab");
  await page.waitForTimeout(50);
  const focused = await page.evaluate(() => {
    const el = document.activeElement;
    return el && el !== document.body;
  });
  if (!focused) {
    fail("Tab did not focus an interactive element");
  } else if (!(await visibleFocus(page))) {
    fail("Focused element lacks a visible focus indicator");
  }

  // o / s navigation
  await page.keyboard.press("o");
  await page.waitForTimeout(100);
  const onOverview = await page.evaluate(() => window.__gewu?.state()?.screen === "overview");
  if (!onOverview) fail("o did not navigate to overview");

  await page.keyboard.press("s");
  await page.waitForTimeout(100);
  const onSummary = await page.evaluate(() => window.__gewu?.state()?.screen === "summary");
  if (!onSummary) fail("s did not navigate to summary");

  // Feedback belongs to a chunk, so open the form via `c` on the first one
  // (optional interaction shot).
  const chunkTarget: Target = { screen: "chunk", index: 0 };
  await page.evaluate(async (target) => {
    await window.__gewu?.goto(target);
  }, chunkTarget);
  await page.keyboard.press("c");
  await page.waitForTimeout(150);
  const feedback = await page.evaluate(() => {
    const state = window.__gewu?.state();
    const dialog = document.querySelector('[role="dialog"]');
    return { overlay: state?.overlay ?? null, hasDialog: !!dialog };
  });
  if (feedback.overlay === "feedback" || feedback.hasDialog) {
    interactionShots.push({ name: "feedback-open", target: chunkTarget });
    await page.keyboard.press("Escape");
    await page.waitForTimeout(100);
  }

  // Restore expected page via goto is caller's job; return to overview for stability
  await page.evaluate(async () => {
    await window.__gewu?.goto({ screen: "overview" });
  });

  return { findings, interactionShots };
}
