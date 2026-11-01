//! Whether the code prose names exists, and whether the reader has it in view.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Serialize;

use super::{ChunkProse, InvisibleLine, SymbolIndex, prose, prose::Reference};
use crate::{ReviewChunk, ValidatedReview};

/// Keywords whose next identifier is being defined.
const DEFINING: [&str; 11] = [
    "fn", "struct", "enum", "trait", "type", "const", "static", "mod", "union", "def", "class",
];

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceReport {
    pub unresolved: Vec<UnresolvedReference>,
    /// Names outside any repository, such as a tool's setting; reported.
    pub external: Vec<UnresolvedReference>,
    pub forward: Vec<ForwardReference>,
    pub distant: Vec<DistantReference>,
    /// Cited line numbers no view of their chunk shows: reported for the
    /// judge, because an anchor to an earlier chunk is one too.
    pub invisible: Vec<InvisibleLine>,
}

/// A code span naming nothing in the corpus: a typo, a hallucination, or a
/// name the change removed and the prose forgot.
#[derive(Clone, Debug, Serialize)]
pub struct UnresolvedReference {
    /// A chunk id, or `overview`.
    pub location: String,
    pub reference: String,
}

/// Prose leaning on a definition the walkthrough has not shown yet.
#[derive(Clone, Debug, Serialize)]
pub struct ForwardReference {
    pub chunk_id: String,
    pub symbol: String,
    pub defined_in: String,
}

/// Prose leaning on a reviewed definition last shown many chunks ago, with
/// nothing in this chunk or its context pane to re-anchor it.
#[derive(Clone, Debug, Serialize)]
pub struct DistantReference {
    pub chunk_id: String,
    pub symbol: String,
    pub last_shown: String,
    pub distance: usize,
}

impl ReferenceReport {
    pub(super) fn new(review: &ValidatedReview, symbols: &SymbolIndex, threshold: usize) -> Self {
        let chunks = &review.spec().chunks;
        let shown: Vec<_> = chunks.iter().map(|c| Shown::new(review, c)).collect();
        let mut defined: HashMap<&str, usize> = HashMap::new();
        for (index, view) in shown.iter().enumerate() {
            for name in &view.defines {
                defined.entry(name).or_insert(index);
            }
        }
        let overview = &review.spec().overview;
        let overview_text: Vec<&str> = std::iter::once(overview.intent.as_str())
            .chain(overview.changes.iter().map(String::as_str))
            .chain(overview.concepts.iter().map(String::as_str))
            .collect();
        let (unresolved, external) =
            Self::unresolved("overview", &overview_text.join("\n"), symbols);
        let mut report = Self {
            unresolved,
            external,
            forward: Vec::new(),
            distant: Vec::new(),
            invisible: super::citations::check(review),
        };
        for (index, chunk) in chunks.iter().enumerate() {
            let text = ChunkProse::new(chunk).text();
            let (unresolved, external) = Self::unresolved(&chunk.id, &text, symbols);
            report.unresolved.extend(unresolved);
            report.external.extend(external);
            let context: HashSet<_> = ChunkProse::context(chunk)
                .into_iter()
                .flat_map(prose::identifiers)
                .collect();
            let names: BTreeSet<_> = prose::code_spans(&text)
                .into_iter()
                .filter_map(Reference::parse)
                .filter_map(|reference| reference.name())
                .collect();
            for name in names {
                let Some(&definition) = defined.get(name) else {
                    continue;
                };
                if definition > index {
                    report.forward.push(ForwardReference {
                        chunk_id: chunk.id.clone(),
                        symbol: name.to_owned(),
                        defined_in: chunks[definition].id.clone(),
                    });
                    continue;
                }
                if shown[index].words.contains(name) || context.contains(name) {
                    continue;
                }
                let last = (0..index).rev().find(|j| shown[*j].words.contains(name));
                if let Some(last) = last.filter(|last| index - last >= threshold) {
                    report.distant.push(DistantReference {
                        chunk_id: chunk.id.clone(),
                        symbol: name.to_owned(),
                        last_shown: chunks[last].id.clone(),
                        distance: index - last,
                    });
                }
            }
        }
        report
    }

    #[must_use]
    pub fn passes(&self) -> bool {
        self.unresolved.is_empty()
    }

    /// Unresolved names of the codebase, and names that point outside it.
    /// An absolute path (`/dev/null`) or a dotted name with no `::`
    /// (`core.quotePath`, a tool's setting) cannot be found in any
    /// repository, so it is reported for the judge instead of failing.
    fn unresolved(
        location: &str,
        text: &str,
        symbols: &SymbolIndex,
    ) -> (Vec<UnresolvedReference>, Vec<UnresolvedReference>) {
        let spans: BTreeSet<_> = prose::code_spans(text).into_iter().collect();
        let (mut unresolved, mut external) = (Vec::new(), Vec::new());
        for span in spans {
            let missing = match Reference::parse(span) {
                None => false,
                Some(Reference::Path(path)) => !symbols.has_path(path),
                Some(Reference::Symbol(segments)) => {
                    !segments.iter().all(|segment| symbols.has_word(segment))
                }
            };
            if !missing {
                continue;
            }
            let outside = span.starts_with('/')
                || (span.contains('.') && !span.contains("::") && !span.contains('/'));
            let reference = UnresolvedReference {
                location: location.to_owned(),
                reference: span.to_owned(),
            };
            if outside {
                external.push(reference)
            } else {
                unresolved.push(reference)
            }
        }
        (unresolved, external)
    }
}

/// What one chunk puts in front of the reader.
struct Shown<'a> {
    words: HashSet<&'a str>,
    /// Names defined on added or unchanged lines; a removed definition is no
    /// longer something the reader can look up.
    defines: Vec<&'a str>,
}

impl<'a> Shown<'a> {
    fn new(review: &'a ValidatedReview, chunk: &ReviewChunk) -> Self {
        let mut shown = Self {
            words: HashSet::new(),
            defines: Vec::new(),
        };
        for quote in chunk.quotes() {
            let Some(change) = review
                .input()
                .changes
                .iter()
                .find(|change| change.id == quote.change_id)
            else {
                continue;
            };
            for text in change.body(quote.start, quote.end) {
                let code = text.get(1..).unwrap_or_default();
                shown.words.extend(prose::identifiers(code));
                if text.starts_with('-') {
                    continue;
                }
                // Whitespace-separated, so `"type": "x"` in JSON defines nothing.
                let tokens: Vec<_> = code.split_whitespace().collect();
                shown.defines.extend(
                    tokens
                        .windows(2)
                        .filter(|pair| DEFINING.contains(&pair[0]))
                        .filter_map(|pair| prose::identifiers(pair[1]).next()),
                );
            }
        }
        shown
    }
}
