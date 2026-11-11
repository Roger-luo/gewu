//! A claim's `share` names this step's part of a measured result: an amount,
//! not a description of what the step enables, and not the whole result again.

use super::Explainer;

/// The numbers in `text`, as written: "18–32%" gives "18" and "32".
fn numbers(text: &str) -> Vec<&str> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|part| part.trim_matches('.'))
        .filter(|part| !part.is_empty())
        .collect()
}

/// Shares in `chunks` that give no amount, or only repeat the result's own
/// numbers, as errors an agent can act on.
///
/// Found by QA of Cursor's runs of #276: shares such as "Enables swapping"
/// and "part of the 18–32% speedups" credited a step without saying how much.
#[must_use]
pub fn vague_shares<'a>(chunks: impl IntoIterator<Item = &'a crate::ReviewChunk>) -> Vec<String> {
    let mut errors = Vec::new();
    for chunk in chunks {
        for block in &chunk.content {
            let crate::ChunkBlock::Explainer(Explainer::Claim {
                result,
                share: Some(share),
                ..
            }) = block
            else {
                continue;
            };
            if share.to_lowercase().contains("all of") {
                continue;
            }
            let amounts = numbers(share);
            let lower = share.trim_start().to_lowercase();
            // Found by QA of Cursor's run E of #276: "enables the 32-byte
            // `Word` layout (24 MiB saved …)" credited a step with what it
            // made possible and another section delivered.
            if lower.starts_with("enables") || lower.starts_with("enabling") {
                errors.push(format!(
                    "chunk `{}` has a share that says what the chunk enables: \"{share}\"; a \
                     share is the part of the result this chunk delivers itself, so leave \
                     `share` and `result` out and say what it enables in `how`",
                    chunk.id
                ));
            } else if amounts.is_empty() {
                errors.push(format!(
                    "chunk `{}` has a share with no amount: \"{share}\"; say how much of the \
                     result this chunk accounts for (an estimate is fine), or leave `share` and \
                     `result` out when it only enables the result",
                    chunk.id
                ));
            } else if let Some(result) = result
                && amounts
                    .iter()
                    .all(|amount| numbers(result).contains(amount))
            {
                errors.push(format!(
                    "chunk `{}` has a share that repeats the whole result: \"{share}\"; give this \
                     chunk's part of it, or write \"all of it\" and why if it accounts for the \
                     whole",
                    chunk.id
                ));
            }
        }
    }
    errors
}
