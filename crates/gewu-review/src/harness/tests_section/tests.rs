use super::test_expansion;
use crate::{Change, ChunkBlock, ChunkKind, Quote, ReviewInput, SourceIdentity, harness::WorkItem};

fn input(code: &[String]) -> ReviewInput {
    let patch = format!(
        "@@ -0,0 +1,{} @@\n{}\n",
        code.len(),
        code.iter()
            .map(|line| format!("+{line}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    ReviewInput::new(
        SourceIdentity::new("base", "head", b"diff"),
        vec![Change::new(
            "src/tests.rs",
            (0, 0, 1, u32::try_from(code.len()).expect("lines")),
            patch,
        )],
    )
}

fn item(input: &ReviewInput) -> WorkItem {
    let change = &input.changes[0];
    WorkItem {
        id: "tests".into(),
        title: "Tests".into(),
        purpose: "Tests.".into(),
        covers: vec![Quote::whole(change.id.clone(), change.body_lines())],
        expanded: false,
        chunks: Vec::new(),
        tests: true,
    }
}

fn test(name: &str, doc: &str, body: usize) -> Vec<String> {
    let mut lines = vec![
        format!("/// {doc}"),
        "#[test]".to_owned(),
        format!("fn {name}() {{"),
    ];
    lines.extend((0..body).map(|index| format!("    assert_eq!({index}, {index});")));
    lines.push("}".to_owned());
    lines
}

/// Asked for after timing Cursor on #276: test sections cost a third of the
/// agent's time, and Gewu can write them from the code.
#[test]
fn tests_become_named_chunks_with_their_doc_comments() {
    let mut code = vec!["use super::*;".to_owned(), String::new()];
    code.extend(test(
        "grows_by_a_quarter",
        "Grows by a quarter when full.",
        3,
    ));
    code.push(String::new());
    code.extend(test(
        "keeps_the_tail",
        "Keeps the tail after a rollback.",
        3,
    ));
    code.push(String::new());
    code.extend(test("rejects_overflow", "Rejects a cap that overflows.", 3));
    let input = input(&code);

    let expansion = test_expansion(&input, &item(&input), 6).expect("written");

    let chunk = &expansion.chunks[0];
    assert_eq!(chunk.kind, ChunkKind::Test);
    assert!(
        chunk.title.contains("grows_by_a_quarter"),
        "{}",
        chunk.title
    );
    let ChunkBlock::Narrative { markdown } = &chunk.content[0] else {
        panic!("prose first");
    };
    assert!(markdown.contains("without an agent"), "{markdown}");
    assert!(
        markdown.contains("Keeps the tail after a rollback."),
        "{markdown}"
    );
}

#[test]
fn a_test_too_long_for_one_chunk_is_shown_in_parts() {
    let input = input(&test("long", "A long test.", 70));

    let expansion = test_expansion(&input, &item(&input), 6).expect("written");

    assert!(expansion.chunks.len() >= 2, "{:?}", expansion.chunks.len());
}

/// Found by running Cursor on #276: test items of 200 to 370 lines, which
/// the plan accepts, did not fit six chunks of the row budget and went to the
/// agent, which spent 25 minutes on one. Whole tests are shown instead, in
/// at most six views that scroll.
#[test]
fn an_item_too_large_for_fitting_chunks_shows_whole_tests_that_scroll() {
    let code: Vec<String> = (0..12)
        .flat_map(|index| test(&format!("case_{index}"), "One case.", 14))
        .collect();
    let input = input(&code);

    let expansion = test_expansion(&input, &item(&input), 6).expect("written");

    assert!(expansion.chunks.len() <= 6, "{}", expansion.chunks.len());
    assert!(
        expansion
            .chunks
            .iter()
            .any(|chunk| chunk.oversized_reason.is_some())
    );
    assert!(
        expansion.chunks[0].title.contains("case_0"),
        "{}",
        expansion.chunks[0].title
    );
}
