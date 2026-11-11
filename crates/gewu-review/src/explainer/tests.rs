use super::formula::Expr;
use super::{Explainer, LayoutField, LayoutRow, Param, Scale, Series, Unit};

fn eval(text: &str, names: &[&str], values: &[f64]) -> f64 {
    Expr::parse(text, names).expect("parses").eval(values)
}

#[test]
fn formulas_follow_arithmetic_precedence() {
    assert_eq!(eval("1 + 2 * 3", &[], &[]), 7.0);
    assert_eq!(eval("(1 + 2) * 3", &[], &[]), 9.0);
    assert_eq!(eval("2^3^2", &[], &[]), 512.0);
    assert_eq!(eval("-2^2", &[], &[]), -4.0);
    assert_eq!(eval("2^-1", &[], &[]), 0.5);
    assert_eq!(eval("n * 41", &["n"], &[1024.0]), 41_984.0);
    assert_eq!(
        eval("next_pow2(n * 8 / 7)", &["n"], &[524_288.0]),
        1_048_576.0
    );
    assert_eq!(eval("max(log2(n), ceil(1.5))", &["n"], &[8.0]), 3.0);
}

#[test]
fn formulas_name_only_declared_parameters_and_known_functions() {
    for (text, problem) in [
        ("n * m", "`m` is not a declared parameter"),
        ("exec(n)", "unknown function `exec`"),
        ("min(n)", "`min` takes 2 argument(s), got 1"),
        ("(n + 1", "never closed"),
        ("n +", "ends too early"),
        ("n $ 2", "unexpected `$`"),
    ] {
        let error = Expr::parse(text, &["n"]).expect_err(text).to_string();
        assert!(error.contains(problem), "{text}: {error}");
    }
}

fn model(formula: &str, min: i64) -> Explainer {
    Explainer::CostModel {
        title: "Index memory".into(),
        params: vec![Param {
            id: "b".into(),
            label: "Strings kept (B)".into(),
            min,
            max: 2_097_152,
            default: 524_288,
            scale: Scale::Log2,
            marks: vec![131_072, 524_288],
        }],
        series: vec![Series {
            label: "Before".into(),
            formula: formula.into(),
            unit: Unit::Bytes,
        }],
        note: None,
        source: "Word<2> is 32 bytes (compact_word_hash_and_ord test)".into(),
    }
}

#[test]
fn a_sound_cost_model_has_no_problems() {
    assert!(
        model("next_pow2(b * 8 / 7) * 41", 32_768)
            .problems()
            .is_empty()
    );
}

#[test]
fn a_cost_model_must_stay_finite_and_positive() {
    let problems = model("1 / (b - 32768)", 32_768).problems();
    assert!(
        problems[0].contains("at the parameters' minimums"),
        "{problems:?}"
    );
    let problems = model("b * 2", 0).problems();
    assert!(problems[0].contains("needs a positive min"), "{problems:?}");
}

#[test]
fn layouts_and_claims_are_checked() {
    let layout = Explainer::Layout {
        title: "One index entry".into(),
        rows: vec![LayoutRow {
            label: "Before".into(),
            fields: vec![LayoutField {
                name: "Word<C>".into(),
                bytes: 0,
                note: None,
            }],
        }],
        source: String::new(),
    };
    let problems = layout.problems();
    assert!(
        problems.iter().any(|p| p.contains("source of its sizes")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("between 1 and 4096 bytes")),
        "{problems:?}"
    );

    let claim = Explainer::Claim {
        claim: "use 61–65% less peak memory".into(),
        result: None,
        share: None,
        how: " ".into(),
    };
    assert_eq!(
        claim.problems(),
        ["a claim needs `how` this step serves it"]
    );

    let whole = Explainer::Claim {
        claim: "use 61–65% less peak memory".into(),
        result: Some("940 → 367 MiB (−61%)".into()),
        share: None,
        how: "The index shrinks.".into(),
    };
    assert!(
        whole.problems()[0].contains("needs `share`"),
        "{:?}",
        whole.problems()
    );
}

#[test]
fn explainers_read_as_tagged_blocks() {
    let block: crate::ChunkBlock = serde_json::from_str(
        r#"{"type":"explainer","kind":"claim","claim":"18–32% faster","how":"Fewer clones."}"#,
    )
    .expect("block");
    assert!(matches!(
        block,
        crate::ChunkBlock::Explainer(Explainer::Claim { .. })
    ));
}

#[test]
fn explainers_read_as_markdown_where_they_cannot_be_drawn() {
    let text = model("next_pow2(b * 8 / 7) * 41", 32_768).markdown();
    assert!(text.contains("Strings kept (B) = 524288"), "{text}");
    assert!(text.contains("Before: 41.0 MiB"), "{text}");
    assert_eq!(super::format_value(1_234_567.0, Unit::Count), "1,234,567");
}

#[test]
fn claims_must_quote_the_authors_description() {
    let chunk = |claim: &str| crate::ReviewChunk {
        id: "c".into(),
        title: "t".into(),
        summary: "s".into(),
        kind: crate::ChunkKind::Concept,
        content: vec![crate::ChunkBlock::Explainer(Explainer::Claim {
            claim: claim.into(),
            result: None,
            share: None,
            how: "h".into(),
        })],
        context: None,
        confidence: None,
        oversized_reason: None,
        section: None,
    };
    let description =
        "With the default streaming proposer, the measured ladder\ncases run 18–32% faster.";
    assert!(
        super::unquoted_claims(
            [&chunk("the measured ladder cases run 18–32% faster")],
            Some(description)
        )
        .is_empty()
    );
    let invented = super::unquoted_claims([&chunk("runs 50% faster")], Some(description));
    assert!(
        invented[0].contains("not in the author's description"),
        "{invented:?}"
    );
    let none = super::unquoted_claims([&chunk("anything")], None);
    assert!(none[0].contains("no author description"), "{none:?}");
}
