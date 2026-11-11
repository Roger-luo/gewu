import type { Snapshot } from "../protocol";
import { explainersSnapshot } from "./explainers";
import { pairedSnapshot } from "./paired";
import { paletteSnapshot } from "./palette";
import { selfReviewSnapshot } from "./self-review";
import { showcaseSnapshot } from "./showcase";
import { showcaseFeedbackSnapshot } from "./showcase-feedback";

const mocks: Record<string, () => Snapshot> = {
  showcase: showcaseSnapshot,
  "showcase-feedback": showcaseFeedbackSnapshot,
  "self-review": selfReviewSnapshot,
  palette: paletteSnapshot,
  paired: pairedSnapshot,
  explainers: explainersSnapshot,
};

export function createMockSnapshot(name: string): Snapshot {
  const factory = mocks[name];
  if (!factory) {
    const known = Object.keys(mocks).join(", ");
    throw new Error(`Unknown mock "${name}". Known: ${known}`);
  }
  return factory();
}

export {
  showcaseSnapshot,
  showcaseFeedbackSnapshot,
  selfReviewSnapshot,
  paletteSnapshot,
  pairedSnapshot,
  explainersSnapshot,
};
