import type { ReviewPlace } from "../protocol";

const KEY = "gewu.places";

/** Where the reader was in each task, kept across reloads. */
export class PlaceStore {
  private readonly places: Map<string, ReviewPlace>;

  constructor() {
    let saved: Record<string, ReviewPlace> = {};
    try {
      saved = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, ReviewPlace>;
    } catch {
      // A corrupt entry only costs the reader their place.
    }
    this.places = new Map(Object.entries(saved));
  }

  get(taskId: string): ReviewPlace | undefined {
    return this.places.get(taskId);
  }

  set(taskId: string, place: ReviewPlace): void {
    this.places.set(taskId, place);
    this.save();
  }

  delete(taskId: string): void {
    this.places.delete(taskId);
    this.save();
  }

  private save(): void {
    try {
      localStorage.setItem(KEY, JSON.stringify(Object.fromEntries(this.places)));
    } catch {
      // Storage may be full or disabled; the place still holds for this session.
    }
  }
}
