import { motion } from "motion/react";

import type { Explainer } from "../../explainer";
import { LayoutMorph } from "./LayoutMorph";

/** Records drawn to scale: each field's width is its share of the largest record. */
export function LayoutView({ explainer }: { explainer: Extract<Explainer, { kind: "layout" }> }) {
  const totals = explainer.rows.map((row) => row.fields.reduce((sum, field) => sum + field.bytes, 0));
  const widest = Math.max(...totals, 1);
  return (
    <figure className="explainer layout-view" aria-label={explainer.title}>
      <figcaption className="explainer-label">{explainer.title}</figcaption>
      {explainer.rows.length > 1 && <LayoutMorph rows={explainer.rows} widest={widest} />}
      <div className="layout-rows">
        {explainer.rows.map((row, index) => (
          <div className="layout-row" key={row.label}>
            <span className="layout-row-label">{row.label}</span>
            <div className="layout-track" aria-hidden>
              <motion.div
                className="layout-record"
                initial={{ width: 0 }}
                animate={{ width: `${(totals[index] / widest) * 100}%` }}
                transition={{ duration: 0.6, delay: index * 0.15, ease: "easeOut" }}
              >
                {row.fields.map((field) => {
                  // Exactly its share of the record; a label that does not
                  // fit is left out, and the legend below names every field.
                  const share = (field.bytes / totals[index]) * 100;
                  const roomy = (field.bytes / widest) * 100 >= 15;
                  return (
                    <span
                      key={field.name}
                      className="layout-field"
                      style={{ flex: `0 0 ${share}%` }}
                      title={`${field.name}: ${field.bytes} B${field.note ? ` — ${field.note}` : ""}`}
                    >
                      {roomy && <span className="layout-field-name">{field.name}</span>}
                    </span>
                  );
                })}
              </motion.div>
            </div>
            <span className="layout-total">{totals[index]} B</span>
            <p className="layout-legend">
              {row.fields.map((field) => (
                <span key={field.name}>
                  <span className="layout-legend-name">{field.name}</span> {field.bytes} B
                  {field.note && <span className="muted"> ({field.note})</span>}
                </span>
              ))}
            </p>
          </div>
        ))}
      </div>
      {totals.length === 2 && totals[1] > 0 && totals[0] !== totals[1] && (
        <p className="explainer-verdict">
          {explainer.rows[1].label} is{" "}
          <strong>
            {totals[0] > totals[1]
              ? `${(totals[0] / totals[1]).toFixed(1)}× smaller`
              : `${(totals[1] / totals[0]).toFixed(1)}× larger`}
          </strong>{" "}
          than {explainer.rows[0].label.toLowerCase()}.
        </p>
      )}
      <p className="explainer-source muted">Sizes: {explainer.source}</p>
    </figure>
  );
}
