import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { signOutMockGitHub } from "./backend/mockGitHub";
import { renderApp, waitForReady } from "./test/renderApp";

async function openSummary(user: ReturnType<typeof userEvent.setup>) {
  const nav = screen.getByRole("navigation", { name: /review navigator/i });
  await user.click(within(nav).getByRole("button", { name: /^summary$/i }));
  await waitForReady();
  return screen.findByRole("region", { name: /publication preview/i });
}

async function signIn(user: ReturnType<typeof userEvent.setup>) {
  const account = await screen.findByRole("button", { name: /sign in to github/i });
  await user.click(account);
  const dialog = screen.getByRole("dialog", { name: /sign in to github/i });
  await user.type(within(dialog).getByLabelText(/github token/i), "ghp_example");
  await user.click(within(dialog).getByRole("button", { name: /^sign in$/i }));
}

describe("publication preview", () => {
  beforeEach(() => signOutMockGitHub());
  afterEach(() => signOutMockGitHub());

  it("shows the drafts with real request paths and does not call GitHub", async () => {
    const user = userEvent.setup();
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    renderApp("showcase-feedback");
    await waitForReady();

    const preview = await openSummary(user);
    expect(preview).toHaveTextContent("Nothing is sent until you press Publish");
    expect(preview).toHaveTextContent("Not sent · POST /repos/Roger-luo/gewu/issues");
    expect(preview).toHaveTextContent("Not sent · POST /repos/acme/demo/issues");
    expect(preview).toHaveTextContent("The before/after illustration makes the orphaning failure mode clear.");
    expect(preview).toHaveTextContent("This is about the walkthrough Gewu wrote.");
    expect(preview).toHaveTextContent("Confirm the Windows path join");
    expect(preview).not.toHaveTextContent("Link to the generation section");
    expect(fetchSpy).not.toHaveBeenCalled();
    fetchSpy.mockRestore();
  });

  it("asks for sign-in before publishing", async () => {
    const user = userEvent.setup();
    renderApp("showcase-feedback");
    await waitForReady();

    const preview = await openSummary(user);
    expect(within(preview).getByRole("button", { name: /^publish/i })).toBeDisabled();
    expect(preview).toHaveTextContent("Sign in to GitHub at the bottom of the sidebar");

    await signIn(user);
    const account = screen.getByRole("contentinfo", { name: /github account/i });
    expect(account).toHaveTextContent("octocat");
    expect(account.querySelector("img")?.getAttribute("src")).toContain("data:image/svg+xml");
    expect(within(preview).getByRole("button", { name: /^publish/i })).toBeEnabled();
  });

  it("sends nothing when the confirmation is cancelled", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp("showcase-feedback");
    await waitForReady();
    const preview = await openSummary(user);
    await signIn(user);

    await user.click(within(preview).getByRole("button", { name: /^publish/i }));
    const confirm = screen.getByRole("dialog", { name: /publish to github/i });
    expect(confirm).toHaveTextContent("as octocat");
    expect(confirm).toHaveTextContent("POST /repos/acme/demo/issues");
    await user.click(within(confirm).getByRole("button", { name: /cancel/i }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(backend.requestsSent).toBe(0);
  });

  it("publishes once after confirmation and lists what was posted", async () => {
    const user = userEvent.setup();
    const { backend } = renderApp("showcase-feedback");
    await waitForReady();
    const preview = await openSummary(user);
    await signIn(user);

    await user.click(within(preview).getByRole("button", { name: /^publish/i }));
    const confirm = screen.getByRole("dialog", { name: /publish to github/i });
    await user.click(within(confirm).getByRole("button", { name: /send to github/i }));

    expect(await within(preview).findByRole("status")).toHaveTextContent(/Sent \d+ items to GitHub/);
    expect(backend.requestsSent).toBeGreaterThan(0);
    expect(await within(preview).findByText("Already on GitHub")).toBeInTheDocument();
    expect(within(preview).getAllByRole("link")[0]).toHaveAttribute(
      "href",
      expect.stringContaining("https://github.com/acme/demo/issues/"),
    );
    expect(preview).toHaveTextContent("No open notes to publish.");
    expect(within(preview).queryByRole("button", { name: /^publish/i })).not.toBeInTheDocument();
  });
});
