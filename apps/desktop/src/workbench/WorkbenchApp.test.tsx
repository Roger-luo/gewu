import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { waitForReady } from "../test/renderApp";
import { MockWorkbench } from "./mock";
import { WorkbenchApp } from "./WorkbenchApp";

function renderWorkbench(backend = new MockWorkbench(0)) {
  render(<WorkbenchApp backend={backend} />);
  return backend;
}

const main = () => screen.getByRole("main");

describe("workbench", () => {
  beforeEach(() => localStorage.clear());

  it("takes a first-time reviewer from no repository to a raw-diff review", async () => {
    const user = userEvent.setup();
    renderWorkbench();
    await waitForReady();
    expect(main()).toHaveAttribute("data-gewu-screen", "home");
    expect(screen.getByText(/start by adding a git repository/i)).toBeInTheDocument();

    await user.type(screen.getByRole("textbox", { name: "Repository path" }), "/Users/ada/Code/gewu");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await screen.findByRole("heading", { name: "Reviews in gewu" });

    await user.click(screen.getByRole("button", { name: "New review" }));
    expect(main()).toHaveAttribute("data-gewu-screen", "new-review");
    expect(screen.getByRole("combobox", { name: "Repository" })).toHaveDisplayValue(/gewu/);
    expect(screen.getByRole("tab", { name: "Commit range" })).toHaveAttribute("aria-selected", "true");
    const agents = screen.getByRole("radiogroup", { name: "Agent" });
    await user.click(within(agents).getByRole("radio", { name: /No agent/ }));
    await user.click(screen.getByRole("button", { name: "Start review" }));

    await vi.waitFor(() => expect(main()).toHaveAttribute("data-gewu-screen", "task"));
    expect(await screen.findByText("Raw diff ready")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open review" }));
    await waitForReady();
    expect(main()).toHaveAttribute("data-gewu-screen", "overview");
    expect(screen.getByRole("region", { name: "Review task" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Back to reviews" }));
    expect(main()).toHaveAttribute("data-gewu-screen", "home");
    expect(within(screen.getByRole("list", { name: "Reviews" })).getAllByRole("listitem")).toHaveLength(1);
  });

  it("shows every review's state and keeps the reader's place per task", async () => {
    const user = userEvent.setup();
    renderWorkbench(MockWorkbench.seeded(0));
    await waitForReady();
    const list = screen.getByRole("list", { name: "Reviews" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(3);
    expect(within(list).getByText("Ready to read")).toBeInTheDocument();
    expect(within(list).getByText("Failed")).toBeInTheDocument();

    await user.click(within(list).getByRole("button", { name: "Open review of Pull request #42" }));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Start guided review" }));
    await waitForReady();
    expect(main()).toHaveAttribute("data-gewu-screen", "chunk");

    await user.click(screen.getByRole("button", { name: "Back to reviews" }));
    await user.click(within(screen.getByRole("list", { name: "Reviews" })).getByRole("button", { name: "Pull request #42" }));
    expect(main()).toHaveAttribute("data-gewu-screen", "task");
    await user.click(screen.getByRole("button", { name: "Open review" }));
    await waitForReady();
    expect(main()).toHaveAttribute("data-gewu-screen", "chunk");
  });

  it("holds the next batch until the reader has marked the current one reviewed", async () => {
    const user = userEvent.setup();
    renderWorkbench(MockWorkbench.seeded(0));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Pull request #42" }));
    const cont = screen.getByRole("button", { name: "Continue" });
    expect(cont).toBeDisabled();
    expect(main()).toHaveTextContent(/finishing a batch starts the next/);
  });

  it("reopens a finished batch at the first step of the next one", async () => {
    const user = userEvent.setup();
    renderWorkbench(MockWorkbench.seeded(0));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Open review of Pull request #42" }));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Start guided review" }));
    for (let step = 0; step < 2; step++) {
      await waitForReady();
      await user.click(screen.getByRole("button", { name: "Mark reviewed" }));
    }
    await waitForReady();
    expect(main()).toHaveAttribute("data-gewu-screen", "summary");

    await user.click(screen.getByRole("button", { name: "Back to reviews" }));
    await user.click(screen.getByRole("button", { name: "Pull request #42" }));
    await user.click(screen.getByRole("button", { name: "Continue" }));
    const open = await screen.findByRole("button", { name: "Open review" });
    await vi.waitFor(() => expect(main()).toHaveTextContent(/2 of \d+ steps reviewed/));
    await user.click(open);
    await waitForReady();
    expect(main()).toHaveAttribute("data-gewu-screen", "chunk");
    expect(window.__gewu?.state().chunk_index).toBe(2);
  });

  it("explains a failed review and an agent that is not installed", async () => {
    const user = userEvent.setup();
    renderWorkbench(MockWorkbench.seeded(0));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Uncommitted changes against HEAD" }));
    // Backticks render as code, not as literal marks.
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent(/It was refused: git stash/);
    expect(within(alert).getByText("git stash").tagName).toBe("CODE");
    expect(screen.getByRole("log", { name: "Agent log" })).toHaveTextContent(/Refused/);

    await user.click(screen.getByRole("button", { name: "New review" }));
    const gemini = within(screen.getByRole("radiogroup", { name: "Agent" })).getByRole("radio", { name: /Gemini CLI/ });
    expect(gemini).toBeDisabled();
    expect(gemini).toHaveAccessibleDescription(/not found on PATH/);
  });

  it("deletes a review after confirmation", async () => {
    const user = userEvent.setup();
    renderWorkbench(MockWorkbench.seeded(0));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "9b1d03aa..4f2a9c1e" }));
    await user.click(screen.getByRole("button", { name: "Delete…" }));
    const dialog = screen.getByRole("dialog", { name: "Delete this review?" });
    await user.click(within(dialog).getByRole("button", { name: "Delete review" }));
    expect(main()).toHaveAttribute("data-gewu-screen", "home");
    expect(within(screen.getByRole("list", { name: "Reviews" })).getAllByRole("listitem")).toHaveLength(2);
  });

  it("offers to start again a review an older Gewu prepared", async () => {
    const user = userEvent.setup();
    const backend = MockWorkbench.seeded(0);
    const old = (await backend.workbench()).tasks.find((task) => task.source.type === "commit_range")!;
    backend.outdate(old.id, "the pull request's description, which explainers quote");
    renderWorkbench(backend);
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "9b1d03aa..4f2a9c1e" }));
    expect(screen.getByRole("status")).toHaveTextContent(/older version of Gewu .* lacks the pull request's description/);
    await user.click(screen.getByRole("button", { name: "Start again…" }));
    const dialog = screen.getByRole("dialog", { name: "Start this review again?" });
    await user.click(within(dialog).getByRole("button", { name: "Start again" }));

    const tasks = (await backend.workbench()).tasks;
    expect(tasks.some((task) => task.id === old.id)).toBe(false);
    const fresh = tasks.find((task) => task.source.type === "commit_range")!;
    expect(fresh.outdated).toBeNull();
    expect(main()).toHaveAttribute("data-gewu-task-id", fresh.id);
    expect(screen.queryByText(/older version of Gewu/)).not.toBeInTheDocument();
  });

  it("refuses a second review of the same uncommitted changes", async () => {
    const user = userEvent.setup();
    renderWorkbench(MockWorkbench.seeded(0));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: /notes/ }));
    await user.click(screen.getByRole("button", { name: "New review" }));
    await user.click(screen.getByRole("tab", { name: "Uncommitted changes" }));
    await user.click(screen.getByRole("button", { name: "Start review" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/already has a review of its uncommitted changes/);
  });

  it("removes a repository together with its reviews after asking", async () => {
    const user = userEvent.setup();
    renderWorkbench(MockWorkbench.seeded(0));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: /notes/ }));
    await user.click(screen.getByRole("button", { name: "Remove notes from Gewu…" }));
    await user.click(screen.getByRole("button", { name: "Remove notes and its review" }));
    await vi.waitFor(() => expect(screen.queryByRole("button", { name: /notes/ })).not.toBeInTheDocument());
    expect(within(screen.getByRole("list", { name: "Reviews" })).getAllByRole("listitem")).toHaveLength(2);
  });

  it("leaves New review with Escape", async () => {
    const user = userEvent.setup();
    renderWorkbench(MockWorkbench.seeded(0));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "New review" }));
    expect(main()).toHaveAttribute("data-gewu-screen", "new-review");
    await user.keyboard("{Escape}");
    expect(main()).toHaveAttribute("data-gewu-screen", "home");
  });

  it("remembers the reader's place across a reload", async () => {
    const user = userEvent.setup();
    const backend = MockWorkbench.seeded(0);
    const { unmount } = render(<WorkbenchApp backend={backend} />);
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Open review of Pull request #42" }));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Start guided review" }));
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Mark reviewed" }));
    await waitForReady();
    const index = window.__gewu?.state().chunk_index;
    unmount();

    render(<WorkbenchApp backend={backend} />);
    await waitForReady();
    await user.click(screen.getByRole("button", { name: "Open review of Pull request #42" }));
    await waitForReady();
    expect(main()).toHaveAttribute("data-gewu-screen", "chunk");
    expect(window.__gewu?.state().chunk_index).toBe(index);
  });
});
