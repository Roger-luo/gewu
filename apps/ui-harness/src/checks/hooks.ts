import type { Page } from "playwright";
import type { CheckFinding, PageInfo, Target } from "../types.js";

export async function runHookChecks(
  page: Page,
  meta: { pageId: string; viewport: string; expected: PageInfo },
): Promise<CheckFinding[]> {
  const findings: CheckFinding[] = [];
  const probe = await page.evaluate((expectedTarget: Target) => {
    const ready = document.documentElement.dataset.gewuReady === "true";
    const main = document.querySelector("main[data-gewu-screen]");
    const screen = main?.getAttribute("data-gewu-screen") ?? null;
    const nav = document.querySelector('nav[aria-label="Review navigator"]');
    const api = window.__gewu;
    const state = api?.state() ?? null;
    let stateOk = false;
    if (state) {
      if (expectedTarget.screen === "chunk") {
        stateOk =
          state.screen === "chunk" && state.chunk_index === expectedTarget.index;
      } else {
        stateOk = state.screen === expectedTarget.screen;
      }
    }
    let screenOk = false;
    if (expectedTarget.screen === "chunk") {
      screenOk =
        screen === "chunk" &&
        main?.getAttribute("data-gewu-chunk-id") != null;
    } else {
      screenOk = screen === expectedTarget.screen;
    }
    return {
      ready,
      hasMain: !!main,
      hasNav: !!nav,
      hasApi: !!api,
      stateOk,
      screenOk,
      state,
      screen,
    };
  }, meta.expected.target);

  if (!probe.ready) {
    findings.push({
      key: "hooks.missing",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: "html[data-gewu-ready]",
      description: "Missing data-gewu-ready=\"true\"",
    });
  }
  if (!probe.hasMain) {
    findings.push({
      key: "hooks.missing",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: "main[data-gewu-screen]",
      description: "Missing main[data-gewu-screen]",
    });
  } else if (!probe.screenOk) {
    findings.push({
      key: "hooks.missing",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: "main[data-gewu-screen]",
      description: `Screen attribute "${probe.screen}" inconsistent with goto target`,
    });
  }
  if (!probe.hasNav) {
    findings.push({
      key: "hooks.missing",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: 'nav[aria-label="Review navigator"]',
      description: "Missing Review navigator landmark",
    });
  }
  if (!probe.hasApi) {
    findings.push({
      key: "hooks.missing",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: "window.__gewu",
      description: "Missing window.__gewu test API",
    });
  } else if (!probe.stateOk) {
    findings.push({
      key: "hooks.missing",
      page: meta.pageId,
      viewport: meta.viewport,
      selector: "window.__gewu.state()",
      description: `state() inconsistent with goto target (got ${JSON.stringify(probe.state)})`,
    });
  }
  return findings;
}
