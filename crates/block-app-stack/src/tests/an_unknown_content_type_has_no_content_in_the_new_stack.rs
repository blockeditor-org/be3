use super::*;

#[test]
fn an_unknown_content_type_has_no_content_in_the_new_stack() {
    let harness = Harness::start();
    harness.connect();
    wait_until("connected and knew the counter and checklist", |shared| {
        shared.connected
            && is_known(CounterContent::CONTENT_TYPE)
            && is_known(ChecklistContent::CONTENT_TYPE)
    });
    assert!(!is_known(Uuid::from_u128(0xdead_beef)));
    let block = Uuid::new_v4();
    let asked = status().wakes;

    open(block, Uuid::from_u128(0xdead_beef));

    wait_until("took the open", |shared| shared.wakes > asked);
    assert!(content(block).is_none());
}
