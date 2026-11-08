import { execFileSync } from "node:child_process";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

/** Two throwaway repositories, a local `origin` with a pull request, and a fake `gh`. */
export interface Fixture {
  root: string;
  shop: string;
  notes: string;
  home: string;
  /** Directory to put first on the server's PATH; holds the fake `gh`. */
  bin: string;
  /** Environment for the server process. */
  env: Record<string, string>;
  /** A custom agent command line running the fake ACP agent. */
  agent(mode?: "harness" | "fail", delaySeconds?: number): string;
  dispose(): Promise<void>;
}

function git(cwd: string, ...args: string[]): string {
  return execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: "Ada Reviewer",
      GIT_AUTHOR_EMAIL: "ada@example.invalid",
      GIT_COMMITTER_NAME: "Ada Reviewer",
      GIT_COMMITTER_EMAIL: "ada@example.invalid",
    },
  }).trim();
}

async function write(repo: string, file: string, text: string): Promise<void> {
  await mkdir(path.dirname(path.join(repo, file)), { recursive: true });
  await writeFile(path.join(repo, file), text, "utf8");
}

async function shop(repo: string, origin: string): Promise<void> {
  await mkdir(repo, { recursive: true });
  git(repo, "init", "-q", "-b", "main");
  await write(repo, "README.md", "# Shop\n\nA tiny cart used by the Gewu journeys.\n");
  await write(repo, "src/cart.rs", "pub struct Cart {\n    pub items: Vec<u32>,\n}\n\nimpl Cart {\n    pub fn total(&self) -> u32 {\n        self.items.iter().sum()\n    }\n}\n");
  await write(repo, "src/pricing/mod.rs", "pub mod discount;\n");
  await write(repo, "src/pricing/discount.rs", "pub fn apply(total: u32, _code: &str) -> u32 {\n    total\n}\n");
  await write(repo, "docs/guide.md", "# Guide\n\nAdd items, then check out.\n");
  git(repo, "add", ".");
  git(repo, "commit", "-qm", "feat: initial cart");
  await write(repo, "src/pricing/discount.rs", "/// Percent off for each known code.\nconst CODES: &[(&str, u32)] = &[(\"SPRING\", 10), (\"VIP\", 25)];\n\npub fn apply(total: u32, code: &str) -> u32 {\n    let percent = CODES\n        .iter()\n        .find(|(known, _)| *known == code)\n        .map_or(0, |(_, percent)| *percent);\n    total - total * percent / 100\n}\n");
  await write(repo, "src/cart.rs", "use crate::pricing::discount;\n\npub struct Cart {\n    pub items: Vec<u32>,\n    pub code: Option<String>,\n}\n\nimpl Cart {\n    pub fn total(&self) -> u32 {\n        let total = self.items.iter().sum();\n        match &self.code {\n            Some(code) => discount::apply(total, code),\n            None => total,\n        }\n    }\n}\n");
  git(repo, "commit", "-qam", "feat(pricing): apply discount codes");

  git(repo, "checkout", "-qb", "feature/receipts");
  await write(repo, "src/receipt.rs", "use crate::cart::Cart;\n\npub fn render(cart: &Cart) -> String {\n    format!(\"{} item(s), total {}\", cart.items.len(), cart.total())\n}\n");
  git(repo, "add", ".");
  git(repo, "commit", "-qm", "feat: print receipts");
  git(repo, "checkout", "-q", "main");

  git(path.dirname(origin), "init", "-q", "--bare", origin);
  git(repo, "remote", "add", "origin", origin);
  git(repo, "push", "-q", "origin", "main", "feature/receipts:refs/pull/7/head");
  git(repo, "fetch", "-q", "origin");

  await write(repo, "README.md", "# Shop\n\nA tiny cart used by the Gewu journeys.\n\nDiscount codes: SPRING, VIP.\n");
  await write(repo, "docs/guide.md", "# Guide\n\nAdd items, enter a discount code, then check out.\n");
}

async function notes(repo: string): Promise<void> {
  await mkdir(repo, { recursive: true });
  git(repo, "init", "-q", "-b", "main");
  await write(repo, "notes/todo.md", "# Todo\n\n- write the guide\n");
  git(repo, "add", ".");
  git(repo, "commit", "-qm", "docs: start notes");
  await write(repo, "notes/todo.md", "# Todo\n\n- write the guide\n- review discount codes\n");
  await write(repo, "notes/ideas.md", "# Ideas\n\n- gift cards\n");
  git(repo, "add", ".");
  git(repo, "commit", "-qm", "docs: more notes");
}

export async function createFixture(repoRoot: string): Promise<Fixture> {
  const root = await mkdtemp(path.join(tmpdir(), "gewu-journeys-"));
  const fixture = {
    root,
    shop: path.join(root, "shop"),
    notes: path.join(root, "notes"),
    home: path.join(root, "home"),
    bin: path.join(repoRoot, "fixtures", "bin"),
  };
  await shop(fixture.shop, path.join(root, "origin.git"));
  await notes(fixture.notes);
  await chmod(path.join(fixture.bin, "gh"), 0o755);
  const pulls = path.join(root, "pulls.json");
  await writeFile(
    pulls,
    JSON.stringify({
      shop: [
        { number: 7, title: "Print receipts", author: { login: "ada" }, headRefName: "feature/receipts", baseRefName: "main" },
      ],
    }),
    "utf8",
  );
  const agentScript = path.join(repoRoot, "fixtures", "agents", "fake_acp_agent.py");
  return {
    ...fixture,
    env: {
      GEWU_HOME: fixture.home,
      GEWU_FAKE_GH: pulls,
      PATH: `${fixture.bin}${path.delimiter}${process.env.PATH ?? ""}`,
    },
    agent: (mode = "harness", delaySeconds = 0.4) =>
      `env FAKE_AGENT_MODE=${mode} FAKE_AGENT_DELAY=${delaySeconds} python3 ${agentScript}`,
    dispose: () => rm(root, { recursive: true, force: true }),
  };
}
