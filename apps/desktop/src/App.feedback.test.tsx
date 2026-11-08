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

  it("marking a chunk reviewed opens the next unreviewed one", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    await user.click(
      screen.getByRole("button", { name: /start guided review/i }),
    );
    await waitForReady();
    const first = screen.getByRole("main").getAttribute("data-gewu-chunk-id");

    const toolbar = screen.getByRole("group", { name: /mark this step/i });
    await user.click(within(toolbar).getByRole("button", { name: /mark reviewed/i }));
    await waitForReady();

    const main = screen.getByRole("main");
    expect(main).toHaveAttribute("data-gewu-screen", "chunk");
    expect(main.getAttribute("data-gewu-chunk-id")).not.toBe(first);
    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    expect(
      within(nav).getAllByRole("button", { name: /, reviewed$/i }).length,
    ).toBeGreaterThan(0);
  });

  it("marking the last unreviewed chunk opens the summary", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    await user.click(
      screen.getByRole("button", { name: /start guided review/i }),
    );
    await waitForReady();
    for (;;) {
      await user.click(screen.getByRole("button", { name: /^mark reviewed$/i }));
      await waitForReady();
      if (screen.getByRole("main").getAttribute("data-gewu-screen") === "summary") {
        break;
      }
    }
    expect(screen.getByText("All generated chunks are reviewed.")).toBeInTheDocument();
  });

  it("renders inline Markdown in chunk titles and summaries", async () => {
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();
    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(
      within(nav).getByRole("button", { name: /^Event path wiring via session_key\(\),/ }),
    );
    await waitForReady();

    const main = screen.getByRole("main", { name: "Event path wiring via session_key()" });
    const title = within(main).getByRole("heading", { level: 2 });
    expect(title.textContent).toBe("Event path wiring via session_key()");
    expect(title.querySelector("code")?.textContent).toBe("session_key()");
    expect(main.querySelector(".chunk-summary code")?.textContent).toBe("session_key()");
    expect(main.querySelector(".eyebrow code")?.textContent).toBe("review_id");
    expect(main.querySelector(".notice.uncertainty code")?.textContent).toBe("session_key()");
    expect(main.textContent).not.toContain("`");
    expect(nav.querySelector(".section-label code")?.textContent).toBe("review_id");
    expect(nav.textContent).not.toContain("`");
  });
});
