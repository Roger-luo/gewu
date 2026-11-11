import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import type { Locator, Page } from "playwright";
import { runAxeChecks } from "../checks/axe.js";
import { runLayoutChecks } from "../checks/layout.js";
import { runRawMarkdownChecks } from "../checks/markdown.js";
import { runTargetSizeChecks } from "../checks/targets.js";
import { runTextSizeChecks } from "../checks/text.js";
import { pad } from "../artifacts.js";
import type { CheckFinding } from "../types.js";

export interface StepRecord {
  index: number;
  name: string;
  ok: boolean;
  error: string | null;
  screen: string | null;
  shot: string;
  aria: string;
}

export interface JourneyRecord {
  id: string;
  story: number;
  title: string;
  goal: string;
  steps: StepRecord[];
  /** The step the script could not perform through visible controls. */
  blocked: string | null;
}

type Role = Parameters<Page["getByRole"]>[0];

/** Drives one story through visible controls, capturing every step. */
export class Journey {
  readonly record: JourneyRecord;
  readonly findings: CheckFinding[] = [];
  private readonly dir: string;

  constructor(
    readonly page: Page,
    private readonly outDir: string,
    readonly viewport: string,
    meta: Omit<JourneyRecord, "steps" | "blocked">,
  ) {
    this.record = { ...meta, steps: [], blocked: null };
    this.dir = `journeys/${pad(meta.story)}-${meta.id}`;
  }

  get blocked(): boolean {
    return this.record.blocked !== null;
  }

  /** Run `act`, then capture the screen it leaves. A failure blocks the rest. */
  async step(name: string, act: () => Promise<void>): Promise<void> {
    if (this.blocked) {
      return;
    }
    let error: string | null = null;
    try {
      await act();
      await this.settle();
    } catch (err) {
      error = (err instanceof Error ? err.message : String(err)).split("\n").slice(0, 6).join("\n");
      this.record.blocked = `${name}: ${error}`;
    }
    const index = this.record.steps.length + 1;
    const base = `${this.dir}/${pad(index)}-${slug(name)}`;
    await mkdir(path.join(this.outDir, this.dir), { recursive: true });
    await this.page.screenshot({ path: path.join(this.outDir, `${base}.png`), animations: "disabled" });
    const aria = await this.page.locator("body").ariaSnapshot().catch(() => "");
    await writeFile(path.join(this.outDir, `${base}.aria.yaml`), aria, "utf8");
    const screen = await this.page
      .locator("main[data-gewu-screen]")
      .first()
      .getAttribute("data-gewu-screen", { timeout: 1000 })
      .catch(() => null);
    this.record.steps.push({ index, name, ok: error === null, error, screen, shot: `${base}.png`, aria: `${base}.aria.yaml` });
    if (error === null) {
      await this.check(`${this.record.id}/${pad(index)}-${slug(name)}`, `${base}.png`);
    }
  }

  /** The control named `name`, which must be visible. */
  control(role: Role, name: string | RegExp, within?: Locator): Locator {
    return (within ?? this.page).getByRole(role, { name, exact: typeof name === "string" });
  }

  async click(role: Role, name: string | RegExp, within?: Locator): Promise<void> {
    await this.control(role, name, within).click({ timeout: 5000 });
  }

  async fill(name: string | RegExp, value: string): Promise<void> {
    const box = this.page.getByRole("textbox", { name, exact: typeof name === "string" }).or(
      this.page.getByRole("combobox", { name, exact: typeof name === "string" }),
    );
    await box.first().fill(value, { timeout: 5000 });
  }

  /** Wait until `text` shows on screen, e.g. a status an agent reaches. */
  async see(text: string | RegExp, timeoutMs = 10_000, within?: Locator): Promise<void> {
    await (within ?? this.page).getByText(text).first().waitFor({ state: "visible", timeout: timeoutMs });
  }

  async screen(expected: string, timeoutMs = 10_000): Promise<void> {
    await this.page.locator(`main[data-gewu-screen="${expected}"]`).waitFor({ timeout: timeoutMs });
  }

  private async settle(): Promise<void> {
    await this.page
      .waitForFunction(
        () =>
          document.documentElement.dataset.gewuReady === "true" &&
          !document.querySelector("[data-gewu-pending]"),
        undefined,
        { timeout: 10_000 },
      )
      .catch(() => undefined);
    await this.page.waitForTimeout(120);
  }

  private async check(pageId: string, evidence: string): Promise<void> {
    const meta = { pageId, viewport: this.viewport };
    const found = [
      ...(await runAxeChecks(this.page, meta)),
      ...(await runLayoutChecks(this.page, meta)),
      ...(await runTextSizeChecks(this.page, meta)),
      ...(await runTargetSizeChecks(this.page, meta)),
      // Journeys reach screens the page capture never visits, such as a
      // failed task or a blocked publish. Found by the final UI judge.
      ...(await runRawMarkdownChecks(this.page, meta)),
    ];
    for (const finding of found) {
      finding.evidence = evidence;
      this.findings.push(finding);
    }
  }
}

function slug(text: string): string {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 48);
}
