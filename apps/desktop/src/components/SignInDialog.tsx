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
  const [busy, setBusy] = useState<SignInMethod | null>(null);

  // Started from the click, not an effect, so one press asks GitHub for one
  // code and opens the browser once.
  const choose = async (method: SignInMethod) => {
    setBusy(method);
    const switched =
      github.methods?.method === method
        ? { account: github.account }
        : await github.chooseMethod(method);
    if (!switched) {
      setBusy(null);
      return;
    }
    if (switched.account) {
      onClose();
      return;
    }
    const next = await github.startSignIn();
    setBusy(null);
    setStart(next);
  };

  return (
    <Dialog title="Sign in to GitHub" onClose={onClose}>
      {start === null ? (
        <SignInChoice
          methods={github.methods}
          signedIn={github.account !== null}
          busy={busy}
          onChoose={(method) => void choose(method)}
        />
      ) : start.kind === "token" ? (
        <TokenForm github={github} onClose={onClose} />
      ) : (
        <DeviceCode github={github} start={start} onClose={onClose} />
      )}
      {github.error && (
        <p className="github-error" role="alert">
          {github.error}
        </p>
      )}
      {start === null && (
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onClose}>
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
  onClose,
}: {
  github: GitHubState;
  start: Extract<SignInStart, { kind: "device" }>;
  onClose: () => void;
}) {
  const [ended, setEnded] = useState<string | null>(null);
  const { pollSignIn } = github;
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);

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
        <button type="button" className="btn" onClick={onClose}>
          {ended ? "Close" : "Cancel"}
        </button>
      </div>
    </>
  );
}

function TokenForm({ github, onClose }: { github: GitHubState; onClose: () => void }) {
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    setBusy(true);
    const signedIn = await github.signIn(token.trim());
    setBusy(false);
    if (signedIn) {
      onClose();
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
        <button type="button" className="btn" onClick={onClose}>
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
