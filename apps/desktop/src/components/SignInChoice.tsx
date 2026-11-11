import type { CliState, SignInMethod, SignInMethods } from "../backend/types";

/**
 * The two ways to sign in. The GitHub CLI suits reviewers who already use
 * `gh`, and organizations that have approved it but not Gewu's app.
 */
export function SignInChoice({
  methods,
  signedIn,
  busy,
  onChoose,
}: {
  methods: SignInMethods | null;
  /** Whether the method in use is signed in. */
  signedIn: boolean;
  busy: SignInMethod | null;
  onChoose: (method: SignInMethod) => void;
}) {
  const cli = methods?.cli ?? { status: "missing" };
  const inUse = (method: SignInMethod) => signedIn && methods?.method === method;
  const options = [
    <Option
      key="gewu"
      title="Sign in with your browser"
      detail="Approve Gewu's GitHub app with a one-time code. The token stays in your keychain. Some organizations must approve the app before it can post there."
      inUse={inUse("gewu")}
      busy={busy === "gewu"}
      disabled={busy !== null}
      onChoose={() => onChoose("gewu")}
    />,
    <Option
      key="gh"
      title="Use the GitHub CLI"
      detail={cliDetail(cli)}
      inUse={inUse("gh")}
      busy={busy === "gh"}
      disabled={busy !== null || cli.status === "missing"}
      onChoose={() => onChoose("gh")}
    />,
  ];
  return (
    <>
      <p className="muted">
        Choose how Gewu posts as you. Either way, nothing is sent until you press
        Publish and confirm.
      </p>
      <div className="sign-in-methods" role="group" aria-label="Sign-in method">
        {cli.status === "signed_in" ? options.reverse() : options}
      </div>
    </>
  );
}

function cliDetail(cli: CliState): string {
  switch (cli.status) {
    case "signed_in":
      return `Post as ${cli.login} through gh, which is already signed in. Nothing else to set up.`;
    case "signed_out":
      return "gh is installed but not signed in. Run `gh auth login` in a terminal, or paste a token next.";
    case "missing":
      return "Install the GitHub CLI (cli.github.com) to use its sign-in.";
  }
}

function Option({
  title,
  detail,
  inUse,
  busy,
  disabled,
  onChoose,
}: {
  title: string;
  detail: string;
  inUse: boolean;
  busy: boolean;
  disabled: boolean;
  onChoose: () => void;
}) {
  return (
    <button
      type="button"
      className="sign-in-method"
      aria-pressed={inUse}
      disabled={disabled}
      onClick={onChoose}
    >
      <strong>
        {title}
        {inUse && <span className="sign-in-in-use"> · In use</span>}
        {busy && <span className="muted"> · Working…</span>}
      </strong>
      <span className="muted">{detail}</span>
    </button>
  );
}
