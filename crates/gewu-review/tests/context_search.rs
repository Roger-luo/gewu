use gewu_review::{Change, ReviewInput, SourceIdentity, context::ContextReader};

#[test]
fn literal_search_pages_exact_global_coordinates_without_source_copies() {
    let input = ReviewInput::new(
        SourceIdentity::new("base", "head", b"search"),
        vec![
            Change::new(
                "a.rs",
                (0, 0, 1, 80),
                format!("@@ header\n{}", "+λ_target();\n".repeat(80)),
            ),
            Change::new("b.rs", (0, 0, 1, 1), "+other();\n"),
        ],
    );
    let reader = ContextReader::new(&input, 1024);
    let mut cursor = None;
    let mut found = vec![];
    loop {
        let page = reader.search("λ_target", cursor.as_ref()).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() <= 1024);
        found.extend(page.matches.iter().map(|hit| hit.quote.start));
        assert!(
            page.matches
                .iter()
                .all(|hit| hit.path == "a.rs" && hit.quote.start == hit.quote.end)
        );
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(found, (1..=80).collect::<Vec<_>>());
    let page = reader.search("λ_target", None).unwrap();
    assert!(reader.search("other", page.next.as_ref()).is_err());
    let mut stale = page.next.unwrap();
    stale.source.head = "changed".into();
    assert!(reader.search("λ_target", Some(&stale)).is_err());
    assert!(reader.search("", None).is_err());
    assert!(reader.search(&"x".repeat(257), None).is_err());
    assert!(reader.search("missing", None).unwrap().matches.is_empty());
    assert!(
        ContextReader::new(&input, 10)
            .search("λ_target", None)
            .is_err()
    );
}
