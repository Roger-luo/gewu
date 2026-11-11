import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { signInMockGitHub, signOutMockGitHub } from "./backend/mockGitHub";
import { renderApp, waitForReady } from "./test/renderApp";

async function openFirstChunk(fixture = "showcase") {
  renderApp(fixture);
  await waitForReady();
  await userEvent
    .setup()
    .click(screen.getByRole("button", { name: /start guided review/i }));
  await waitForReady();
  return screen.getByRole("main");
}

describe("comments on blocks and explanations", () => {
  it("anchors a comment on a code block to the lines of that block", async () => {
    const user = userEvent.setup();
    const main = await openFirstChunk();

    const [codeGroup] = within(main).getAllByRole("group", {
      name: /feedback on this code/i,
    });
    await user.click(within(codeGroup).getByRole("button", { name: /comment on code/i }));

    const dialog = screen.getByRole("dialog", { name: /add comment/i });
    expect(dialog.textContent).toMatch(/Anchored to \S+:\d+/);
    expect(within(dialog).getByRole("combobox")).toHaveValue("code");
    await user.type(within(dialog).getByRole("textbox"), "Block comment.");
    await user.click(within(dialog).getByRole("button", { name: /save/i }));
    await waitForReady();

    const item = within(main).getByText("Block comment.").closest("li");
    expect(item).toHaveTextContent(/on code · \S+:\d+/);
  });

  it("opens a note's comment on the explanation, not the code", async () => {
    const user = userEvent.setup();
    const main = await openFirstChunk("paired");

    const [noteGroup] = within(main).getAllByRole("group", {
      name: /feedback on this explanation/i,
    });
    await user.click(
      within(noteGroup).getByRole("button", { name: /request change on explanation/i }),
    );

    const dialog = screen.getByRole("dialog", { name: /request changes/i });
    expect(within(dialog).getByRole("combobox")).toHaveValue("explanation");
    await user.type(within(dialog).getByRole("textbox"), "This note is unclear.");
    await user.click(within(dialog).getByRole("button", { name: /save/i }));
    await waitForReady();

    const item = within(main).getByText("This note is unclear.").closest("li");
    expect(item).toHaveTextContent(/on explanation/);
    expect(item).toHaveTextContent(/\S+:\d+/);
  });

  it("narrows a block comment to the lines the reader selected in it", async () => {
    const user = userEvent.setup();
    const main = await openFirstChunk();
    const [codeGroup] = within(main).getAllByRole("group", { name: /feedback on this code/i });
    const block = codeGroup.closest(".block") ?? main;
    const [line] = within(block as HTMLElement).getAllByRole("button", { name: /^select line \d+$/i });
    const number = line.getAttribute("aria-label")?.match(/\d+/)?.[0];
    await user.click(line);
    await user.click(within(codeGroup).getByRole("button", { name: /comment on code/i }));

    const dialog = screen.getByRole("dialog", { name: /add comment/i });
    expect(dialog.textContent).toMatch(new RegExp(`Anchored to \\S+:${number}(?!\\d|-)`));
  });

  it("forgets a block anchor when the dialog is cancelled", async () => {
    const user = userEvent.setup();
    const main = await openFirstChunk();

    const [codeGroup] = within(main).getAllByRole("group", {
      name: /feedback on this code/i,
    });
    await user.click(within(codeGroup).getByRole("button", { name: /comment on code/i }));
    const dialog = screen.getByRole("dialog", { name: /add comment/i });
    await user.click(within(dialog).getByRole("button", { name: /cancel/i }));
    await waitForReady();

    await user.click(within(main).getByRole("button", { name: /^comment$/i }));
    const plain = screen.getByRole("dialog", { name: /add comment/i });
    expect(plain.textContent).toMatch(/applies to the whole chunk/i);
  });
});

describe("mentions in comments", () => {
  beforeEach(() => signOutMockGitHub());
  afterEach(() => signOutMockGitHub());

  async function openComment(user: ReturnType<typeof userEvent.setup>) {
    const main = await openFirstChunk();
    const [codeGroup] = within(main).getAllByRole("group", { name: /feedback on this code/i });
    await user.click(within(codeGroup).getByRole("button", { name: /comment on code/i }));
    return screen.getByRole("dialog", { name: /add comment/i });
  }

  it("suggests the repository's contributors after @ and inserts the pick", async () => {
    signInMockGitHub();
    const user = userEvent.setup();
    const dialog = await openComment(user);
    const body = within(dialog).getByRole("textbox");
    expect(await within(dialog).findByText(/type @ to mention/i)).toBeInTheDocument();

    await user.type(body, "cc @li");
    const list = within(dialog).getByRole("listbox", { name: /mention a contributor/i });
    expect(within(list).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "@linus",
    ]);
    await user.keyboard("{Enter}");
    expect(body).toHaveValue("cc @linus ");
    expect(within(dialog).queryByRole("listbox")).not.toBeInTheDocument();

    await user.type(body, "and @");
    const everyone = within(dialog).getByRole("listbox", { name: /mention a contributor/i });
    expect(within(everyone).getAllByRole("option")).toHaveLength(4);
    await user.keyboard("{ArrowDown}{ArrowDown}{Tab}");
    expect(body).toHaveValue("cc @linus and @linus ");

    await user.type(body, "@g");
    await user.keyboard("{Escape}");
    expect(within(dialog).queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: /add comment/i })).toBeInTheDocument();
  });

  it("suggests nobody while signed out", async () => {
    const user = userEvent.setup();
    const dialog = await openComment(user);
    await user.type(within(dialog).getByRole("textbox"), "cc @");
    expect(within(dialog).queryByRole("listbox")).not.toBeInTheDocument();
    expect(dialog).not.toHaveTextContent(/type @ to mention/i);
  });
});
