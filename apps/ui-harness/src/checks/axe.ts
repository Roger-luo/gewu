import AxeBuilder from "@axe-core/playwright";
import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

export async function runAxeChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();

  const findings: CheckFinding[] = [];
  for (const violation of results.violations) {
    const impact = violation.impact ?? "moderate";
    const key =
      impact === "critical"
        ? "axe.critical"
        : impact === "serious"
          ? "axe.serious"
          : impact === "minor"
            ? "axe.minor"
            : "axe.moderate";
    for (const node of violation.nodes) {
      findings.push({
        key,
        page: meta.pageId,
        viewport: meta.viewport,
        selector: node.target.join(", "),
        description: `${violation.id}: ${violation.help}`,
      });
    }
  }
  return findings;
}
