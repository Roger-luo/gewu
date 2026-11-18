import type { WriteAhead } from "./protocol";

/**
 * How far the agent writes ahead of the reader: no limit, the default,
 * since agents write faster than people read, or a number of batches.
 */
export function WriteAheadField({
  value,
  max,
  onChange,
}: {
  value: WriteAhead;
  max: number;
  onChange: (value: WriteAhead) => void;
}) {
  const limited = value !== "unlimited";
  const batches = limited ? value.batches : 1;
  return (
    <>
      <label className="field">
        <span>Write ahead of you</span>
        <select
          value={limited ? "limited" : "unlimited"}
          onChange={(event) =>
            onChange(event.target.value === "unlimited" ? "unlimited" : { batches })
          }
        >
          <option value="unlimited">No limit: the whole walkthrough while you read</option>
          <option value="limited">Up to a number of batches</option>
        </select>
      </label>
      {limited && (
        <label className="field">
          <span>Batches ahead</span>
          <input
            type="number"
            min={0}
            max={max}
            value={batches}
            onChange={(event) => onChange({ batches: Number(event.target.value) })}
          />
        </label>
      )}
      <p className="muted field-hint">
        {limited
          ? `The agent pauses once this many batches beyond the one you are reading are written, and
             writes on as you mark steps reviewed; 0 keeps it level with you. Between 0 and ${max}.`
          : "The agent writes every batch while you read the first, so the next is always ready. A request you make later may send some of that work back."}{" "}
        Applies to reviews from their next batch.
      </p>
    </>
  );
}
