import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { renderApp, waitForReady } from "./test/renderApp";

describe("publication preview", () => {
  it("shows the drafts and does not call GitHub", async () => {
    const user = userEvent.setup();
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    renderApp("showcase-feedback");
    await waitForReady();

    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await user.click(within(nav).getByRole("button", { name: /^summary$/i }));
    await waitForReady();

    const preview = screen.getByRole("region", { name: /publication preview/i });
    expect(preview).toHaveTextContent("Nothing on this page is sent");
    expect(preview).toHaveTextContent("Not sent · POST /repos/{owner}/gewu/issues");
    expect(preview).toHaveTextContent("The before/after illustration makes the orphaning failure mode clear.");
    expect(preview).toHaveTextContent("This is about the walkthrough Gewu wrote.");
    expect(preview).toHaveTextContent("Confirm the Windows path join");
    expect(preview).toHaveTextContent("Issues on this repository");
    expect(preview).not.toHaveTextContent("Link to the generation section");
    expect(fetchSpy).not.toHaveBeenCalled();
    fetchSpy.mockRestore();
  });

  it("links a GitHub account locally and shows its avatar", async () => {
    localStorage.removeItem("gewu.github");
    const user = userEvent.setup();
    renderApp("showcase");
    await waitForReady();

    const account = screen.getByRole("contentinfo", { name: /github account/i });
    await user.click(within(account).getByRole("button", { name: /sign in to github/i }));
    const dialog = screen.getByRole("dialog", { name: /link github/i });
    await user.type(within(dialog).getByRole("textbox", { name: /github login/i }), "octocat");
    await user.click(within(dialog).getByRole("button", { name: /link account/i }));

    const linked = screen.getByRole("contentinfo", { name: /github account/i });
    expect(linked).toHaveTextContent("octocat");
    expect(linked.querySelector("img")?.getAttribute("src")).toContain("data:image/svg+xml");
    expect(localStorage.getItem("gewu.github")).toContain("octocat");
    localStorage.removeItem("gewu.github");
  });
});
