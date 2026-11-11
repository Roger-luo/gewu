use std::collections::{BTreeMap, HashMap};

use crate::{ChunkBlock, ChunkKind, ReadingTime, ReviewChunk};

use super::{ChunkLoad, MAX_STATE_CHARS, Rubric, WalkthroughLoad, chunk_state};

fn rubric() -> &'static Rubric {
    Rubric::builtin()
}

fn read(seconds: f64) -> ReadingTime {
    ReadingTime {
        prose: seconds,
        ..ReadingTime::default()
    }
}

fn answers(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(name, p)| ((*name).to_owned(), *p))
        .collect()
}

fn score(seconds: f64, answers: &BTreeMap<String, f64>) -> ChunkLoad {
    ChunkLoad::score("c", read(seconds), rubric(), &rubric().questions, answers)
}

fn chunk_with(overloaded: f64) -> ChunkLoad {
    ChunkLoad {
        overloaded,
        ..score(10.0, &BTreeMap::new())
    }
}

fn walk(chunks: Vec<ChunkLoad>) -> WalkthroughLoad {
    WalkthroughLoad::of(chunks, &rubric().equation)
}

/// The release's rubric must parse, or every score would panic.
#[test]
fn the_builtin_rubric_is_valid() {
    let rubric = rubric();
    assert!(rubric.version >= 3);
    assert!(!rubric.questions.is_empty());
    assert!(!rubric.guidance.recalibrate.trim().is_empty());
}

#[test]
fn a_rubric_with_a_repeated_question_or_bad_weight_is_refused() {
    let text = include_str!("../../rubrics/load.toml");
    let repeated = format!(
        "{text}\n[[question]]\nname = \"padded\"\ninstructions = \"x\"\nwhen_true = \"x\"\n\
         when_false = \"x\"\nweight = 0.1\ntests = true\n"
    );
    assert!(Rubric::parse(&repeated).is_err());
    let negative = text.replacen("weight = 0.6", "weight = -0.6", 1);
    assert!(Rubric::parse(&negative).is_err());
    let unknown = text.replacen("[budget]", "[budget]\nrows = 40", 1);
    assert!(Rubric::parse(&unknown).is_err(), "unknown keys are typos");
}

#[test]
fn a_short_unanswered_chunk_is_not_overloaded_and_says_why() {
    let load = score(60.0, &BTreeMap::new());
    assert_eq!(load.overloaded, 0.0);
    assert_eq!(load.unanswered, rubric().questions.len());
}

/// Reading time alone crosses θ at `1 + θ/w_t` budgets' worth.
#[test]
fn reading_time_alone_can_overload_a_chunk() {
    let equation = rubric().equation;
    let budget = f64::from(rubric().budget.chunk_seconds);
    let load = score(
        budget * (1.0 + equation.threshold / equation.time_weight),
        &BTreeMap::new(),
    );
    assert!((load.time_load - equation.threshold).abs() < 1e-9);
    assert_eq!(load.overloaded, 1.0);
}

/// Two coin flips, each below θ alone and above it together: exactly 1/4.
#[test]
fn the_probability_is_exact_over_independent_signs() {
    let load = score(
        10.0,
        &answers(&[("distant_recall", 0.5), ("several_points", 0.5)]),
    );
    assert!(
        (load.overloaded - 0.25).abs() < 1e-12,
        "{}",
        load.overloaded
    );
    assert!((load.expected - 0.55).abs() < 1e-12);
    assert_eq!(load.drivers[0].0, "distant_recall");
}

#[test]
fn malformed_answers_are_ignored() {
    let load = score(
        10.0,
        &answers(&[
            ("distant_recall", 1.5),
            ("several_points", f64::NAN),
            ("made_up", 1.0),
        ]),
    );
    assert!(load.answers.is_empty());
    assert_eq!(load.overloaded, 0.0);
}

#[test]
fn a_walkthrough_is_high_when_more_than_a_quarter_is_overloaded() {
    let fine = walk(vec![
        chunk_with(1.0),
        chunk_with(0.0),
        chunk_with(0.0),
        chunk_with(0.0),
    ]);
    assert_eq!(fine.allowed, 1);
    assert!(!fine.high());
    let heavy = walk(vec![
        chunk_with(1.0),
        chunk_with(1.0),
        chunk_with(0.0),
        chunk_with(0.0),
    ]);
    assert!(heavy.high());
    assert!((heavy.distribution.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    assert!((heavy.expected_overloaded() - 2.0).abs() < 1e-12);
}

/// Four chunks at even odds: P(more than one) = 1 − (1 + 4)/16.
#[test]
fn the_walkthrough_tail_is_poisson_binomial() {
    let walk = walk(vec![chunk_with(0.5); 4]);
    assert!((walk.excess - 11.0 / 16.0).abs() < 1e-12, "{}", walk.excess);
}

fn titled(title: &str, kind: ChunkKind, content: Vec<ChunkBlock>) -> ReviewChunk {
    ReviewChunk {
        id: title.into(),
        title: title.into(),
        summary: "x".repeat(200),
        kind,
        content,
        context: None,
        confidence: None,
        oversized_reason: None,
        section: None,
    }
}

#[test]
fn the_state_shows_prose_and_is_cut_when_long() {
    let long = vec![ChunkBlock::Narrative {
        markdown: "word ".repeat(10_000),
    }];
    let state = chunk_state(
        &titled("Title", ChunkKind::Other, long),
        &HashMap::new(),
        &[],
    );
    assert!(state.starts_with("# This chunk: Title\n\nxxx"));
    assert!(state.len() < MAX_STATE_CHARS + 100);
    assert!(state.ends_with("[cut: the chunk continues]\n"));
}

/// The decider can only judge what the reader already has if it sees it; the
/// nearest earlier chunks are kept when they do not all fit.
#[test]
fn the_state_lists_the_nearest_earlier_chunks() {
    let earlier: Vec<_> = (0..100)
        .map(|index| titled(&format!("Earlier {index}"), ChunkKind::Other, Vec::new()))
        .collect();
    let now = titled("Now", ChunkKind::Other, Vec::new());
    let state = chunk_state(&now, &HashMap::new(), &earlier);
    assert!(state.contains("- Earlier 99: "));
    assert!(!state.contains("- Earlier 0: "));
    assert!(state.contains("earlier chunks not listed"));
    assert!(state.find("Earlier 99") < state.find("# This chunk: Now"));
}

/// A test section Gewu writes has no prose, so it is not asked about prose.
#[test]
fn test_chunks_skip_the_questions_about_prose() {
    let asked = rubric().questions_for(&titled("t", ChunkKind::Test, Vec::new()));
    assert!(asked.iter().all(|question| question.tests));
    assert!(!asked.iter().any(|question| question.name == "padded"));
    let all = rubric().questions_for(&titled("c", ChunkKind::Other, Vec::new()));
    assert_eq!(all.len(), rubric().questions.len());
}
