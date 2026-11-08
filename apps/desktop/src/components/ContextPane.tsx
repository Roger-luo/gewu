import type { ChunkContext } from "../protocol";
import { CodeHighlight } from "./CodeHighlight";
import { InlineMarkdown } from "./Markdown";

/** `language` is the language of the chunk's code, used for its examples. */
export function ContextPane({
  context,
  language,
}: {
  context: ChunkContext;
  language: string | null;
}) {
  return (
    <aside className="context-pane" aria-label="Context">
      <h3>{context.kind === "concept" ? "Concept" : "Refactor"}</h3>
      {context.kind === "concept" ? (
        <>
          <div className="field">
            <div className="field-label">Definition</div>
            <div>
              <InlineMarkdown source={context.definition} />
            </div>
          </div>
          <div className="field">
            <div className="field-label">Example</div>
            <CodeHighlight code={context.example} language={language} className="context-code" />
          </div>
          <div className="field">
            <div className="field-label">Relation</div>
            <div>
              <InlineMarkdown source={context.relation} />
            </div>
          </div>
        </>
      ) : (
        <>
          <div className="field">
            <div className="field-label">Reason</div>
            <div>
              <InlineMarkdown source={context.reason} />
            </div>
          </div>
          <details className="field signatures">
            <summary>Before / after signatures</summary>
            <div className="field">
              <div className="field-label">Before</div>
              <CodeHighlight code={context.before} language={language} className="context-code" />
            </div>
            <div className="field">
              <div className="field-label">After</div>
              <CodeHighlight code={context.after} language={language} className="context-code" />
            </div>
          </details>
        </>
      )}
    </aside>
  );
}
