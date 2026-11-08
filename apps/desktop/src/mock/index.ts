import type { Snapshot } from "../protocol";
import { selfReviewSnapshot } from "./self-review";
import { showcaseSnapshot } from "./showcase";
import { showcaseFeedbackSnapshot } from "./showcase-feedback";

const mocks: Record<string, () => Snapshot> = {
  showcase: showcaseSnapshot,
  "showcase-feedback": showcaseFeedbackSnapshot,
  "self-review": selfReviewSnapshot,
};

export function createMockSnapshot(name: string): Snapshot {
  const factory = mocks[name];
  if (!factory) {
    const known = Object.keys(mocks).join(", ");
    throw new Error(`Unknown mock "${name}". Known: ${known}`);
  }
  return factory();
}

export { showcaseSnapshot, showcaseFeedbackSnapshot, selfReviewSnapshot };
