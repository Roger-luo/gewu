import type { SettingsView } from "./protocol";

/** Where Gewu keeps things: set with environment variables, shown here. */
export function StorageSettings({ settings }: { settings: SettingsView }) {
  return (
    <section className="panel settings-section" aria-labelledby="settings-storage">
      <h3 id="settings-storage">Where Gewu keeps things</h3>
      <p className="muted">
        These are set before Gewu starts, with environment variables, and shown here so nothing is
        hidden.
      </p>
      <dl className="settings-paths">
        <dt>Reviews, repositories, and settings</dt>
        <dd>
          <code>{settings.home}</code> <span className="muted">· GEWU_HOME</span>
        </dd>
        <dt>Gewu command agents call</dt>
        <dd>
          <code>{settings.gewu}</code> <span className="muted">· GEWU_BIN</span>
        </dd>
        <dt>Never contact GitHub for one launch</dt>
        <dd>
          <code>GEWU_GITHUB=record</code>{" "}
          <span className="muted">· a stand-in GitHub that records instead of sending</span>
        </dd>
      </dl>
    </section>
  );
}
