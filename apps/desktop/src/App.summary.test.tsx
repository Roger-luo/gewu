import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { renderApp, waitForReady } from "./test/renderApp";

describe("summary and decision", () => {
  it("lists unresolved and unreviewed items with jump links", async () => {
    const user = userEvent.setup();
    renderApp("showcase-feedback");
    await waitForReady();

    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(within(nav).getByRole("button", { name: /^summary$/i }));
    await waitForReady();

    expect(screen.getByRole("main")).toHaveAttribute(
      "data-gewu-screen",
      "summary",
    );
    const main = screen.getByRole("main");
    expect(
      within(main).getByRole("button", {
        name: /Confirm the Windows path join/i,
      }),
    ).toBeInTheDocument();
    expect(
      within(main).getByRole("button", { name: /Event path wiring/i }),
    ).toBeInTheDocument();
    expect(
      within(main).getByText(/Decisions are batch-scoped/i),
    ).toBeInTheDocument();

  });

  it("records a decision and warns about unreviewed chunks", async () => {
    const user = userEvent.setup();
    renderApp("showcase-feedback");
    await waitForReady();

    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(within(nav).getByRole("button", { name: /^summary$/i }));
    await waitForReady();

    expect(
      screen.getByText(/some chunks are still unreviewed/i),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^approve$/i }));
    await waitForReady();
    expect(screen.getByText(/recorded: approve/i)).toBeInTheDocument();
  });
});
