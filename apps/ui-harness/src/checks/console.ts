import type { ConsoleMessage, Page, Request } from "playwright";
import type { CheckFinding } from "../types.js";

export interface ConsoleCollector {
  findings: () => CheckFinding[];
  dispose: () => void;
  setContext: (pageId: string, viewport: string) => void;
}

export function attachConsoleCollector(page: Page): ConsoleCollector {
  let pageId = "";
  let viewport = "";
  const findings: CheckFinding[] = [];

  const onConsole = (msg: ConsoleMessage) => {
    if (msg.type() !== "error") return;
    findings.push({
      key: "console.errors",
      page: pageId || "unknown",
      viewport: viewport || "unknown",
      selector: "console",
      description: msg.text(),
    });
  };

  const onPageError = (err: Error) => {
    findings.push({
      key: "console.errors",
      page: pageId || "unknown",
      viewport: viewport || "unknown",
      selector: "pageerror",
      description: err.message,
    });
  };

  const onRequestFailed = (req: Request) => {
    findings.push({
      key: "console.errors",
      page: pageId || "unknown",
      viewport: viewport || "unknown",
      selector: req.url(),
      description: `Request failed: ${req.failure()?.errorText ?? "unknown"}`,
    });
  };

  page.on("console", onConsole);
  page.on("pageerror", onPageError);
  page.on("requestfailed", onRequestFailed);

  return {
    findings: () => findings.slice(),
    dispose: () => {
      page.off("console", onConsole);
      page.off("pageerror", onPageError);
      page.off("requestfailed", onRequestFailed);
    },
    setContext: (pid, vp) => {
      pageId = pid;
      viewport = vp;
    },
  };
}
