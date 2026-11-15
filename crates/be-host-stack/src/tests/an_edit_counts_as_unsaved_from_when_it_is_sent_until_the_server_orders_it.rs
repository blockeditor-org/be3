use super::*;

/// An edit is unsaved from when it is sent until the server's order holds it: while the server's
/// answers are held, the edit shows but stays unsaved, and once they arrive nothing is.
#[test]
fn an_edit_counts_as_unsaved_from_when_it_is_sent_until_the_server_orders_it() {
    let harness = Harness::relayed();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    flush();
    assert_eq!(status().unsaved, 0);

    harness.relay().hold_answers(true);
    add(block, 3);
    assert!(status().unsaved > 0, "an edit not yet taken is unsaved");
    wait_for_count(block, 3);
    assert!(
        status().unsaved > 0,
        "an edit the module reported but the server has not ordered is unsaved"
    );

    harness.relay().hold_answers(false);
    wait_until("the server ordered the edit", |shared| shared.undurable == 0);
    assert_eq!(flush(), 0);
}
