//! Writing one batch ahead of the reviewer: the next is ready when they
//! finish, and no more than two batches are ever unread.

use super::tests::fixture;

/// Asked for by the reviewer after a Cursor run of #276 made them wait
/// minutes after every section for the next to be written.
#[test]
fn the_next_batch_is_written_while_the_reviewer_reads_the_last() {
    let mut fixture = fixture();
    fixture.runner.write_ahead = true;

    // The first batch is accepted, and the harness asks for the next at once.
    let (list, ahead) = fixture.append_first();
    assert!(
        ahead.starts_with("# Gewu harness: expand `item-1`"),
        "{ahead}"
    );
    assert!(ahead.contains("written ahead of them"), "{ahead}");

    // The second is accepted; the first is still unread, so the gate holds.
    fixture.stage_expansion(&list.items[1]);
    let waiting = fixture.runner.next().expect("append the second");
    assert!(
        waiting.starts_with("# Gewu harness: waiting for the reviewer"),
        "{waiting}"
    );
    assert!(
        !fixture.runner.would_expand().expect("gate"),
        "nothing is left"
    );

    // Nothing is left to write, so even the last batch must be read first.
    fixture.record_reviewed("chunk-item-0");
    let still = fixture.runner.next().expect("gate");
    assert!(still.contains("still unreviewed"), "{still}");
    fixture.record_reviewed("chunk-item-1");
    let complete = fixture.runner.next().expect("complete");
    assert!(complete.starts_with("# COMPLETE"), "{complete}");
}
