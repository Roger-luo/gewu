import { readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium, type Locator, type Page } from "playwright";
import { aggregateChecks, runPageChecks } from "../checks/index.js";
import type { SandboxFile } from "./sandbox.js";

type Role = Parameters<Page["getByRole"]>[0];

/** Window size and color scheme the tester chose, kept across actions. */
interface Display {
  viewport?: { width: number; height: number };
  colorScheme?: "light" | "dark";
}

/** A name given as `/text/` matches as a case-insensitive pattern; otherwise exactly. */
function name(text: string): string | RegExp {
  const pattern = /^\/(.+)\/$/.exec(text);
  return pattern ? new RegExp(pattern[1]!, "i") : text;
}

function field(page: Page, label: string): Locator {
  const match = name(label);
  const exact = typeof match === "string";
  return page
    .getByRole("textbox", { name: match, exact })
    .or(page.getByRole("combobox", { name: match, exact }))
    .or(page.getByRole("spinbutton", { name: match, exact }))
    .first();
}

async function settle(page: Page): Promise<void> {
  await page
    .waitForFunction(
      () =>
        document.documentElement.dataset.gewuReady === "true" &&
        !document.querySelector("[data-gewu-pending]"),
      undefined,
      { timeout: 10_000 },
    )
    .catch(() => undefined);
  await page.waitForTimeout(150);
}

const USAGE = `gewu-ui act --sandbox <dir> <action>

Actions, as a user would do them (names are accessible names; /text/ is a pattern):
  look                       report the screen without acting
  open [path]                load the app, e.g. after a reload
  click <role> <name>        e.g. click button "Open review"
  fill <label> <text>        type into a text field
  select <label> <option>    choose a dropdown option by its text
  check <role> <name>        tick a radio or checkbox
  press <key>                e.g. press j, press Escape
  wait <text> [seconds]      wait until text is visible
  scroll <up|down>           scroll the main screen a page
  resize <width>x<height>    resize the window, e.g. resize 800x700
  scheme <light|dark>        switch the system color scheme
  checks                     run the mechanical UI checks on this screen`;

/** Perform one user action in the sandbox browser, then report the screen it leaves. */
export async function act(dir: string, argv: string[]): Promise<string> {
  const [action, ...rest] = argv;
  if (!action) return USAGE;
  const sandbox = JSON.parse(await readFile(path.join(dir, "sandbox.json"), "utf8")) as SandboxFile;
  const browser = await chromium.connectOverCDP(sandbox.cdp);
  try {
    const page = browser.contexts()[0]?.pages()[0];
    if (!page) throw new Error("the sandbox browser has no page; restart the sandbox");
    // Emulation ends with each DevTools session, so every action reapplies the last size and scheme.
    const displayFile = path.join(dir, "display.json");
    const display = (await readFile(displayFile, "utf8").then(JSON.parse, () => ({}))) as Display;
    if (display.viewport) await page.setViewportSize(display.viewport);
    if (display.colorScheme) await page.emulateMedia({ colorScheme: display.colorScheme });
    const report: string[] = [];
    switch (action) {
      case "look":
        break;
      case "open":
        await page.goto(new URL(rest[0] ?? "", sandbox.url).toString(), { waitUntil: "domcontentloaded" });
        break;
      case "click":
        await page.getByRole(rest[0] as Role, { name: name(rest[1] ?? ""), exact: !rest[1]?.startsWith("/") }).first().click({ timeout: 5000 });
        break;
      case "fill":
        await field(page, rest[0] ?? "").fill(rest.slice(1).join(" "), { timeout: 5000 });
        break;
      case "select":
        await field(page, rest[0] ?? "").selectOption({ label: rest.slice(1).join(" ") }, { timeout: 5000 });
        break;
      case "check":
        await page.getByRole(rest[0] as Role, { name: name(rest[1] ?? ""), exact: !rest[1]?.startsWith("/") }).first().check({ timeout: 5000 });
        break;
      case "press":
        await page.keyboard.press(rest[0] ?? "");
        break;
      case "wait":
        await page.getByText(name(rest[0] ?? "")).first().waitFor({ state: "visible", timeout: Number(rest[1] ?? 30) * 1000 });
        break;
      case "scroll":
        await page.evaluate((up) => {
          const main = document.querySelector("main") ?? document.scrollingElement;
          main?.scrollBy({ top: (up ? -0.8 : 0.8) * window.innerHeight });
        }, rest[0] === "up");
        break;
      case "resize": {
        const [width, height] = (rest[0] ?? "").split("x").map(Number);
        if (!width || !height) throw new Error("resize expects WIDTHxHEIGHT, e.g. resize 800x700");
        display.viewport = { width, height };
        await writeFile(displayFile, JSON.stringify(display), "utf8");
        await page.setViewportSize(display.viewport);
        break;
      }
      case "scheme": {
        const scheme = rest[0];
        if (scheme !== "light" && scheme !== "dark") throw new Error("scheme expects light or dark");
        display.colorScheme = scheme;
        await writeFile(displayFile, JSON.stringify(display), "utf8");
        await page.emulateMedia({ colorScheme: scheme });
        break;
      }
      case "checks": {
        const screen = await page.evaluate(() => document.querySelector("main[data-gewu-screen]")?.getAttribute("data-gewu-screen") ?? "none");
        const { findings } = await runPageChecks(page, {
          pageId: screen,
          viewport: "sandbox",
          expected: { id: screen, label: screen, target: { screen: "overview" } },
        });
        const checks = aggregateChecks(findings.filter((f) => f.key !== "hooks.missing"));
        report.push("Mechanical findings:");
        report.push(...(checks.findings.length ? checks.findings.map((f) => `- ${f.key}: ${f.description} (${f.selector})`) : ["- none"]));
        break;
      }
      default:
        return `Unknown action "${action}"\n\n${USAGE}`;
    }
    await settle(page);
    const shots = path.join(dir, "shots");
    const index = (await readdir(shots)).filter((file) => file.endsWith(".png")).length;
    const shot = path.join(shots, `${String(index + 1).padStart(3, "0")}-${[action, ...rest].join("-").replace(/[^\w.-]+/g, "_").slice(0, 60)}.png`);
    await page.screenshot({ path: shot });
    const screen = await page.evaluate(() => document.querySelector("main[data-gewu-screen]")?.getAttribute("data-gewu-screen") ?? "none");
    const aria = await page.locator("body").ariaSnapshot();
    await writeFile(shot.replace(/\.png$/, ".aria.yaml"), aria, "utf8");
    return [`screen: ${screen}`, `screenshot: ${shot}`, ...report, "", aria].join("\n");
  } finally {
    await browser.close();
  }
}
