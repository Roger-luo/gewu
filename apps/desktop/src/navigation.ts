import { plainInline } from "./markdown";
import type { GewuTestPage, ReviewPlace, Screen, Snapshot, ReviewTarget } from "./protocol";

export function pagesFromSnapshot(snapshot: Snapshot): GewuTestPage<ReviewTarget>[] {
  const pages: GewuTestPage<ReviewTarget>[] = [
    { id: "overview", label: "Overview", target: { screen: "overview" } },
  ];
  for (const [index, chunk] of snapshot.chunks.entries()) {
    pages.push({
      id: `chunk-${chunk.id}`,
      label: plainInline(chunk.title),
      target: { screen: "chunk", index },
    });
  }
  pages.push({
    id: "summary",
    label: "Summary",
    target: { screen: "summary" },
  });
  return pages;
}

export function pageIndex(
  pages: GewuTestPage<ReviewTarget>[],
  screen: Screen,
  chunkIndex: number | null,
): number {
  return pages.findIndex((page) => {
    if (page.target.screen !== screen) {
      return false;
    }
    if (screen === "chunk" && page.target.screen === "chunk") {
      return page.target.index === chunkIndex;
    }
    return true;
  });
}

export function neighborTarget(
  pages: GewuTestPage<ReviewTarget>[],
  screen: Screen,
  chunkIndex: number | null,
  delta: -1 | 1,
): ReviewTarget | null {
  const current = pageIndex(pages, screen, chunkIndex);
  if (current < 0) {
    return null;
  }
  const next = pages[current + delta];
  return next?.target ?? null;
}

/** Where to go after marking a chunk reviewed: the next unreviewed chunk, wrapping, else the summary. */
export function afterReviewed(snapshot: Snapshot, chunkIndex: number): ReviewTarget {
  const count = snapshot.chunks.length;
  for (let step = 1; step < count; step++) {
    const index = (chunkIndex + step) % count;
    if (!snapshot.chunks[index]?.status.reviewed) {
      return { screen: "chunk", index };
    }
  }
  return { screen: "summary" };
}

/** Where the guided path picks up: the first unreviewed chunk, else the first. */
export function firstUnreviewed(snapshot: Snapshot): number {
  return Math.max(0, snapshot.chunks.findIndex((chunk) => !chunk.status.reviewed));
}

/**
 * Where to reopen a review. A reader who left on the summary has finished what
 * existed then, so chunks that arrived since take them to the first unreviewed one.
 */
export function resumeTarget(snapshot: Snapshot, place: ReviewPlace): ReviewTarget {
  if (place.screen === "summary") {
    const index = snapshot.chunks.findIndex(
      (chunk, at) => at >= place.chunkCount && !chunk.status.reviewed,
    );
    if (index !== -1) {
      return { screen: "chunk", index };
    }
  }
  if (place.screen === "chunk" && place.chunkIndex !== null) {
    return { screen: "chunk", index: place.chunkIndex };
  }
  return { screen: place.screen === "chunk" ? "overview" : place.screen };
}

export function sectionPosition(
  snapshot: Snapshot,
  chunkIndex: number,
): string | null {
  const chunk = snapshot.chunks[chunkIndex];
  if (!chunk?.section) {
    return null;
  }
  const peers = snapshot.chunks.filter((item) => item.section === chunk.section);
  if (peers.length < 2) {
    return chunk.section;
  }
  const position = peers.findIndex((item) => item.id === chunk.id) + 1;
  return `${chunk.section} · ${position} of ${peers.length}`;
}

export function chunkNavState(
  chunk: Snapshot["chunks"][number],
): "reviewed" | "unresolved" | "unreviewed" {
  if (chunk.status.open_issues > 0) {
    return "unresolved";
  }
  if (chunk.status.reviewed) {
    return "reviewed";
  }
  return "unreviewed";
}
