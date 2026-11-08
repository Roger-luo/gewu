import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { renderApp, waitForReady } from "./test/renderApp";

describe("feedback flow", () => {
  it("adds a comment and lists it on the chunk", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    await user.click(
      screen.getByRole("button", { name: /start guided review/i }),
    );
    await waitForReady();

    const main = screen.getByRole("main");
    await user.click(within(main).getByRole("button", { name: /^comment$/i }));
    const dialog = screen.getByRole("dialog", { name: /add comment/i });
    await user.type(within(dialog).getByRole("textbox"), "Looks solid.");
    await user.click(within(dialog).getByRole("button", { name: /save/i }));
    await waitForReady();

    expect(screen.getByText("Looks solid.")).toBeInTheDocument();
  });

  it("resolves and deletes feedback from its row with confirm", async () => {
    const user = userEvent.setup();
    renderApp("showcase-feedback");
    await waitForReady();

    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(within(nav).getByRole("button", { name: /Event path wiring/i }));
    await waitForReady();

    await user.click(
      screen.getByRole("button", { name: /^resolve: Confirm the Windows path join/i }),
    );
    await waitForReady();
    expect(
      screen.getByRole("button", { name: /^reopen: Confirm the Windows path join/i }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: /^delete: Confirm the Windows path join/i }),
    );
    const confirm = screen.getByRole("dialog", { name: /delete feedback/i });
    await user.click(within(confirm).getByRole("button", { name: /^delete$/i }));
    await waitForReady();

    expect(
      screen.queryByText(/Confirm the Windows path join/i),
    ).not.toBeInTheDocument();
  });

  it("marks a chunk reviewed from the sticky toolbar", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    await user.click(
      screen.getByRole("button", { name: /start guided review/i }),
    );
    await waitForReady();

    const toolbar = screen.getByRole("group", { name: /mark this step/i });
    await user.click(within(toolbar).getByRole("button", { name: /mark reviewed/i }));
    await waitForReady();
    expect(
      within(toolbar).getByRole("button", { name: /^reviewed$/i }),
    ).toHaveAttribute("aria-pressed", "true");
  });
});
