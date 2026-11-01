use gewu_review::{
    Change, ReviewInput, SourceIdentity,
    research::{Evidence, Finding, ResearchNotes, ResearchPolicy, ResearchRun},
};

fn input() -> ReviewInput {
    ReviewInput::new(
        SourceIdentity::new("base", "head", b"research"),
        vec![
            Change::new("code.rs", (1, 1, 1, 1), "@@ -1 +1 @@\n-old\n+new\n"),
            Change::new(
                "large.svg",
                (0, 0, 1, 1),
                format!("+{}", "λ🙂".repeat(4000)),
            ),
        ],
    )
}

fn notes(task: &gewu_review::research::ResearchTask) -> ResearchNotes {
    ResearchNotes {
        schema_version: 1,
        source: task.page.source.clone(),
        request_id: task.id.clone(),
        findings: vec![Finding {
            summary: "The supplied source page contains these exact characters.".into(),
            evidence: vec![Evidence {
                offset: task.page.offset,
                text: task.page.text.chars().take(10).collect(),
            }],
            questions: vec!["A semantic quality review remains necessary.".into()],
        }],
    }
}

#[test]
fn batches_resume_and_export_only_after_every_source_page_was_accounted_for() {
    let input = input();
    let policy = ResearchPolicy {
        page_bytes: 1024,
        request_bytes: 4096,
        ..ResearchPolicy::default()
    };
    let mut ledger = ResearchRun::new(&input, policy).unwrap();
    assert!(ledger.findings().is_err());
    let mut count = 0;
    while let Some(task) = ledger.next().unwrap() {
        assert!(serde_json::to_vec(&task).unwrap().len() <= 4096);
        let response = notes(&task);
        ledger.accept(response.clone()).unwrap();
        assert!(
            ledger.accept(response).is_err(),
            "duplicate submission must not advance twice"
        );
        let saved = serde_json::from_slice(&serde_json::to_vec(ledger.ledger()).unwrap()).unwrap();
        ledger = ResearchRun::resume(&input, saved).unwrap();
        count += 1;
    }
    assert!(count > input.changes.len());
    let findings = ledger.findings().unwrap();
    assert_eq!(findings.len(), count);
    assert!(findings.iter().all(|finding| !finding.evidence.is_empty()));
    assert!(
        findings
            .iter()
            .all(|finding| !finding.evidence_spans.is_empty())
    );
    assert_eq!(findings[0].evidence_spans[0].start, 1);
    assert_eq!(findings[0].evidence_spans[0].end, 2);
    assert!(
        findings[1..]
            .iter()
            .all(|finding| finding.evidence_spans[0].start == 1
                && finding.evidence_spans[0].end == 1)
    );
    assert!(
        findings
            .iter()
            .all(|finding| finding.summary.contains("semantic quality review"))
    );
}

#[test]
fn resume_rejects_corrupted_or_reordered_records_and_changed_manifest_content() {
    let input = input();
    let mut run = ResearchRun::new(&input, ResearchPolicy::default()).unwrap();
    for _ in 0..2 {
        let task = run.next().unwrap().unwrap();
        run.accept(notes(&task)).unwrap();
    }
    let saved = serde_json::to_value(run.ledger()).unwrap();
    for variant in 0..3 {
        let mut bad = saved.clone();
        let records = bad["records"].as_array_mut().unwrap();
        match variant {
            0 => {
                records.remove(0);
            }
            1 => records.swap(0, 1),
            _ => {
                records[0]["findings"][0]["evidence"][0]["text"] = "fabricated".into();
            }
        }
        assert!(ResearchRun::resume(&input, serde_json::from_value(bad).unwrap()).is_err());
    }
    let mut changed = input.clone();
    changed.changes[0]
        .patch
        .push_str("+edited without updating source identity\n");
    assert!(ResearchRun::resume(&changed, run.ledger().clone()).is_err());
}

#[test]
fn paged_findings_and_supported_overview_are_bound_to_the_accepted_ledger() {
    use gewu_review::research::{GroundedOverview, SupportedText};
    let input = input();
    let mut run = ResearchRun::new(&input, ResearchPolicy::default()).unwrap();
    while let Some(task) = run.next().unwrap() {
        run.accept(notes(&task)).unwrap();
    }
    let mut cursor = None;
    let mut collected = Vec::new();
    let first = run.finding_page(None, 1024).unwrap();
    loop {
        let page = run.finding_page(cursor.as_ref(), 1024).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() <= 1024);
        assert_eq!(page.start_index, collected.len());
        collected.extend(page.findings);
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(collected, run.findings().unwrap());
    let claim = SupportedText {
        text: "Source-backed claim, to be judged independently.".into(),
        findings: vec![0],
    };
    let overview = GroundedOverview {
        schema_version: 1,
        source: input.source.clone(),
        ledger_sha256: first.ledger_sha256,
        intent: claim.clone(),
        changes: vec![claim],
        concepts: vec![],
    };
    run.assemble(&overview).unwrap().validate(&input).unwrap();
    let mut bad = overview.clone();
    bad.intent.findings = vec![usize::MAX];
    assert!(run.assemble(&bad).is_err());
    bad = overview;
    bad.ledger_sha256 = "stale".into();
    assert!(run.assemble(&bad).is_err());
    if let Some(mut stale) = first.next {
        stale.ledger_sha256 = "stale".into();
        assert!(run.finding_page(Some(&stale), 1024).is_err());
    } else {
        panic!("fixture should exercise finding pagination");
    }
}

#[test]
fn rejects_fabricated_stale_empty_and_oversized_notes_without_advancing() {
    let input = input();
    let mut ledger = ResearchRun::new(&input, ResearchPolicy::default()).unwrap();
    let task = ledger.next().unwrap().unwrap();
    let valid = notes(&task);
    let before = ledger.ledger().clone();
    for variant in 0..5 {
        let mut bad = valid.clone();
        match variant {
            0 => bad.findings[0].evidence[0].text = "fabricated".into(),
            1 => bad.source.head = "stale".into(),
            2 => bad.findings[0].summary = " ".into(),
            3 => bad.findings[0].evidence.clear(),
            _ => bad.findings[0].summary = "x".repeat(20_000),
        }
        assert!(ledger.accept(bad).is_err());
        assert_eq!(ledger.ledger(), &before);
    }
    ledger.accept(valid).unwrap();
    let mut changed = input.clone();
    changed.source.head = "changed".into();
    assert!(ResearchRun::resume(&changed, ledger.ledger().clone()).is_err());
}

#[test]
fn budget_failure_does_not_return_an_oversized_task_or_claim_completion() {
    let input = input();
    let policy = ResearchPolicy {
        request_bytes: 100,
        ..ResearchPolicy::default()
    };
    let ledger = ResearchRun::new(&input, policy).unwrap();
    assert!(ledger.next().is_err());
    let policy = ResearchPolicy {
        max_batches: 1,
        ..ResearchPolicy::default()
    };
    let mut ledger = ResearchRun::new(&input, policy).unwrap();
    let task = ledger.next().unwrap().unwrap();
    ledger.accept(notes(&task)).unwrap();
    assert!(ledger.next().is_err());
    assert!(ledger.findings().is_err());
}

#[test]
fn evidence_attribution_must_fit_a_finding_page_before_notes_are_accepted() {
    let input = ReviewInput::new(
        SourceIdentity::new("a", "b", b"long-path"),
        vec![Change::new("p".repeat(300), (0, 0, 1, 1), "+source")],
    );
    let policy = ResearchPolicy {
        request_bytes: 4096,
        page_bytes: 1024,
        ..ResearchPolicy::default()
    };
    let mut run = ResearchRun::new(&input, policy).unwrap();
    let task = run.next().unwrap().unwrap();
    let mut response = notes(&task);
    response.findings[0].evidence = vec![
        Evidence {
            offset: 0,
            text: "+".into()
        };
        20
    ];
    assert!(serde_json::to_vec(&response).unwrap().len() < 4096);
    let before = run.ledger().clone();
    assert!(
        run.accept(response)
            .unwrap_err()
            .to_string()
            .contains("exported finding bytes")
    );
    assert_eq!(run.ledger(), &before);
}

#[test]
fn completed_findings_are_reused_while_many_pages_preserve_order() {
    let input = ReviewInput::new(
        SourceIdentity::new("a", "b", b"many"),
        (0..200)
            .map(|i| Change::new(format!("file-{i}.rs"), (0, 0, 1, 1), "+source"))
            .collect(),
    );
    let mut run = ResearchRun::new(&input, ResearchPolicy::default()).unwrap();
    while let Some(task) = run.next().unwrap() {
        run.accept(notes(&task)).unwrap();
    }
    let original = run.findings().unwrap();
    let pointer = original.as_ptr();
    let mut cursor = None;
    let mut count = 0;
    loop {
        let page = run.finding_page(cursor.as_ref(), 2048).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() <= 2048);
        assert_eq!(page.start_index, count);
        assert_eq!(page.findings, original[count..count + page.findings.len()]);
        count += page.findings.len();
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(count, 200);
    assert_eq!(run.findings().unwrap().as_ptr(), pointer);
}

#[test]
fn reuse_requires_identical_change_and_policy_and_never_advances_research() {
    let before = input();
    let policy = ResearchPolicy::default();
    let mut prior = ResearchRun::new(&before, policy.clone()).unwrap();
    while let Some(task) = prior.next().unwrap() {
        prior.accept(notes(&task)).unwrap();
    }
    let mut after = before.clone();
    after.source = SourceIdentity::new("base", "new-head", b"new corpus");
    after.changes[1] = Change::new("large.svg", (0, 0, 1, 1), "+changed");
    let mut current = ResearchRun::new(&after, policy.clone()).unwrap();
    let pending = current.next().unwrap().unwrap();
    let candidate = current
        .reuse_candidate(&before, prior.ledger().clone())
        .unwrap()
        .unwrap();
    assert_eq!(candidate.source, after.source);
    assert_eq!(candidate.request_id, pending.id);
    assert_eq!(candidate.findings, notes(&pending).findings);
    assert!(
        current.ledger().clone()
            == ResearchRun::new(&after, policy.clone())
                .unwrap()
                .ledger()
                .clone()
    );
    current.accept(candidate).unwrap();
    assert!(
        current
            .reuse_candidate(&before, prior.ledger().clone())
            .unwrap()
            .is_none()
    );
    let mut changed_policy = policy;
    changed_policy.page_bytes /= 2;
    let current = ResearchRun::new(&after, changed_policy).unwrap();
    assert!(
        current
            .reuse_candidate(&before, prior.ledger().clone())
            .unwrap()
            .is_none()
    );
    let mut shifted = before.clone();
    shifted.changes[0].new_start += 1;
    let current = ResearchRun::new(&shifted, ResearchPolicy::default()).unwrap();
    assert!(
        current
            .reuse_candidate(&before, prior.ledger().clone())
            .unwrap()
            .is_none()
    );
    let mut corrupt = serde_json::to_value(prior.ledger()).unwrap();
    corrupt["records"][0]["findings"][0]["evidence"][0]["text"] = "fabricated".into();
    assert!(
        current
            .reuse_candidate(&before, serde_json::from_value(corrupt).unwrap())
            .is_err()
    );
}
