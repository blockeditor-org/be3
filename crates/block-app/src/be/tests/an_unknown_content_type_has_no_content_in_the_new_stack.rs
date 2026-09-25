use super::*;

#[test]
fn an_unknown_content_type_has_no_content_in_the_new_stack() {
    assert!(is_known(CounterContent::CONTENT_TYPE));
    assert!(is_known(ChecklistContent::CONTENT_TYPE));
    assert!(!is_known(Uuid::from_u128(0xdead_beef)));

    let harness = Harness::start();
    harness.connect();
    wait_until("connected", |shared| shared.connected);
    let block = Uuid::new_v4();
    let asked = status().wakes;

    open(block, Uuid::from_u128(0xdead_beef));

    wait_until("took the open", |shared| shared.wakes > asked);
    assert!(content(block).is_none());
}
