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

/**
 * Show the drawing at its own coordinate size, so one unit is one pixel.
 *
 * A `max-height` on the SVG scales every label down with the picture. Mermaid's
 * font stays "16px" in CSS while the glyphs shrink, which is how an overview
 * diagram became too small to read.
 */
export function readableSvg(svg: string): string {
  return svg.replace(/<svg\b([^>]*)>/, (_match, attrs: string) => {
    const box = /viewBox="\s*[\d.]+\s+[\d.]+\s+([\d.]+)\s+([\d.]+)\s*"/.exec(attrs);
    if (!box) {
      return `<svg${attrs}>`;
    }
    const width = Math.ceil(Number(box[1]));
    const height = Math.ceil(Number(box[2]));
    const rest = attrs
      .replace(/\s(?:width|height)="[^"]*"/g, "")
      .replace(/\sstyle="[^"]*"/g, "");
    return `<svg${rest} width="${width}" height="${height}">`;
  });
}

/** A spoken name for a Mermaid diagram: its node labels, in source order. */
export function diagramLabel(source: string): string {
  const labels = [...source.matchAll(/[[({]+"?([^"\])}]+)"?[\])}]+/g)]
    .map((match) => match[1]!.trim())
    .filter((text, index, all) => text && all.indexOf(text) === index);
  return labels.length > 0 ? `Diagram of ${labels.join(", ")}` : "Diagram of the change";
}
