import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import {
  holdMockApproval,
  MOCK_CLI_LOGIN,
  MOCK_CODE,
  mockGitHub,
  signOutMockGitHub,
} from "./backend/mockGitHub";
import { renderApp, waitForReady } from "./test/renderApp";

async function openSummary(user: ReturnType<typeof userEvent.setup>) {
  const nav = screen.getByRole("navigation", { name: /review navigator/i });
  await user.click(within(nav).getByRole("button", { name: /^summary$/i }));
  await waitForReady();
  return screen.findByRole("region", { name: /publication preview/i });
}

/** The device flow: the dialog shows the code and closes once GitHub approves it. */
async function signIn(user: ReturnType<typeof userEvent.setup>) {
  const account = await screen.findByRole("button", { name: /sign in to github/i });
  await user.click(account);
  const dialog = await screen.findByRole("dialog", { name: /sign in to github/i });
  await user.click(within(dialog).getByRole("button", { name: /sign in with your browser/i }));
  expect(within(dialog).getByLabelText(/device code/i)).toHaveTextContent(MOCK_CODE);
  await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
}

describe("publication preview", () => {
  beforeEach(() => signOutMockGitHub());
  afterEach(() => {
    signOutMockGitHub();
    holdMockApproval(false);
  });

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
    expect(preview).toHaveTextContent("This is about a walkthrough Gewu wrote.");
    expect(preview).toHaveTextContent("(public; only what you wrote about the walkthrough)");
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
    const account = screen.getByRole("region", { name: /account and settings/i });
    expect(account).toHaveTextContent("octocat");
    expect(account.querySelector("img")?.getAttribute("src")).toContain("data:image/svg+xml");
    expect(within(preview).getByRole("button", { name: /^publish/i })).toBeEnabled();
  });

  it("signs out and forgets the account", async () => {
    const user = userEvent.setup();
    renderApp("showcase-feedback");
    await waitForReady();
    await signIn(user);

    const account = screen.getByRole("region", { name: /account and settings/i });
    await user.click(within(account).getByRole("button", { name: /switch/i }));
    const dialog = screen.getByRole("dialog", { name: /github account/i });
    await user.click(within(dialog).getByRole("button", { name: /sign out of gewu's app/i }));

    expect(await within(account).findByRole("button", { name: /sign in to github/i })).toBeInTheDocument();
    expect(account).not.toHaveTextContent("octocat");
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

  it("uses the GitHub CLI's login without a code, and switches back", async () => {
    const user = userEvent.setup();
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    renderApp("showcase-feedback");
    await waitForReady();

    await user.click(await screen.findByRole("button", { name: /sign in to github/i }));
    const dialog = screen.getByRole("dialog", { name: /sign in to github/i });
    const methods = within(dialog).getByRole("group", { name: /sign-in method/i });
    const [first] = within(methods).getAllByRole("button");
    expect(first).toHaveTextContent(`Post as ${MOCK_CLI_LOGIN} through gh`);
    await user.click(first);

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    const account = screen.getByRole("region", { name: /account and settings/i });
    expect(account).toHaveTextContent(MOCK_CLI_LOGIN);
    expect(account).toHaveTextContent("via GitHub CLI");

    const preview = await openSummary(user);
    expect(within(preview).getByRole("button", { name: /^publish/i })).toBeEnabled();

    await user.click(within(account).getByRole("button", { name: /switch/i }));
    const again = screen.getByRole("dialog", { name: /github account/i });
    expect(within(again).queryByRole("button", { name: /sign out/i })).not.toBeInTheDocument();
    expect(within(again).getByRole("button", { name: /use the github cli/i })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.click(within(again).getByRole("button", { name: /sign in with your browser/i }));
    expect(within(again).getByLabelText(/device code/i)).toHaveTextContent(MOCK_CODE);
    await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(account).toHaveTextContent("octocat");
    expect(account).toHaveTextContent("via Gewu's GitHub app");
    expect(fetchSpy).not.toHaveBeenCalled();
    fetchSpy.mockRestore();
  });

  it("rehearses without sending or marking notes as sent", async () => {
    const user = userEvent.setup();
    await mockGitHub.updateSettings({ method: "gewu", client_id: null, rehearse: true });
    const { backend } = renderApp("showcase-feedback");
    await waitForReady();
    const preview = await openSummary(user);
    await signIn(user);

    await user.click(within(preview).getByRole("button", { name: /^rehearse publishing/i }));
    const confirm = screen.getByRole("dialog", { name: /publish to github/i });
    expect(confirm).toHaveTextContent("send nothing");
    await user.click(within(confirm).getByRole("button", { name: /^rehearse$/i }));

    expect(await within(preview).findByRole("status")).toHaveTextContent(/Rehearsed \d+ items/);
    expect(preview).toHaveTextContent("Nothing was sent to GitHub");
    expect(preview).toHaveTextContent("Rehearsal is on in Settings");
    expect(backend.requestsSent).toBe(0);
    expect(within(preview).queryByText("Already on GitHub")).not.toBeInTheDocument();
    expect(within(preview).getByRole("button", { name: /^rehearse publishing/i })).toBeInTheDocument();
  });

  it("goes back to the CLI when a switch to the browser is cancelled", async () => {
    const user = userEvent.setup();
    renderApp("showcase-feedback");
    await waitForReady();
    await user.click(await screen.findByRole("button", { name: /sign in to github/i }));
    await user.click(screen.getByRole("button", { name: /use the github cli/i }));
    const account = screen.getByRole("region", { name: /account and settings/i });
    expect(account).toHaveTextContent(MOCK_CLI_LOGIN);

    await user.click(within(account).getByRole("button", { name: /switch/i }));
    const dialog = screen.getByRole("dialog", { name: /github account/i });
    holdMockApproval(true);
    await user.click(within(dialog).getByRole("button", { name: /sign in with your browser/i }));
    expect(within(dialog).getByLabelText(/device code/i)).toHaveTextContent(MOCK_CODE);
    await user.click(within(dialog).getByRole("button", { name: /cancel/i }));
    holdMockApproval(false);

    await vi.waitFor(() => expect(account).toHaveTextContent("via GitHub CLI"));
    expect(account).toHaveTextContent(MOCK_CLI_LOGIN);
  });
});
