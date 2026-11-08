import { diagramLabel, flowLabel, mermaidFlow } from "../diagram";
import type { Illustration } from "../protocol";
import { DiagramPlot } from "./DiagramPlot";

export function IllustrationView({ illustration }: { illustration: Illustration }) {
  switch (illustration.kind) {
    case "before_after":
      return (
        <div className="illustration">
          <div className="pair">
            <div className="card">
              <div className="field-label">Before</div>
              <pre>{illustration.before}</pre>
            </div>
            <div className="card">
              <div className="field-label">After</div>
              <pre>{illustration.after}</pre>
            </div>
          </div>
        </div>
      );
    case "flow":
      return (
        <div className="illustration">
          <DiagramPlot
            source={mermaidFlow(illustration.steps)}
            label={flowLabel(illustration.steps)}
          />
        </div>
      );
    case "example":
      return (
        <div className="illustration">
          <div className="pair">
            <div className="card">
              <div className="field-label">Input</div>
              <pre>{illustration.input}</pre>
            </div>
            <div className="card">
              <div className="field-label">Output</div>
              <pre>{illustration.output}</pre>
            </div>
          </div>
        </div>
      );
    case "diagram":
      return (
        <div className="illustration">
          <DiagramPlot source={illustration.text} label={diagramLabel(illustration.text)} />
        </div>
      );
  }
}
