import type { ChunkContext } from "../protocol";

export function ContextPane({ context }: { context: ChunkContext }) {
  return (
    <aside className="context-pane" aria-label="Context">
      <h3>{context.kind === "concept" ? "Concept" : "Refactor"}</h3>
      {context.kind === "concept" ? (
        <>
          <div className="field">
            <div className="field-label">Definition</div>
            <div>{context.definition}</div>
          </div>
          <div className="field">
            <div className="field-label">Example</div>
            <pre className="code-plain">{context.example}</pre>
          </div>
          <div className="field">
            <div className="field-label">Relation</div>
            <div>{context.relation}</div>
          </div>
        </>
      ) : (
        <>
          <div className="field">
            <div className="field-label">Reason</div>
            <div>{context.reason}</div>
          </div>
          <details className="field signatures">
            <summary>Before / after signatures</summary>
            <div className="field">
              <div className="field-label">Before</div>
              <pre className="code-plain">{context.before}</pre>
            </div>
            <div className="field">
              <div className="field-label">After</div>
              <pre className="code-plain">{context.after}</pre>
            </div>
          </details>
        </>
      )}
    </aside>
  );
}
