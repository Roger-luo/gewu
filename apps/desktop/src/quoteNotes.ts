import type { QuoteView } from "./protocol";

function citations(sentence: string): RegExpMatchArray[] {
  return [...sentence.matchAll(/(?:lines|line)\s+(\d+)(?:\s*[–—-]\s*(\d+))?/gi)];
}

export interface AssignedNotes {
  /** Sentences that name no single view, kept above the code. */
  lead: string;
  /** One note per quote, empty when nothing in the prose named that view. */
  notes: string[];
}

/**
 * Moves prose that names a view's lines onto that view. A sentence listing
 * several ranges becomes one note per range, so each code block carries the
 * instruction for reading it.
 */
export function assignNotes(markdown: string, quotes: QuoteView[]): AssignedNotes {
  const lead: string[] = [];
  const notes = quotes.map(() => [] as string[]);
  for (const sentence of sentences(markdown)) {
    const cites = citations(sentence);
    if (cites.length === 0) {
      lead.push(sentence);
      continue;
    }
    if (cites.length === 1) {
      const index = quoteFor(cites[0], quotes);
      if (index === null) lead.push(sentence);
      else notes[index].push(sentence);
      continue;
    }
    const placed = distribute(sentence, cites, quotes);
    if (placed === null) lead.push(sentence);
    else placed.forEach((note, index) => note && notes[index].push(note));
  }
  return {
    lead: lead.join("\n\n"),
    notes: notes.map((parts) => parts.join("\n\n")),
  };
}

function sentences(markdown: string): string[] {
  const parts: string[] = [];
  let current = "";
  let code = false;
  for (let i = 0; i < markdown.length; i += 1) {
    const ch = markdown[i] ?? "";
    if (ch === "`") code = !code;
    current += ch;
    const boundary = !code && (ch === "." || ch === "!" || ch === "?");
    if (boundary && /\s/.test(markdown[i + 1] ?? "")) {
      parts.push(current.trim());
      current = "";
      while (/\s/.test(markdown[i + 1] ?? "")) i += 1;
    }
  }
  if (current.trim()) parts.push(current.trim());
  return parts.filter((part) => part.length > 0);
}

/** A sentence that names several views: the symbol beside each, the shared remainder on all of them. */
function distribute(
  sentence: string,
  cites: RegExpMatchArray[],
  quotes: QuoteView[],
): string[] | null {
  const last = cites[cites.length - 1];
  if (!last) return null;
  const lastEnd = endOf(sentence, last);
  const tail = sentence.slice(lastEnd).replace(/^[\s,;)]+/, "").trim();
  const notes = quotes.map(() => "");
  let placed = false;
  cites.forEach((cite, i) => {
    const index = quoteFor(cite, quotes);
    if (index === null) return;
    const prev = i === 0 ? 0 : endOf(sentence, cites[i - 1]);
    let label = sentence.slice(prev, cite.index ?? 0);
    label = label.replace(/^[\s,;)(]+/, "").replace(/^(and|or)\s+/i, "").replace(/\(\s*$/, "").trim();
    const body = tail ? tail[0].toUpperCase() + tail.slice(1) : "";
    notes[index] = [label, body].filter(Boolean).join("\n\n");
    placed = true;
  });
  return placed ? notes : null;
}

function endOf(sentence: string, cite: RegExpMatchArray): number {
  const end = (cite.index ?? 0) + cite[0].length;
  return sentence[end] === ")" ? end + 1 : end;
}

function quoteFor(cite: RegExpMatchArray, quotes: QuoteView[]): number | null {
  const start = Number(cite[1]);
  const end = Number(cite[2] ?? cite[1]);
  let bestIndex: number | null = null;
  let bestSpan = Number.POSITIVE_INFINITY;
  for (let index = 0; index < quotes.length; index += 1) {
    const range = shownLines(quotes[index]);
    if (!range || end < range.start || start > range.end) continue;
    const span = range.end - range.start;
    if (span < bestSpan) {
      bestIndex = index;
      bestSpan = span;
    }
  }
  return bestIndex;
}

function shownLines(quote: QuoteView): { start: number; end: number } | null {
  const numbers = quote.lines.flatMap((line) => [line.new_line, line.old_line]).filter(
    (line): line is number => line !== null,
  );
  if (numbers.length === 0) return null;
  return { start: Math.min(...numbers), end: Math.max(...numbers) };
}
