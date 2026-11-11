//! Explainers as Markdown, for the terminal UI and anywhere that cannot draw.

use super::{Explainer, Unit, formula::Expr};

impl Explainer {
    /// The explainer told in Markdown: a quoted claim, the byte totals of a
    /// layout, or a cost model's values at its default parameters.
    #[must_use]
    pub fn markdown(&self) -> String {
        match self {
            Self::Claim {
                claim,
                result,
                share,
                how,
            } => {
                let mut text = format!("> **Claim:** {claim}");
                if let Some(result) = result {
                    text.push_str(&format!("\n>\n> **Measured:** {result}"));
                }
                if let Some(share) = share {
                    text.push_str(&format!("\n>\n> **This step:** {share}"));
                }
                format!("{text}\n\n{how}")
            }
            Self::Layout {
                title,
                rows,
                source,
            } => {
                let mut text = format!("**{title}**\n");
                for row in rows {
                    let parts: Vec<String> = row
                        .fields
                        .iter()
                        .map(|field| format!("{} ({} B)", field.name, field.bytes))
                        .collect();
                    let total: u32 = row.fields.iter().map(|field| field.bytes).sum();
                    text.push_str(&format!(
                        "\n- {}: {} = {total} B",
                        row.label,
                        parts.join(" + ")
                    ));
                }
                format!("{text}\n\n*Sizes: {source}*")
            }
            Self::CostModel {
                title,
                params,
                series,
                note,
                source,
            } => {
                let names: Vec<&str> = params.iter().map(|param| param.id.as_str()).collect();
                #[allow(clippy::cast_precision_loss)]
                let values: Vec<f64> = params.iter().map(|param| param.default as f64).collect();
                let at: Vec<String> = params
                    .iter()
                    .map(|param| format!("{} = {}", param.label, param.default))
                    .collect();
                let mut text = format!("**{title}**, at {}:\n", at.join(", "));
                for series in series {
                    let value = Expr::parse(&series.formula, &names)
                        .map_or(f64::NAN, |expr| expr.eval(&values));
                    text.push_str(&format!(
                        "\n- {}: {} (`{}`)",
                        series.label,
                        format_value(value, series.unit),
                        series.formula
                    ));
                }
                if let Some(note) = note {
                    text.push_str(&format!("\n\n{note}"));
                }
                format!("{text}\n\n*Source: {source}*")
            }
        }
    }
}

/// A value in its unit: bytes in KiB, MiB, or GiB; counts with separators.
#[must_use]
pub fn format_value(value: f64, unit: Unit) -> String {
    if !value.is_finite() {
        return "not computable".into();
    }
    match unit {
        Unit::Bytes => {
            let mut amount = value;
            for suffix in ["B", "KiB", "MiB", "GiB"] {
                if amount < 1024.0 || suffix == "GiB" {
                    return if suffix == "B" {
                        format!("{amount:.0} B")
                    } else {
                        format!("{amount:.1} {suffix}")
                    };
                }
                amount /= 1024.0;
            }
            unreachable!("the loop returns at GiB")
        }
        Unit::Seconds => format!("{value:.2} s"),
        Unit::Count => {
            let whole = format!("{:.0}", value);
            let mut grouped = String::new();
            for (index, digit) in whole.chars().rev().enumerate() {
                if index > 0 && index % 3 == 0 {
                    grouped.push(',');
                }
                grouped.push(digit);
            }
            grouped.chars().rev().collect()
        }
    }
}
