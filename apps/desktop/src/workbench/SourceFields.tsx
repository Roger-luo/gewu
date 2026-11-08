import type { RepositoryRefs, SourceType } from "./protocol";
import { shortDate } from "./format";
import type { SourceDraft } from "./sourceDraft";

type Change = (patch: Partial<SourceDraft>) => void;

/** Git's empty tree; Base for a repository's first commit, which has no parent. */
const EMPTY_TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

export function SourceFields({
  type,
  draft,
  refs,
  onChange,
}: {
  type: SourceType;
  draft: SourceDraft;
  refs: RepositoryRefs | null;
  onChange: Change;
}) {
  switch (type) {
    case "commit_range":
      return <CommitRange draft={draft} refs={refs} onChange={onChange} />;
    case "working_tree":
      return (
        <div className="source-fields">
          <Text label="Base" value={draft.workingBase} onChange={(workingBase) => onChange({ workingBase })} />
          <p className="field-hint muted">
            Reviews the edits in the repository itself, against this commit. Gewu keeps one
            review of uncommitted changes per repository.
          </p>
          {refs && !refs.dirty && (
            <p className="warning">This repository has no uncommitted changes right now.</p>
          )}
        </div>
      );
    case "pull_request":
      return <PullRequest draft={draft} refs={refs} onChange={onChange} />;
    case "directory":
      return <Directory draft={draft} refs={refs} onChange={onChange} />;
  }
}

function CommitRange({ draft, refs, onChange }: { draft: SourceDraft; refs: RepositoryRefs | null; onChange: Change }) {
  return (
    <div className="source-fields two">
      <Text label="Base" value={draft.base} list="commits" onChange={(base) => onChange({ base })} />
      <Text label="Head" value={draft.head} list="commits" onChange={(head) => onChange({ head })} />
      <datalist id="commits">
        {refs?.branches.map((branch) => <option key={branch} value={branch} />)}
        {refs?.commits.map((commit) => <option key={commit.sha} value={commit.sha.slice(0, 12)} label={commit.subject} />)}
      </datalist>
      <p className="field-hint muted span">The commits after Base, up to and including Head.</p>
      {refs && refs.commits.length > 0 && (
        <fieldset className="span commit-picks">
          <legend>Or review one recent commit</legend>
          <ul>
            {refs.commits.slice(0, 8).map((commit) => {
              const head = commit.sha.slice(0, 12);
              // A root commit has no parent; compare against Git's empty tree instead.
              const base = commit.has_parent ? `${head}~1` : EMPTY_TREE;
              const chosen = draft.head === head && draft.base === base;
              return (
                <li key={commit.sha}>
                  <button
                    type="button"
                    className="commit-pick"
                    aria-pressed={chosen}
                    onClick={() => onChange({ base, head })}
                  >
                    <code>{commit.sha.slice(0, 8)}</code>
                    <span className="commit-subject">{commit.subject}</span>
                    <span className="muted">{commit.author} · {shortDate(commit.date)}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </fieldset>
      )}
    </div>
  );
}

function Directory({ draft, refs, onChange }: { draft: SourceDraft; refs: RepositoryRefs | null; onChange: Change }) {
  const directories = refs?.directories ?? [];
  return (
    <div className="source-fields two">
      <Text
        label="Directory"
        value={draft.directory}
        list="directories"
        placeholder="Whole repository"
        onChange={(directory) => onChange({ directory })}
      />
      <datalist id="directories">
        {directories.map((directory) => (
          <option key={directory} value={directory} />
        ))}
      </datalist>
      <Text label="Ref" value={draft.ref} onChange={(ref) => onChange({ ref })} />
      <p className="field-hint muted span">
        Every file under the directory at that ref, read as if it were newly added. Leave the
        directory empty to review the whole repository.
      </p>
      {directories.length > 0 && (
        <fieldset className="span commit-picks">
          <legend>Or pick a directory</legend>
          <ul aria-label="Directories">
            {directories.slice(0, 12).map((directory) => {
              const chosen = draft.directory === directory;
              return (
                <li key={directory}>
                  <button
                    type="button"
                    className="commit-pick directory-pick"
                    aria-pressed={chosen}
                    onClick={() => onChange({ directory })}
                  >
                    <code>{directory}</code>
                    <span className="muted">{chosen ? "Selected" : "Review this directory"}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </fieldset>
      )}
    </div>
  );
}

function PullRequest({ draft, refs, onChange }: { draft: SourceDraft; refs: RepositoryRefs | null; onChange: Change }) {
  const pulls = refs?.pull_requests;
  return (
    <div className="source-fields">
      {pulls === null ? (
        <p className="warning">
          Gewu could not list pull requests: the GitHub CLI (<code>gh</code>) is not installed or
          not signed in for this repository. Type the number instead.
        </p>
      ) : pulls && pulls.length === 0 ? (
        <p className="muted">This repository has no open pull requests.</p>
      ) : (
        pulls && (
          <div className="pull-list" role="listbox" aria-label="Open pull requests">
            {pulls.map((pull) => {
              const selected = draft.number === String(pull.number);
              return (
                <div
                  key={pull.number}
                  role="option"
                  aria-selected={selected}
                  tabIndex={selected || (!draft.number && pull === pulls[0]) ? 0 : -1}
                  className="pull-option"
                  onClick={() => onChange({ number: String(pull.number) })}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      onChange({ number: String(pull.number) });
                    }
                  }}
                >
                  <span className="pull-number">#{pull.number}</span>
                  <span className="pull-title">{pull.title}</span>
                  <span className="muted">{pull.author} · {pull.head_ref}</span>
                </div>
              );
            })}
          </div>
        )
      )}
      <Text label="Pull request number" value={draft.number} inputMode="numeric" onChange={(number) => onChange({ number })} />
    </div>
  );
}

function Text({
  label,
  value,
  onChange,
  list,
  placeholder,
  inputMode,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  list?: string;
  placeholder?: string;
  inputMode?: "numeric";
}) {
  return (
    <label className="field">
      <span>{label}</span>
      <input
        type="text"
        value={value}
        list={list}
        placeholder={placeholder}
        inputMode={inputMode}
        spellCheck={false}
        autoComplete="off"
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
