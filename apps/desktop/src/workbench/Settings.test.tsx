import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { mockGitHub, mockRehearsing, signOutMockGitHub } from "../backend/mockGitHub";
import { waitForReady } from "../test/renderApp";
import { MockWorkbench } from "./mock";
import { WorkbenchApp } from "./WorkbenchApp";

const main = () => screen.getByRole("main");

/** Opens a Settings tab, as the reader does before using its fields. */
async function tab(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(screen.getByRole("tab", { name: new RegExp(name) }));
}

async function openSettings(user: ReturnType<typeof userEvent.setup>) {
  render(<WorkbenchApp backend={MockWorkbench.seeded(0)} />);
  await waitForReady();
  const nav = screen.getByRole("navigation", { name: "Workbench" });
  await user.click(within(nav).getByRole("button", { name: "Settings" }));
  expect(main()).toHaveAttribute("data-gewu-screen", "settings");
  return main();
}

describe("settings", () => {
  beforeEach(() => {
    localStorage.clear();
    signOutMockGitHub();
  });

  it("shows every option, including the ones only environment variables set", async () => {
    const user = userEvent.setup();
    const settings = await openSettings(user);
    const github = await within(settings).findByRole("region", { name: "GitHub" });
    expect(within(github).getByRole("radio", { name: /Gewu's GitHub app/ })).toBeChecked();
    expect(within(github).getByRole("radio", { name: /The GitHub CLI/ })).toHaveAccessibleName(
      /signed in as hubot/,
    );
    expect(within(github).getByRole("textbox", { name: "OAuth app client ID" })).toHaveAttribute(
      "placeholder",
      expect.stringContaining("Gewu's app"),
    );
    expect(within(github).getByRole("checkbox", { name: /Rehearse instead of sending/ })).not.toBeChecked();

    await tab(user, "Agents");
    const agents = within(settings).getByRole("region", { name: "Agents and generation" });
    expect(within(agents).getByRole("combobox", { name: /Default agent/ })).toHaveDisplayValue(
      "The one you used last",
    );
    expect(within(agents).getByRole("spinbutton", { name: "Proofread rounds" })).toHaveValue(2);
    expect(within(agents).getByRole("checkbox", { name: /Explain tests/ })).not.toBeChecked();
    expect(within(agents).getByRole("combobox", { name: "Proofreading agent" })).toHaveDisplayValue(
      "The agent that wrote the review",
    );

    await tab(user, "Storage");
    const storage = within(settings).getByRole("region", { name: "Where Gewu keeps things" });
    expect(storage).toHaveTextContent("GEWU_HOME");
    expect(storage).toHaveTextContent("/Users/ada/.gewu");
    expect(storage).toHaveTextContent("GEWU_BIN");
    expect(storage).toHaveTextContent("GEWU_GITHUB=record");
  });

  it("shows every prompt and what a backend adds to it, read-only", async () => {
    const user = userEvent.setup();
    const settings = await openSettings(user);
    await tab(user, "Prompts");
    const prompts = within(settings).getByRole("region", { name: "Prompts agents are sent" });

    const list = await within(prompts).findByRole("navigation", { name: "Prompts" });
    expect(within(list).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Plan",
      "Expand +",
    ]);
    await user.click(within(list).getByRole("button", { name: /Expand/ }));
    const view = within(prompts).getByRole("article", { name: "Expand prompt" });
    expect(view).toHaveTextContent("Write the walkthrough for one work item.");
    expect(view).not.toHaveTextContent("Added for");

    await user.selectOptions(within(view).getByRole("combobox", { name: "As sent to" }), "codex");
    expect(view).toHaveTextContent("Added for Codex");
    expect(view).toHaveTextContent("State why each part exists");
    await user.selectOptions(within(view).getByRole("combobox", { name: "As sent to" }), "claude");
    expect(view).toHaveTextContent("Claude Code adds nothing here");
    // The prompt is read, not edited: the only text box is the report.
    expect(within(prompts).getAllByRole("textbox").map((box) => box.closest("[role=group]"))).toEqual([
      within(view).getByRole("group", { name: "Report a problem with this prompt" }),
    ]);
  });

  it("reports a problem with a prompt only after a second confirmation", async () => {
    const user = userEvent.setup();
    const backend = MockWorkbench.seeded(0);
    render(<WorkbenchApp backend={backend} />);
    await waitForReady();
    const nav = screen.getByRole("navigation", { name: "Workbench" });
    await user.click(within(nav).getByRole("button", { name: "Settings" }));
    await tab(user, "Prompts");
    const prompts = within(main()).getByRole("region", { name: "Prompts agents are sent" });
    const list = await within(prompts).findByRole("navigation", { name: "Prompts" });
    await user.click(within(list).getByRole("button", { name: /Expand/ }));
    const view = within(prompts).getByRole("article", { name: "Expand prompt" });
    await user.selectOptions(within(view).getByRole("combobox", { name: "As sent to" }), "codex");

    const form = within(view).getByRole("group", { name: "Report a problem with this prompt" });
    expect(form).toHaveTextContent("public issue on Roger-luo/gewu");
    const report = within(form).getByRole("button", { name: "Report a problem" });
    expect(report).toBeDisabled();
    await user.type(within(form).getByRole("textbox"), "The rubric contradicts itself.");
    await user.click(report);
    expect(backend.reports).toEqual([]);

    const confirm = within(form).getByRole("group", { name: "Confirm the report" });
    await user.click(within(confirm).getByRole("button", { name: "Send" }));
    expect(backend.reports).toEqual([
      { prompt_id: "expand", backend: "codex", body: "The rubric contradicts itself." },
    ]);
    expect(await within(form).findByRole("status")).toHaveTextContent(
      /Rehearsed, nothing sent: Issue #901/,
    );
  });

  it("saves the choices and the next review starts from them", async () => {
    const user = userEvent.setup();
    const settings = await openSettings(user);
    const github = await within(settings).findByRole("region", { name: "GitHub" });
    await user.click(within(github).getByRole("radio", { name: /The GitHub CLI/ }));
    await user.click(within(github).getByRole("checkbox", { name: /Rehearse/ }));
    await tab(user, "Agents");
    const agents = within(settings).getByRole("region", { name: "Agents and generation" });
    await user.selectOptions(within(agents).getByRole("combobox", { name: /Default agent/ }), "codex");
    const rounds = within(agents).getByRole("spinbutton", { name: "Proofread rounds" });
    await user.clear(rounds);
    await user.type(rounds, "0");
    await user.click(within(agents).getByRole("checkbox", { name: /Explain tests/ }));
    const proofreader = within(agents).getByRole("combobox", { name: "Proofreading agent" });
    expect(within(proofreader).queryByRole("option", { name: /No agent|Custom/ })).toBeNull();
    await user.selectOptions(proofreader, "claude");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Saved");
    expect(within(agents).getByRole("checkbox", { name: /Explain tests/ })).toBeChecked();
    expect(proofreader).toHaveDisplayValue("Claude Code");
    expect(mockRehearsing()).toBe(true);
    expect((await mockGitHub.settings()).method).toBe("gh");
    await user.click(screen.getByRole("button", { name: /Back to reviews/ }));
    const account = screen.getByRole("region", { name: "Account and settings" });
    expect(account).toHaveTextContent("via GitHub CLI");

    await user.click(screen.getByRole("button", { name: "New review" }));
    const picker = screen.getByRole("radiogroup", { name: "Agent" });
    expect(within(picker).getByRole("radio", { name: /Codex/ })).toBeChecked();
  });

  it("lets the agent write the whole walkthrough ahead by default, or up to a number of batches", async () => {
    const user = userEvent.setup();
    const backend = MockWorkbench.seeded(0);
    render(<WorkbenchApp backend={backend} />);
    await waitForReady();
    await user.click(
      within(screen.getByRole("navigation", { name: "Workbench" })).getByRole("button", { name: "Settings" }),
    );
    await tab(user, "Agents");
    const ahead = screen.getByRole("combobox", { name: "Write ahead of you" });
    expect(ahead).toHaveDisplayValue(/No limit/);
    expect(screen.queryByRole("spinbutton", { name: "Batches ahead" })).toBeNull();

    await user.selectOptions(ahead, "limited");
    const batches = screen.getByRole("spinbutton", { name: "Batches ahead" });
    await user.clear(batches);
    await user.type(batches, "3");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Saved")).toBeInTheDocument();
    expect((await backend.workbench()).settings.write_ahead).toEqual({ batches: 3 });
  });

  it("refuses rounds out of range and keeps the old value", async () => {
    const user = userEvent.setup();
    const settings = await openSettings(user);
    await tab(user, "Agents");
    const rounds = within(settings).getByRole("spinbutton", { name: "Proofread rounds" });
    await user.clear(rounds);
    await user.type(rounds, "9");
    await user.click(screen.getByRole("button", { name: "Save" }));
    const alert = await within(settings).findByRole("alert");
    expect(alert).toHaveTextContent("Not saved: proofread rounds must be between 0 and 5");
    expect(screen.queryByText("Saved")).not.toBeInTheDocument();
  });

  it("is a page of its own, with its sections in the sidebar and a way back", async () => {
    const user = userEvent.setup();
    await openSettings(user);
    // The workbench sidebar gives way to the sections.
    expect(screen.queryByRole("navigation", { name: "Workbench" })).toBeNull();
    const sections = screen.getByRole("navigation", { name: "Settings sections" });
    expect(within(sections).getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "GitHub",
      "Agents",
      "Prompts",
      "Storage",
    ]);
    expect(screen.getByRole("heading", { name: "GitHub", level: 2 })).toBeInTheDocument();
    await tab(user, "Storage");
    expect(screen.getByRole("heading", { name: "Storage", level: 2 })).toBeInTheDocument();

    await user.click(within(sections).getByRole("button", { name: /Back to reviews/ }));
    expect(screen.getByRole("navigation", { name: "Workbench" })).toBeInTheDocument();
    expect(main()).toHaveAttribute("data-gewu-screen", "home");
  });

  it("opens from the app menu's Settings…", async () => {
    let open: (() => void) | null = null;
    const backend = Object.assign(MockWorkbench.seeded(0), {
      onOpenSettings: (listener: () => void) => {
        open = listener;
        return () => {
          open = null;
        };
      },
    });
    render(<WorkbenchApp backend={backend} />);
    await waitForReady();
    expect(open).not.toBeNull();
    act(() => open?.());
    expect(await screen.findByRole("navigation", { name: "Settings sections" })).toBeInTheDocument();
  });
});

describe("sidebar", () => {
  beforeEach(() => {
    localStorage.clear();
    signOutMockGitHub();
  });

  it("puts the ways to start together and Settings with the account", async () => {
    render(<WorkbenchApp backend={MockWorkbench.seeded(0)} />);
    await waitForReady();
    const nav = screen.getByRole("navigation", { name: "Workbench" });
    const newReview = within(nav).getByRole("button", { name: "New review" });
    const add = within(nav).getByRole("button", { name: "Add repository" });
    expect(newReview.parentElement).toBe(add.parentElement);

    const footer = within(nav).getByRole("region", { name: "Account and settings" });
    const settings = within(footer).getByRole("button", { name: "Settings" });
    const signIn = within(footer).getByRole("button", { name: /sign in to github/i });
    expect(settings.parentElement).toBe(signIn.parentElement);
    expect(settings.querySelector("svg")).not.toBeNull();
  });

  it("shows a sign-in made inside a review in the workbench too", async () => {
    const user = userEvent.setup();
    render(<WorkbenchApp backend={MockWorkbench.seeded(0)} />);
    await waitForReady();
    const [open] = await screen.findAllByRole("button", { name: /^Open review/ });
    await user.click(open);
    await waitForReady();
    const inReview = screen.getByRole("region", { name: "Account and settings" });
    await user.click(within(inReview).getByRole("button", { name: /sign in to github/i }));
    await user.click(screen.getByRole("button", { name: /use the github cli/i }));
    expect(inReview).toHaveTextContent("via GitHub CLI");

    await user.click(screen.getByRole("button", { name: "Back to reviews" }));
    const nav = screen.getByRole("navigation", { name: "Workbench" });
    const foot = within(nav).getByRole("region", { name: "Account and settings" });
    expect(foot).toHaveTextContent("via GitHub CLI");

    await user.click(within(foot).getByRole("button", { name: "Switch" }));
    await user.click(screen.getByRole("button", { name: "Stop using the GitHub CLI" }));
    await vi.waitFor(() => expect(foot).toHaveTextContent("Not signed in to GitHub"));
  });

  it("goes back to the review Settings was opened from", async () => {
    const user = userEvent.setup();
    render(<WorkbenchApp backend={MockWorkbench.seeded(0)} />);
    await waitForReady();
    const [open] = await screen.findAllByRole("button", { name: /^Open review/ });
    await user.click(open);
    await waitForReady();
    const foot = screen.getByRole("region", { name: "Account and settings" });
    await user.click(within(foot).getByRole("button", { name: "Settings" }));
    expect(main()).toHaveAttribute("data-gewu-screen", "settings");
    await user.click(screen.getByRole("button", { name: /back to the review/i }));
    await waitForReady();
    expect(main()).not.toHaveAttribute("data-gewu-screen", "settings");
    expect(screen.getByRole("region", { name: "Review task" })).toBeInTheDocument();
  });
});
