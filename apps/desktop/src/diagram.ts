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
