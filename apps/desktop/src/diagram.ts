/** A straight sequence, drawn as a Mermaid flowchart. */
export function mermaidFlow(steps: string[]): string {
  const lines = ["flowchart TD"];
  steps.forEach((step, index) => {
    lines.push(`  s${index}["${step.replaceAll('"', "'")}"]`);
    if (index > 0) {
      lines.push(`  s${index - 1} --> s${index}`);
    }
  });
  return lines.join("\n");
}

/** A spoken name for a sequence: each step in order. */
export function flowLabel(steps: string[]): string {
  return `Flow: ${steps.join(", then ")}`;
}

/** A spoken name for a Mermaid diagram: its node labels, in source order. */
export function diagramLabel(source: string): string {
  const labels = [...source.matchAll(/[[({]+"?([^"\])}]+)"?[\])}]+/g)]
    .map((match) => match[1]!.trim())
    .filter((text, index, all) => text && all.indexOf(text) === index);
  return labels.length > 0 ? `Diagram of ${labels.join(", ")}` : "Diagram of the change";
}
