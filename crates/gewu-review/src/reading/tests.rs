use std::collections::HashMap;

use crate::{
    Change, ChangePresentation, ChunkBlock, ChunkKind, Explainer, Quote, ReviewChunk,
    explainer::Step,
};

use super::ReadingTime;

fn change(patch: &str) -> Change {
    let lines = patch.lines().count() as u32;
    Change::new(
        "a.rs",
        (1, lines, 1, lines),
        format!("@@ -1 +1 @@\n{patch}"),
    )
}

fn chunk(content: Vec<ChunkBlock>) -> ReviewChunk {
    ReviewChunk {
        id: "chunk".into(),
        title: "Title".into(),
        summary: "Summary.".into(),
        kind: ChunkKind::Other,
        content,
        context: None,
        confidence: None,
        oversized_reason: None,
        section: None,
    }
}

fn quoted(change: &Change) -> ChunkBlock {
    ChunkBlock::Changes {
        quotes: vec![Quote::whole(change.id.clone(), change.body_lines())],
        presentation: ChangePresentation::Diff,
    }
}

fn code_time(patch: &str) -> f64 {
    let change = change(patch);
    let chunk = chunk(vec![quoted(&change)]);
    ReadingTime::of(&chunk, &HashMap::from([(&change.id, &change)])).code
}

#[test]
fn changed_logic_costs_more_than_context_imports_and_blanks() {
    let logic = code_time("+    let total = left + right;\n");
    assert!(logic > code_time("     let total = left + right;\n"));
    assert!(logic > code_time("+use std::collections::HashMap;\n"));
    assert!(logic > code_time("+\n"));
}

#[test]
fn a_branch_costs_more_than_a_plain_call() {
    assert!(code_time("+    if ready { go(); }\n") > code_time("+    go(left, right);\n"));
}

/// Twenty near-identical lines are read as one shape, not twenty lines.
#[test]
fn near_repeats_cost_about_once() {
    let one = code_time("+    table.push(entry_0, 0);\n");
    let many: String = (0..20)
        .map(|index| format!("+    table.push(entry_{index}, {index});\n"))
        .collect();
    assert!(code_time(&many) < one * 5.0, "{}", code_time(&many));
}

#[test]
fn a_comment_costs_its_words() {
    let short = code_time("+    // Keep it.\n");
    let long = code_time(&format!("+    // {}\n", "word ".repeat(40)));
    assert!(long > short * 5.0);
}

#[test]
fn prose_is_priced_at_reading_speed() {
    let text = "word ".repeat(230);
    let chunk = chunk(vec![ChunkBlock::Narrative { markdown: text }]);
    let time = ReadingTime::of(&chunk, &HashMap::new());
    // 230 words at 230 a minute, plus the title and summary.
    assert!((60.0..62.0).contains(&time.prose), "{}", time.prose);
}

/// A stepper costs its own steps but discounts the quoted lines it walks.
#[test]
fn a_stepper_over_quoted_code_discounts_it() {
    let change = change("+let a = f(x)?;\n+let b = g(a)?;\n");
    let step = |code: Option<&str>, highlight: &str| Step {
        caption: "Runs this line.".into(),
        code: code.map(str::to_owned),
        highlight: Some(highlight.into()),
        state: Vec::new(),
    };
    let stepper = Explainer::Stepper {
        title: "Walk".into(),
        language: None,
        source: None,
        start_line: None,
        steps: vec![
            step(Some("let a = f(x)?;\nlet b = g(a)?;"), "1"),
            step(None, "2"),
        ],
    };
    let chunk = chunk(vec![quoted(&change), ChunkBlock::Explainer(stepper)]);
    let time = ReadingTime::of(&chunk, &HashMap::from([(&change.id, &change)]));
    assert!(time.explainers > 0.0);
    assert!(time.discount > 0.0 && time.discount < time.code);
    assert!(time.to_string().contains("the steppers save"), "{time}");
}

#[test]
fn display_names_minutes() {
    let time = ReadingTime {
        prose: 50.0,
        code: 75.0,
        ..ReadingTime::default()
    };
    assert_eq!(
        time.to_string(),
        "about 2 min 05 s to read (prose 50 s, code 1 min 15 s)"
    );
}
