import { useEffect, useRef, useState } from "react";

import type { SignInMethod, SignInStart } from "../backend/types";
import type { GitHubState } from "../hooks/useGitHub";
import { Dialog } from "../workbench/Dialog";
import { SignInChoice } from "./SignInChoice";

/**
 * Signs in to GitHub. The reviewer first picks a method: Gewu's app, which
 * shows a device code to approve on github.com, or the GitHub CLI, which is
 * usually signed in already and otherwise takes a pasted token.
 */
export function SignInDialog({
  github,
  onClose,
}: {
  github: GitHubState;
  onClose: () => void;
}) {
  const [start, setStart] = useState<SignInStart | null>(null);
  // Read once, so the title does not change mid-sign-in.
  const [signedIn] = useState(github.account !== null);
  const [busy, setBusy] = useState<SignInMethod | null>(null);
  // The method in use before this dialog switched away from it. Leaving
  // without finishing the new sign-in goes back to it, so Cancel never
  // signs the reviewer out.
  const [previous, setPrevious] = useState<SignInMethod | null>(null);
  const cancel = () => {
    if (previous) void github.chooseMethod(previous);
    onClose();
  };

  // Started from the click, not an effect, so one press asks GitHub for one
  // code and opens the browser once.
  const choose = async (method: SignInMethod) => {
    setBusy(method);
    const current = github.settings?.method ?? null;
    const switched =
      current === method ? { account: github.account } : await github.chooseMethod(method);
    if (!switched) {
      setBusy(null);
      return;
    }
    if (switched.account) {
      onClose();
      return;
    }
    if (current !== method && github.account) setPrevious(current);
    const next = await github.startSignIn();
    setBusy(null);
    setStart(next);
  };

  return (
    <Dialog title={signedIn ? "GitHub account" : "Sign in to GitHub"} onClose={cancel}>
      {start === null ? (
        <SignInChoice
          settings={github.settings}
          signedIn={github.account !== null}
          busy={busy}
          onChoose={(method) => void choose(method)}
        />
      ) : start.kind === "token" ? (
        <TokenForm github={github} onDone={onClose} onCancel={cancel} />
      ) : (
        <DeviceCode github={github} start={start} onDone={onClose} onCancel={cancel} />
      )}
      {github.error && (
        <p className="github-error" role="alert">
          {github.error}
        </p>
      )}
      {start === null && (
        <div className="dialog-actions">
          {github.account && github.settings?.method === "gh" && (
            <button
              type="button"
              className="btn dialog-action-start"
              onClick={() => void github.signOut().then(onClose)}
            >
              Stop using the GitHub CLI
            </button>
          )}
          {github.account && github.settings?.method === "gewu" && (
            <button
              type="button"
              className="btn dialog-action-start"
              onClick={() => void github.signOut().then(onClose)}
            >
              Sign out of Gewu&apos;s app
            </button>
          )}
          <button type="button" className="btn" onClick={cancel}>
            Cancel
          </button>
        </div>
      )}
    </Dialog>
  );
}

/** Shows the device code and polls until GitHub reports it approved. */
function DeviceCode({
  github,
  start,
  onDone,
  onCancel,
}: {
  github: GitHubState;
  start: Extract<SignInStart, { kind: "device" }>;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [ended, setEnded] = useState<string | null>(null);
  const { pollSignIn } = github;
  const close = useRef(onDone);
  useEffect(() => {
    close.current = onDone;
  }, [onDone]);

  useEffect(() => {
    let live = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = (seconds: number) => {
      timer = setTimeout(async () => {
        const result = await pollSignIn();
        if (!live || !result) return;
        switch (result.status) {
          case "signed_in":
            close.current();
            break;
          case "pending":
            poll(seconds);
            break;
          case "slow_down":
            poll(result.interval);
            break;
          case "denied":
            setEnded("The code was declined on GitHub.");
            break;
          case "expired":
            setEnded("The code expired before it was approved.");
            break;
        }
      }, seconds * 1000);
    };
    poll(start.interval);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [start, pollSignIn]);

  return (
    <>
      <p className="muted">
        Your browser opens GitHub&apos;s device page. Enter this code there and
        approve Gewu. It can then file issues and post reviews on public and
        private repositories as you, and only when you press Publish.
      </p>
      <p className="sign-in-code" aria-label="Device code">
        {start.user_code}
      </p>
      <p className="muted">
        Not open? Go to <code>{start.verification_uri}</code>.
      </p>
      {ended ? (
        <p className="github-error" role="alert">
          {ended}
        </p>
      ) : (
        <p role="status">Waiting for approval on GitHub…</p>
      )}
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onCancel}>
          {ended ? "Close" : "Cancel"}
        </button>
      </div>
    </>
  );
}

function TokenForm({
  github,
  onDone,
  onCancel,
}: {
  github: GitHubState;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    setBusy(true);
    const signedIn = await github.signIn(token.trim());
    setBusy(false);
    if (signedIn) {
      onDone();
    }
  };
  return (
    <>
      <p className="muted">
        The GitHub CLI is not signed in. Run <code>gh auth login</code> in a
        terminal and choose it again, or paste a token that can write issues and
        pull-request reviews; Gewu hands it to{" "}
        <code>gh auth login --with-token</code>, which keeps it.
      </p>
      <label>
        <span>GitHub token</span>
        <input
          type="password"
          autoComplete="off"
          value={token}
          onChange={(event) => setToken(event.target.value)}
        />
      </label>
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onCancel}>
          Cancel
        </button>
        <button
          type="button"
          className="btn primary"
          disabled={!token.trim() || busy}
          onClick={() => void submit()}
        >
          {busy ? "Signing in…" : "Sign in"}
        </button>
      </div>
    </>
  );
}
