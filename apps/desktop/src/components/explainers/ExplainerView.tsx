import { MotionConfig } from "motion/react";
import "./explainers.css";

import type { Explainer } from "../../explainer";
import { ClaimCard } from "./ClaimCard";
import { CostModelView } from "./CostModelView";
import { LayoutView } from "./LayoutView";

/** One explainer block. Motion follows the reader's reduce-motion setting. */
export function ExplainerView({ explainer }: { explainer: Explainer }) {
  return (
    <MotionConfig reducedMotion="user">
      {explainer.kind === "claim" && <ClaimCard explainer={explainer} />}
      {explainer.kind === "layout" && <LayoutView explainer={explainer} />}
      {explainer.kind === "cost_model" && <CostModelView explainer={explainer} />}
    </MotionConfig>
  );
}
