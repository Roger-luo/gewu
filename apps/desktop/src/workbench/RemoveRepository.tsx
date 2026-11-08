import { useState } from "react";

/** Removing a repository with reviews deletes them too, so it asks first. */
export function RemoveRepository({
  name,
  reviews,
  onRemove,
}: {
  name: string;
  reviews: number;
  onRemove: () => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const what = reviews === 1 ? "its review" : `its ${reviews} reviews`;
  if (!confirming) {
    return (
      <button type="button" className="btn ghost remove-repository" onClick={() => setConfirming(true)}>
        Remove {name} from Gewu…
      </button>
    );
  }
  return (
    <div className="panel confirm-remove" role="group" aria-label={`Remove ${name}`}>
      <p>
        Removing {name} also deletes {what}. The repository's own files are not touched.
      </p>
      <div className="actions-row">
        <button type="button" className="btn" onClick={() => setConfirming(false)}>
          Keep {name}
        </button>
        <button type="button" className="btn danger" onClick={onRemove}>
          Remove {name} and {what}
        </button>
      </div>
    </div>
  );
}
