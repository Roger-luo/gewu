use crate::{
    ChangePresentation, ChunkBlock, ChunkKind, Overview, Quote, ReviewChunk, ReviewInput,
    ReviewSpec,
};

impl ReviewSpec {
    /// One raw-diff chunk per change, in file order: a review a person can
    /// work through when no agent writes a walkthrough.
    #[must_use]
    pub fn fallback(input: &ReviewInput) -> Self {
        let chunks = input
            .changes
            .iter()
            .enumerate()
            .map(|(index, change)| ReviewChunk {
                id: format!("fallback-{}", index + 1),
                title: format!("Review {}", change.path),
                summary: format!(
                    "Read the whole diff of {} and judge it on its own. No agent explained \
                     this change; choose an agent when you start a review to get a guided \
                     walkthrough.",
                    change.path
                ),
                kind: ChunkKind::Other,
                content: vec![ChunkBlock::Changes {
                    quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
                    presentation: ChangePresentation::Diff,
                }],
                context: None,
                confidence: None,
                oversized_reason: None,
                section: None,
            })
            .collect();
        Self::new(
            input.source.clone(),
            Overview {
                intent: "Review the selected Git changes.".into(),
                changes: vec!["Each changed file is one step, in file order.".into()],
                illustration: None,
                concepts: vec![],
            },
            chunks,
        )
    }
}
