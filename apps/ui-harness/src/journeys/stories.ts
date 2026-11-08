import type { Fixture } from "./fixture.js";
import type { Journey, JourneyRecord } from "./runner.js";

export interface Story {
  meta: Omit<JourneyRecord, "steps" | "blocked">;
  run(journey: Journey, fixture: Fixture): Promise<void>;
}

const READY = /^(Ready to read|Walkthrough ready|Raw diff ready|Complete)$/;

/** Fill in New review and start it. */
async function startReview(
  j: Journey,
  options: {
    repository?: RegExp;
    tab: "Commit range" | "Uncommitted changes" | "Pull request" | "Directory";
    fields?: () => Promise<void>;
    agent: string;
    command?: string;
    title?: string;
  },
): Promise<void> {
  await j.step("Open New review", async () => {
    await j.click("button", "New review");
    await j.screen("new-review");
  });
  await j.step(`Choose ${options.tab}`, async () => {
    if (options.repository) {
      await j.control("combobox", "Repository").selectOption({ label: await optionLabel(j, options.repository) });
    }
    await j.click("tab", options.tab);
    await options.fields?.();
  });
  await j.step(`Pick ${options.agent} and start`, async () => {
    await j.control("radio", new RegExp(`^${escape(options.agent)}`), j.control("radiogroup", "Agent")).check();
    if (options.command) {
      await j.fill("Agent command", options.command);
    }
    if (options.title) {
      await j.fill("Title", options.title);
    }
    await j.click("button", "Start review");
    await j.screen("task");
  });
}

async function optionLabel(j: Journey, pattern: RegExp): Promise<string> {
  const labels = await j.control("combobox", "Repository").locator("option").allTextContents();
  const label = labels.find((text) => pattern.test(text));
  if (!label) {
    throw new Error(`no repository option matches ${pattern}; options: ${labels.join(", ")}`);
  }
  return label;
}

function escape(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

async function backToReviews(j: Journey): Promise<void> {
  await j.step("Back to reviews", async () => {
    await j.click("button", "Back to reviews");
    await j.screen("home");
  });
}

export const STORIES: Story[] = [
  {
    meta: { id: "simple-diff", story: 1, title: "Simple diff", goal: "Add a repository, review the last commit with no agent, mark it reviewed, record a decision." },
    async run(j, fixture) {
      await j.step("Open Gewu for the first time", async () => {
        await j.screen("home");
        await j.see(/add.*repository/i);
      });
      await j.step("Add the shop repository", async () => {
        await j.fill("Repository path", fixture.shop);
        await j.click("button", "Add");
        await j.see("Reviews in shop");
      });
      await startReview(j, { tab: "Commit range", agent: "No agent" });
      await j.step("Wait for the raw diff", async () => {
        await j.page.getByText(READY).first().waitFor({ timeout: 15_000 });
      });
      await j.step("Open review", async () => {
        await j.click("button", "Open review");
        await j.screen("overview");
      });
      await j.step("Start reading", async () => {
        await j.click("button", "Start guided review");
        await j.screen("chunk");
      });
      await j.step("Mark the first step reviewed", async () => {
        await j.click("button", "Mark reviewed");
        await j.control("button", "Reviewed").waitFor();
      });
      await j.step("Go to the summary", async () => {
        await j.click("button", "Summary", j.control("navigation", "Review navigator"));
        await j.screen("summary");
      });
      await j.step("Record a decision", async () => {
        await j.click("button", "Comment");
        await j.see(/recorded: comment/);
      });
      await backToReviews(j);
    },
  },
  {
    meta: { id: "uncommitted-work", story: 2, title: "Uncommitted work", goal: "Review the working tree against HEAD with an agent, watch it run, read the first batch." },
    async run(j, fixture) {
      await startReview(j, {
        tab: "Uncommitted changes",
        agent: "Custom ACP command",
        command: fixture.agent("harness", 0.6),
      });
      await j.step("Watch the agent work", async () => {
        await j.control("log", "Agent log").waitFor({ timeout: 10_000 });
        await j.see(/Investigating/, 15_000, j.control("log", "Agent log"));
      });
      await j.step("Wait for the first batch", async () => {
        await j.see("Ready to read", 60_000, j.page.locator("main"));
      });
      await j.step("Open the first batch", async () => {
        await j.click("button", "Open review");
        await j.screen("overview");
        await j.see(/still to write|still generating/);
      });
      await backToReviews(j);
    },
  },
  {
    meta: { id: "pull-request", story: 3, title: "Pull request", goal: "Pick an open pull request from the list, pick an agent, start, and come back later." },
    async run(j, fixture) {
      await startReview(j, {
        tab: "Pull request",
        fields: async () => {
          await j.click("option", /#7/, j.control("listbox", "Open pull requests"));
        },
        agent: "Custom ACP command",
        command: fixture.agent("harness", 0.3),
      });
      await backToReviews(j);
      await j.step("See it in the list while it runs", async () => {
        await j.control("button", /#7 Print receipts|Pull request #7/, j.control("list", "Reviews")).waitFor();
      });
      await j.step("Come back when it is ready", async () => {
        const row = j.control("list", "Reviews").getByRole("listitem").filter({
          hasText: /#7 Print receipts|Pull request #7/,
        });
        await row.getByText("Ready to read").waitFor({ timeout: 60_000 });
        await row.getByRole("button", { name: /^Open review of / }).click();
        await j.screen("overview");
      });
      await backToReviews(j);
    },
  },
  {
    meta: { id: "directory", story: 4, title: "Directory", goal: "Review everything inside one directory at a ref, as if newly added." },
    async run(j) {
      await startReview(j, {
        tab: "Directory",
        fields: async () => {
          await j.fill("Directory", "src/pricing");
          await j.fill("Ref", "HEAD");
        },
        agent: "No agent",
      });
      await j.step("Wait for the walkthrough", async () => {
        await j.see(READY, 15_000, j.page.locator("main"));
      });
      await j.step("Open review", async () => {
        await j.click("button", "Open review");
        await j.screen("overview");
        await j.see(/discount\.rs/);
      });
      await backToReviews(j);
    },
  },
  {
    meta: { id: "many-reviews", story: 5, title: "Many reviews", goal: "See each review's state at a glance across repositories, switch without losing place, continue one while reading another, cancel and delete." },
    async run(j, fixture) {
      await j.step("Add a second repository", async () => {
        await j.click("button", "Add repository");
        await j.fill("Repository path", fixture.notes);
        await j.click("button", "Add", j.control("dialog", "Add a repository"));
        await j.see("Reviews in notes");
      });
      await startReview(j, { repository: /^notes/, tab: "Commit range", agent: "No agent", title: "Notes this week" });
      await j.step("See every review at a glance", async () => {
        await j.click("button", /^All reviews/);
        const items = j.control("list", "Reviews").getByRole("listitem");
        await items.nth(4).waitFor();
      });
      await j.step("Read the uncommitted-work review", async () => {
        await j.click("button", /^Open review of Uncommitted changes/);
        await j.screen("overview");
        await j.click("button", "Start guided review");
        await j.screen("chunk");
      });
      await j.step("Mark it reviewed and ask for the next batch", async () => {
        await j.click("button", "Mark reviewed");
        await j.control("button", "Reviewed").waitFor();
        await j.click("button", "Continue");
        await j.see("Generating", 5000, j.control("region", "Review task"));
      });
      await j.step("Read another review meanwhile, where it was left", async () => {
        await j.click("button", "Back to reviews");
        await j.click("button", "Open review of HEAD~1..HEAD");
        // Story 1 left this review on its summary; restore if the open landed elsewhere.
        const main = j.page.locator("main[data-gewu-screen]");
        await main.waitFor();
        if ((await main.getAttribute("data-gewu-screen")) !== "summary") {
          await j.click("button", "Summary", j.control("navigation", "Review navigator"));
        }
        await j.screen("summary");
      });
      await j.step("Return to the first review where it was left", async () => {
        await j.click("button", "Back to reviews");
        await j.click("button", /^Open review of Uncommitted changes/);
        await j.screen("chunk");
      });
      await j.step("See the next batch arrive", async () => {
        await j.see(READY, 60_000, j.control("region", "Review task"));
      });
      await backToReviews(j);
      await startReview(j, {
        repository: /^shop/,
        tab: "Commit range",
        agent: "Custom ACP command",
        command: fixture.agent("harness", 5),
        title: "Slow agent",
      });
      await j.step("Cancel the running review", async () => {
        await j.see("Generating", 5000, j.page.locator("main"));
        await j.click("button", "Cancel");
        await j.see("Cancelled", 20_000, j.page.locator("main"));
      });
      await j.step("Confirm deleting it", async () => {
        await j.click("button", "Delete…");
        await j.control("dialog", "Delete this review?").waitFor();
        await j.see(/stops the agent|removes the review/i, 5_000, j.control("dialog", "Delete this review?"));
      });
      await j.step("Delete it", async () => {
        await j.click("button", "Delete review", j.control("dialog", "Delete this review?"));
        await j.screen("home");
        await j.page.getByRole("button", { name: "Slow agent", exact: true }).waitFor({ state: "detached" });
      });
    },
  },
  {
    meta: { id: "failure", story: 6, title: "Failure", goal: "An agent that is not installed, an agent that exits early, and an empty range each say what happened and what to do." },
    async run(j, fixture) {
      await startReview(j, { repository: /^shop/, tab: "Commit range", agent: "Custom ACP command", command: "gewu-no-such-agent --acp", title: "Missing agent" });
      await j.step("Read why the agent did not start", async () => {
        await j.see("Failed", 20_000, j.page.locator("main"));
        await j.page.getByRole("alert").filter({ hasText: /gewu-no-such-agent/ }).waitFor();
      });
      await backToReviews(j);
      await startReview(j, { repository: /^shop/, tab: "Commit range", agent: "Custom ACP command", command: fixture.agent("fail", 0), title: "Crashing agent" });
      await j.step("Read why the agent stopped", async () => {
        await j.see("Failed", 20_000, j.page.locator("main"));
        await j.page.getByRole("alert").filter({ hasText: /simulated failure/ }).waitFor();
      });
      await backToReviews(j);
      await startReview(j, {
        repository: /^shop/,
        tab: "Commit range",
        fields: async () => {
          await j.fill("Base", "HEAD");
          await j.fill("Head", "HEAD");
        },
        agent: "No agent",
        title: "Empty range",
      });
      await j.step("Read why there is nothing to review", async () => {
        await j.see("Failed", 20_000, j.page.locator("main"));
        await j.page.getByRole("alert").filter({ hasText: /no changes|nothing to review|empty/i }).waitFor();
      });
      await backToReviews(j);
    },
  },
];
