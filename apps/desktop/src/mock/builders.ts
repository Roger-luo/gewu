import type {
  BlockView,
  ChunkView,
  Feedback,
  FileView,
  Overview,
  QuoteLine,
  QuoteView,
  Snapshot,
  SummaryView,
} from "../protocol";

export function line(
  kind: QuoteLine["kind"],
  text: string,
  old_line: number | null,
  new_line: number | null,
): QuoteLine {
  return { kind, text, old_line, new_line };
}

export function quote(
  partial: Omit<QuoteView, "more_above" | "more_below"> &
    Partial<Pick<QuoteView, "more_above" | "more_below">>,
): QuoteView {
  return {
    more_above: false,
    more_below: false,
    ...partial,
  };
}

export function narrative(markdown: string): BlockView {
  return { type: "narrative", markdown };
}

export function changes(
  presentation: Extract<BlockView, { type: "changes" }>["presentation"],
  quotes: QuoteView[],
): BlockView {
  return { type: "changes", presentation, quotes };
}

export function chunk(
  partial: Omit<ChunkView, "status" | "context" | "confidence" | "oversized_reason" | "section"> &
    Partial<
      Pick<
        ChunkView,
        "status" | "context" | "confidence" | "oversized_reason" | "section"
      >
    >,
): ChunkView {
  return {
    section: null,
    confidence: null,
    oversized_reason: null,
    context: null,
    status: {
      visited: false,
      reviewed: false,
      open_issues: 0,
      feedback_count: 0,
    },
    ...partial,
  };
}

export function emptySummary(total: number): SummaryView {
  return {
    reviewed: 0,
    total,
    unreviewed: [],
    unresolved_issues: [],
    decision: null,
    suggested_decision: "approve",
    batch_scoped: false,
  };
}

export function snapshot(partial: {
  review_id: string;
  source?: Snapshot["source"];
  overview: Overview;
  generation?: Snapshot["generation"];
  chunks: ChunkView[];
  feedback?: Feedback[];
  last_visited?: string | null;
}): Snapshot {
  const chunks = partial.chunks.map((item, index) => ({
    ...item,
    index: item.index ?? index,
  }));
  return {
    review_id: partial.review_id,
    source: partial.source ?? { base: "main", head: "feature/demo" },
    overview: partial.overview,
    files: quotedFiles(chunks),
    generation: partial.generation ?? {
      remaining_work_items: 0,
      complete: true,
    },
    chunks,
    feedback: partial.feedback ?? [],
    summary: emptySummary(chunks.length),
    last_visited: partial.last_visited ?? null,
  };
}

/** The files the mock's chunks quote, counted over quoted lines. */
function quotedFiles(chunks: ChunkView[]): FileView[] {
  const files = new Map<string, FileView>();
  const seen = new Set<string>();
  for (const block of chunks.flatMap((item) => item.blocks)) {
    if (block.type !== "changes") {
      continue;
    }
    for (const view of block.quotes) {
      const file = files.get(view.path) ?? { path: view.path, added: 0, removed: 0, shown: true };
      files.set(view.path, file);
      view.lines.forEach((row, offset) => {
        const key = `${view.change_id}:${view.start + offset}`;
        if (!seen.has(key)) {
          seen.add(key);
          if (row.kind === "added") file.added += 1;
          if (row.kind === "removed") file.removed += 1;
        }
      });
    }
  }
  return [...files.values()].sort((a, b) => a.path.localeCompare(b.path));
}
