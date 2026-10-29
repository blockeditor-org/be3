use super::*;

#[test]
fn an_edit_counts_as_unsaved_from_when_it_is_sent_until_it_is_sealed() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    flush();
    assert_eq!(status().unsaved, 0);

    add(block, 3);
    assert!(status().unsaved > 0, "an edit not yet taken is unsaved");
    wait_until("took the edit", |shared| shared.pending == 0);
    assert!(
        status().unsaved > 0,
        "an edit the module has not yet reported is unsaved"
    );
    wait_for_count(block, 3);
    assert_eq!(flush(), 0);
}
