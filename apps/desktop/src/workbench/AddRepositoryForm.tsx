import { useId, useState, type FormEvent } from "react";

/** A repository path, typed or chosen with the native folder picker. */
export function AddRepositoryForm({
  onAdd,
  pickDirectory,
  autoFocus = false,
  onCancel,
}: {
  onAdd: (path: string) => Promise<boolean>;
  pickDirectory?: () => Promise<string | null>;
  autoFocus?: boolean;
  onCancel?: () => void;
}) {
  const [path, setPath] = useState("");
  const hint = useId();

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (path.trim() && (await onAdd(path.trim()))) {
      setPath("");
    }
  };

  return (
    <form className="add-repository" onSubmit={(event) => void submit(event)}>
      <label className="field">
        <span>Repository path</span>
        <input
          type="text"
          value={path}
          onChange={(event) => setPath(event.target.value)}
          placeholder="/Users/you/Code/project"
          spellCheck={false}
          autoComplete="off"
          aria-describedby={hint}
          autoFocus={autoFocus}
        />
      </label>
      <p id={hint} className="field-hint muted">
        Any folder inside a Git checkout. <code>~</code> means your home folder.
      </p>
      <div className="dialog-actions">
        {onCancel && (
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
        )}
        {pickDirectory && (
          <button
            type="button"
            className="btn"
            onClick={async () => {
              const picked = await pickDirectory();
              if (picked) {
                await onAdd(picked);
              }
            }}
          >
            Choose folder…
          </button>
        )}
        <button type="submit" className="btn primary" disabled={!path.trim()}>
          Add
        </button>
      </div>
    </form>
  );
}
