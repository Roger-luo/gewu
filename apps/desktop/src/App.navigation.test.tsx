import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { renderApp, waitForReady } from "./test/renderApp";

describe("navigation", () => {
  it("starts on overview and can start the guided review", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();

    expect(screen.getByRole("main")).toHaveAttribute(
      "data-gewu-screen",
      "overview",
    );
    expect(
      screen.getByText(/Introduce stable session keys/i),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: /start guided review/i }),
    );
    await waitForReady();

    const main = screen.getByRole("main");
    expect(main).toHaveAttribute("data-gewu-screen", "chunk");
    expect(main).toHaveAttribute("data-gewu-chunk-id", "session-key");
  });

  it("navigator jumps without marking reviewed", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();

    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(within(nav).getByRole("button", { name: /Event path wiring/i }));
    await waitForReady();

    expect(screen.getByRole("main")).toHaveAttribute(
      "data-gewu-chunk-id",
      "event-path",
    );
    expect(screen.getByText("Not reviewed", { selector: ".step-status" })).toBeInTheDocument();
  });

  it("moves focus to the new screen title after navigating", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();

    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(within(nav).getByRole("button", { name: /Event path wiring/i }));
    await waitForReady();

    expect(screen.getByRole("heading", { level: 2, name: /Event path wiring/i })).toHaveFocus();
    expect(
      within(nav).getByRole("button", { name: /Event path wiring via session_key\(\), not reviewed/i }),
    ).toHaveAttribute("aria-current", "page");
  });

  it("shows context aside on chunk with context", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    await user.click(
      screen.getByRole("button", { name: /start guided review/i }),
    );
    await waitForReady();

    expect(
      screen.getByRole("complementary", { name: /context/i }),
    ).toBeInTheDocument();
  });

  it("puts the concept card and the notes in one side column beside the text", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    await user.click(screen.getByRole("button", { name: /start guided review/i }));
    await waitForReady();

    const main = screen.getByRole("main");
    expect(main).toHaveClass("has-side");
    const column = main.querySelector(".chunk-column");
    // The card is the column's first child, so it floats level with the title.
    expect(column?.firstElementChild).toBe(screen.getByRole("complementary", { name: /context/i }));
    for (const note of main.querySelectorAll(".quote-note")) {
      expect(note.nextElementSibling).toHaveClass("quote-row-code");
    }
  });
});
