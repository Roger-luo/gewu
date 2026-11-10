/** Display helpers for the refactor context pane. */

const IDENTIFIER = /[A-Za-z_][A-Za-z0-9_]*/g;

/**
 * Lays a one-line brace block out as one field per line so a signature can be
 * compared at a glance. Code the author already broke across lines is kept.
 */
export function layoutCode(code: string): string {
  if (code.includes("\n") || !code.includes("{")) {
    return code;
  }
  return layoutBraces(code.trim());
}

/**
 * Identifiers the reason names that the old definition has and the new one
 * does not: the fields a reader should see marked in the before block.
 */
export function droppedIdentifiers(reason: string, before: string, after: string): string[] {
  const beforeWords = new Set(before.match(IDENTIFIER) ?? []);
  const afterWords = new Set(after.match(IDENTIFIER) ?? []);
  const dropped = new Set<string>();
  for (const span of reason.matchAll(/`([^`\n]+)`/g)) {
    for (const word of span[1].match(IDENTIFIER) ?? []) {
      if (beforeWords.has(word) && !afterWords.has(word)) {
        dropped.add(word);
      }
    }
  }
  return [...dropped];
}

/** Wraps named identifiers in highlighted HTML so the mark survives Shiki's spans. */
export function markIdentifiers(html: string, names: string[]): string {
  if (names.length === 0 || typeof DOMParser === "undefined") {
    return html;
  }
  const pattern = identifierPattern(names);
  const doc = new DOMParser().parseFromString(html, "text/html");
  const nodes: Text[] = [];
  const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    nodes.push(walker.currentNode as Text);
  }
  for (const node of nodes) {
    const text = node.data;
    pattern.lastIndex = 0;
    if (!pattern.test(text)) {
      continue;
    }
    pattern.lastIndex = 0;
    const fragment = doc.createDocumentFragment();
    let last = 0;
    for (const match of text.matchAll(pattern)) {
      const index = match.index ?? 0;
      if (index > last) {
        fragment.append(text.slice(last, index));
      }
      const mark = doc.createElement("span");
      mark.className = "dropped-field";
      mark.title = "Dropped";
      mark.textContent = match[1] ?? "";
      fragment.append(mark);
      last = index + match[0].length;
    }
    if (last < text.length) {
      fragment.append(text.slice(last));
    }
    node.parentNode?.replaceChild(fragment, node);
  }
  return doc.body.innerHTML;
}

export function identifierPattern(names: string[]): RegExp {
  const body = [...names]
    .sort((a, b) => b.length - a.length)
    .map((name) => name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("|");
  return new RegExp(`\\b(${body})\\b`, "g");
}

function layoutBraces(code: string): string {
  let brace = 0;
  let angle = 0;
  let quote: '"' | "'" | "`" | null = null;
  let out = "";
  for (let index = 0; index < code.length; index += 1) {
    const ch = code[index] ?? "";
    if (quote) {
      out += ch;
      if (ch === "\\") {
        out += code[index + 1] ?? "";
        index += 1;
      } else if (ch === quote) {
        quote = null;
      }
      continue;
    }
    if (ch === '"' || ch === "'" || ch === "`") {
      quote = ch;
      out += ch;
      continue;
    }
    if (ch === "<") {
      angle += 1;
      out += ch;
      continue;
    }
    if (ch === ">") {
      if (angle > 0) {
        angle -= 1;
      }
      out += ch;
      continue;
    }
    if (ch === "{") {
      brace += 1;
      out += `{\n${indent(brace)}`;
      while (code[index + 1] === " ") {
        index += 1;
      }
      continue;
    }
    if (ch === "}") {
      brace = Math.max(0, brace - 1);
      out = out.replace(/[ \t]+$/, "");
      if (!out.endsWith("\n")) {
        out += "\n";
      }
      out += `${indent(brace)}}`;
      continue;
    }
    if (ch === "," && brace > 0 && angle === 0) {
      out += `,\n${indent(brace)}`;
      while (code[index + 1] === " ") {
        index += 1;
      }
      continue;
    }
    out += ch;
  }
  return out;
}

function indent(depth: number): string {
  return "    ".repeat(depth);
}
