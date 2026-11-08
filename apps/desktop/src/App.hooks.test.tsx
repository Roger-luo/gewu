import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { renderApp, waitForReady } from "./test/renderApp";

describe("test hooks", () => {
  it("exposes state, pages, and goto", async () => {
    renderApp("showcase");
    await waitForReady();

    expect(document.documentElement.dataset.gewuReady).toBe("true");
    expect(window.__gewu).toBeDefined();

    const pages = window.__gewu!.pages();
    expect(pages[0]).toMatchObject({ id: "overview", label: "Overview" });
    expect(pages.some((page) => page.id === "chunk-session-key")).toBe(true);
    expect(pages.at(-1)).toMatchObject({ id: "summary" });

    expect(window.__gewu!.state()).toEqual({
      screen: "overview",
      task_id: null,
      chunk_index: null,
      chunk_count: 5,
      overlay: null,
    });

    await window.__gewu!.goto({ screen: "chunk", index: 1 });
    expect(window.__gewu!.state().screen).toBe("chunk");
    expect(window.__gewu!.state().chunk_index).toBe(1);
    expect(screen.getByRole("main")).toHaveAttribute(
      "data-gewu-chunk-id",
      "event-path",
    );
  });
});

describe("keyboard shortcuts", () => {
  it("navigates with j/k/o/s and opens help with ?", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();

    await user.keyboard("j");
    await waitForReady();
    expect(screen.getByRole("main")).toHaveAttribute(
      "data-gewu-screen",
      "chunk",
    );

    await user.keyboard("s");
    await waitForReady();
    expect(screen.getByRole("main")).toHaveAttribute(
      "data-gewu-screen",
      "summary",
    );

    await user.keyboard("o");
    await waitForReady();
    expect(screen.getByRole("main")).toHaveAttribute(
      "data-gewu-screen",
      "overview",
    );

    await user.keyboard("?");
    expect(screen.getByRole("dialog", { name: /review help/i })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: /review help/i })).not.toBeInTheDocument();
  });

  it("does not trigger shortcuts while typing in a text field", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    await user.click(
      screen.getByRole("button", { name: /start guided review/i }),
    );
    await waitForReady();

    await user.click(screen.getByRole("button", { name: /^comment$/i }));
    const dialog = screen.getByRole("dialog", { name: /add comment/i });
    const box = within(dialog).getByRole("textbox");
    await user.type(box, "js?");
    expect(box).toHaveValue("js?");
    expect(screen.getByRole("dialog", { name: /add comment/i })).toBeInTheDocument();
    expect(screen.getByRole("main")).toHaveAttribute(
      "data-gewu-screen",
      "chunk",
    );
  });

  it("toggles reviewed with r and opens feedback with c/x", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    await user.keyboard("j");
    await waitForReady();

    await user.keyboard("r");
    await waitForReady();
    expect(
      screen.getByRole("button", { name: /^reviewed$/i }),
    ).toHaveAttribute("aria-pressed", "true");


    await user.keyboard("c");
    expect(screen.getByRole("dialog", { name: /add comment/i })).toBeInTheDocument();
    await user.keyboard("{Escape}");

    await user.keyboard("x");
    expect(
      screen.getByRole("dialog", { name: /request change/i }),
    ).toBeInTheDocument();
  });
});
