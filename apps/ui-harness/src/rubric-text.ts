import type { Rubric } from "./types.js";

export function renderRubricText(rubric: Rubric): string {
  const lines: string[] = [
    `# ${rubric.title}`,
    ``,
    `Version: ${rubric.version}`,
    `Scale: ${rubric.scale.min}–${rubric.scale.max}`,
    ``,
  ];
  for (const [k, v] of Object.entries(rubric.scale.anchors)) {
    lines.push(`- ${k}: ${v}`);
  }
  lines.push(``, `## Dimensions`, ``);
  for (const d of rubric.dimensions) {
    lines.push(`### ${d.name} (\`${d.id}\`, weight ${d.weight})`);
    lines.push(d.question);
    lines.push(``);
    lines.push(`Look for:`);
    for (const item of d.look_for) lines.push(`- ${item}`);
    lines.push(``);
    lines.push(`Anchors:`);
    for (const [k, v] of Object.entries(d.anchors)) lines.push(`- ${k}: ${v}`);
    lines.push(``);
    lines.push(`Evidence kinds: ${d.evidence.join(", ")}`);
    lines.push(``);
  }
  if (rubric.gates.length) {
    lines.push(`## Gates`, ``);
    for (const g of rubric.gates) {
      const effect =
        g.effect === "cap" ? `cap total at ${g.cap}` : "fail the run";
      lines.push(
        `- **${g.id}**: \`${g.check}\` ≤ ${g.max} or ${effect}. ${g.description}`,
      );
    }
    lines.push(``);
  }
  if (rubric.sources.length) {
    lines.push(`## Sources`, ``);
    for (const s of rubric.sources) {
      lines.push(`- [${s.title}](${s.url}) (\`${s.id}\`)`);
    }
  }
  return lines.join("\n");
}
