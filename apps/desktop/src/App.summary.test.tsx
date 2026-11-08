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
    const comments = within(main).getByRole("heading", { name: "Comments" }).closest("section");
    expect(within(comments!).getAllByRole("button").length).toBeGreaterThan(0);

  });

  it("records a decision and warns about unreviewed chunks", async () => {
    const user = userEvent.setup();
    renderApp("showcase-feedback");
    await waitForReady();

    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(within(nav).getByRole("button", { name: /^summary$/i }));
    await waitForReady();

    expect(
      screen.getByText(/chunks? (is|are) still unreviewed/i),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^approve$/i }));
    await waitForReady();
    expect(screen.getByText(/recorded: approve/i)).toBeInTheDocument();
  });

  it("offers the next unreviewed step as the obvious action", async () => {
    const user = userEvent.setup();
    renderApp("showcase-feedback");
    await waitForReady();
    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(within(nav).getByRole("button", { name: /^summary$/i }));
    await waitForReady();

    const next = screen.getByRole("button", { name: "Review next unreviewed step" });
    expect(next).toHaveClass("primary");
    for (const decision of ["Approve", "Comment", "Request changes"]) {
      expect(screen.getByRole("button", { name: decision })).not.toHaveClass("primary");
    }
    await user.click(next);
    await waitForReady();
    expect(screen.getByRole("main")).toHaveAttribute("data-gewu-screen", "chunk");
    expect(screen.getByText("Not reviewed")).toBeInTheDocument();
  });
});
