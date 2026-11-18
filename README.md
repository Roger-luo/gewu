# Gewu

Gewu is a human-guided code review tool. An AI agent reads a change — a commit
range, your working tree, or a GitHub pull request — and writes a walkthrough
of it: the order to read it in, the concepts it relies on, and notes beside the
code. You read the walkthrough, mark each step reviewed, and leave feedback.
The agent helps you navigate and focus; it never reaches conclusions or
publishes a review for you.

Gewu writes the walkthrough in batches, so you can start reading while the
agent keeps writing, and send feedback that it picks up in the next batch.

There is no release yet. To try it, clone the repository and run the desktop
app with `mise run desktop`.

## Requirements

- [mise](https://mise.jdx.dev), which installs the pinned Rust, Node, pnpm,
  and other tools
- A coding agent installed and signed in, to write walkthroughs:
  [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (`claude`) or
  [Codex](https://github.com/openai/codex) (`codex`). Gewu sends the code under
  review to the agent you pick.
- For the desktop app on macOS: the Xcode Command Line Tools
  (`xcode-select --install`)

## Run it from a clone

```bash
git clone https://github.com/Roger-luo/gewu.git
cd gewu
mise trust
mise install
pnpm --dir apps/desktop install
mise run desktop
```

`mise run desktop` is the main way to use Gewu today. It builds the `gewu`
binary and opens the desktop app in development mode. From the workbench, pick
a repository and what to review in it (uncommitted changes, a commit range, or
a pull request), choose your agent in Settings, and start the review. Batches
appear as the agent writes them; read them, mark steps reviewed, and leave
feedback for the next batch.

To open a single review that was already generated, pass its repository:
`mise run desktop path/to/repo`.

## Use the terminal instead

The same reviews can be generated and read without the desktop app. Install
the binary with `mise exec -- cargo install --path .`, which puts `gewu` in
`~/.cargo/bin`, then run these inside the repository you want to review:

```bash
# Your uncommitted changes
gewu review generate --agent claude --base HEAD --working-tree

# A commit range
gewu review generate --agent claude --base main --head HEAD

# A GitHub pull request
gewu review pr 123 --agent codex
```

`generate` stops whenever a batch is ready for you. Open it in the terminal UI:

```bash
gewu review open
```

Run `gewu review generate --agent claude` again to have the agent continue
with the next batch, taking your feedback into account. The review and your
progress live in `.gewu/` in that repository. `gewu --help` lists every
command.

## Development

[AGENTS.md](AGENTS.md) describes the architecture, the required checks, and the
commit convention. Before sending a change, run:

```bash
mise exec -- prek install   # once, to install the Git hooks
mise run check
```

Design notes live in [`docs/`](docs/), starting with
[`docs/design.md`](docs/design.md).

## License

[MIT](LICENSE)
