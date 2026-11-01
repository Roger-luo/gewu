use gewu_review::{Change, Quote, ReviewInput, SourceIdentity, context::ContextReader};

fn fixture() -> ReviewInput {
    ReviewInput::new(
        SourceIdentity::new("empty", "head", b"fixture"),
        vec![
            Change::new(
                "unicode.rs",
                (1, 1, 1, 2),
                "@@ -1 +1,2 @@\n-old\n+λ🙂\n+end\n",
            ),
            Change::new(
                "asset.svg",
                (0, 0, 1, 1),
                format!("@@ -0,0 +1 @@\n+{}\n", "🙂\\\"".repeat(4000)),
            ),
            Change::new("image.png", (0, 0, 0, 0), "Binary files differ\n"),
            Change::new(
                "escaped.txt",
                (0, 0, 1, 1),
                format!("@@ -0,0 +1 @@\n+{}\n", "\u{001f}".repeat(4000)),
            ),
        ],
    )
}

#[test]
fn partial_spans_and_duplicate_hunks_keep_global_coordinates() {
    let change = Change::new("same.rs", (1, 2, 1, 2), "@@ -1,2 +1,2 @@\n-a\n+b\n c\n");
    let input = ReviewInput::new(
        SourceIdentity::new("a", "b", b"duplicates"),
        vec![change.clone(), change],
    );
    assert_ne!(input.changes[0].id, input.changes[1].id);
    let quote = Quote {
        change_id: input.changes[1].id.clone(),
        start: 2,
        end: 3,
    };
    let page = ContextReader::new(&input, 1024)
        .source(&quote, None)
        .unwrap();
    assert_eq!(page.text, "+b\n c");
    assert_eq!(page.first_body_line, 2);
    assert_eq!(page.first_line_byte, 0);
    assert_eq!(page.quote, quote);
    assert!(page.next.is_none());
}

#[test]
fn inventory_cursor_rejects_source_changes_and_out_of_bounds_positions() {
    let input = fixture();
    let reader = ContextReader::new(&input, 800);
    let mut cursor = reader.inventory(None).unwrap().next.unwrap();
    cursor.source.head = "stale".into();
    assert!(reader.inventory(Some(&cursor)).is_err());
    cursor.source = input.source.clone();
    cursor.index = input.changes.len();
    assert!(reader.inventory(Some(&cursor)).is_err());
}

#[test]
fn empty_corpus_is_a_bounded_terminal_inventory_page() {
    let input = ReviewInput::new(SourceIdentity::new("a", "a", b""), vec![]);
    let page = ContextReader::new(&input, 1024).inventory(None).unwrap();
    assert!(page.entries.is_empty());
    assert!(page.next.is_none());
}

#[test]
fn multi_megabyte_line_round_trips_in_a_reused_reader() {
    let patch = format!("+{}", "λ🙂".repeat(350_000));
    let change = Change::new("large.svg", (0, 0, 1, 1), &patch);
    let input = ReviewInput::new(
        SourceIdentity::new("a", "b", patch.as_bytes()),
        vec![change],
    );
    let reader = ContextReader::new(&input, 16_384);
    let quote = Quote::whole(&input.changes[0].id, 1);
    let mut cursor = None;
    let mut result = String::new();
    let mut pages = 0;
    let started = std::time::Instant::now();
    loop {
        let page = reader.source(&quote, cursor.as_ref()).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() <= 16_384);
        assert_eq!(page.first_body_line, 1);
        assert_eq!(page.first_line_byte, result.len());
        result.push_str(&page.text);
        pages += 1;
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(result, patch);
    println!(
        "{} bytes, {pages} pages, {:?}",
        result.len(),
        started.elapsed()
    );
}

#[test]
fn bounded_pages_reconstruct_every_original_span_without_reidentifying_changes() {
    let input = fixture();
    let reader = ContextReader::new(&input, 1024);
    let mut cursor = None;
    let mut ids = Vec::new();
    loop {
        let page = reader.inventory(cursor.as_ref()).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() <= 1024);
        ids.extend(page.entries.iter().map(|entry| entry.id.clone()));
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        ids,
        input
            .changes
            .iter()
            .map(|change| change.id.clone())
            .collect::<Vec<_>>()
    );
    for change in &input.changes {
        let quote = Quote::whole(&change.id, change.body_lines());
        let mut cursor = None;
        let mut reconstructed = String::new();
        loop {
            let page = reader.source(&quote, cursor.as_ref()).unwrap();
            assert!(serde_json::to_vec(&page).unwrap().len() <= 1024);
            assert_eq!(page.quote, quote);
            assert_eq!(page.source, input.source);
            assert_eq!(page.offset, reconstructed.len());
            reconstructed.push_str(&page.text);
            cursor = page.next;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(
            reconstructed,
            change.body(1, change.body_lines()).join("\n")
        );
    }
}

#[test]
fn stale_or_different_span_cursors_and_impossible_budgets_fail_explicitly() {
    let input = fixture();
    let reader = ContextReader::new(&input, 1024);
    let quote = Quote::whole(&input.changes[1].id, input.changes[1].body_lines());
    let cursor = reader.source(&quote, None).unwrap().next.unwrap();
    let wrong_span = Quote {
        start: 2,
        end: 2,
        ..quote.clone()
    };
    assert!(reader.source(&wrong_span, Some(&cursor)).is_err());
    let mut changed = input.clone();
    changed.source.head = "other".into();
    assert!(
        ContextReader::new(&changed, 1024)
            .source(&quote, Some(&cursor))
            .is_err()
    );
    assert!(ContextReader::new(&input, 1).source(&quote, None).is_err());
    assert!(ContextReader::new(&input, 1).inventory(None).is_err());
}
