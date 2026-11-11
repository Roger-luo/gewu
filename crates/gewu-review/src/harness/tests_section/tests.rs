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

/// Found by QA of the #276 walkthrough: a long test cut mid-loop left two
/// chunks with one title. It is shown whole, in one view that scrolls.
#[test]
fn a_test_too_long_for_one_chunk_is_shown_whole() {
    let input = input(&test("long", "A long test.", 70));

    let expansion = test_expansion(&input, &item(&input), 6).expect("written");

    assert_eq!(expansion.chunks.len(), 1);
    assert!(expansion.chunks[0].oversized_reason.is_some());
    assert_eq!(expansion.chunks[0].title, "Test `long`");
}

/// Found by QA of the #276 walkthrough: helpers were titled as tests, and
/// edits inside existing tests were counted as tests "shown as written".
#[test]
fn helpers_and_edits_are_not_called_tests() {
    let code = vec![
        "fn propose(width: usize) -> Vec<u8> {".to_owned(),
        "    vec![0; width]".to_owned(),
        "}".to_owned(),
    ];
    let helper = input(&code);
    let expansion = test_expansion(&helper, &item(&helper), 6).expect("written");
    assert_eq!(expansion.chunks[0].title, "Test helpers `propose`");

    let patch = "@@ -1,3 +1,3 @@\n fn existing() {\n-    old();\n+    new();\n }\n";
    let edit = ReviewInput::new(
        SourceIdentity::new("base", "head", b"diff"),
        vec![Change::new("src/tests.rs", (1, 3, 1, 3), patch)],
    );
    let expansion = test_expansion(&edit, &item(&edit), 6).expect("written");
    assert_eq!(
        expansion.chunks[0].title,
        "Changes to existing tests in `src/tests.rs`"
    );
    assert!(
        expansion.chunks[0]
            .summary
            .contains("as the diff changes them")
    );
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
