use super::*;

#[test]
fn an_unmigrated_block_type_has_no_content_in_the_new_stack() {
    use block::Block;

    assert_eq!(
        content_type_for(block_client::blocks::counter::Counter::TYPE_ID),
        Some(CounterContent::CONTENT_TYPE)
    );
    assert_eq!(
        content_type_for(block_client::blocks::checklist::Checklist::TYPE_ID),
        Some(ChecklistContent::CONTENT_TYPE)
    );
    assert_eq!(
        content_type_for(block_client::blocks::text::TextDocument::TYPE_ID),
        None
    );

    let harness = Harness::start();
    harness.connect();
    wait_until("connected", |shared| shared.connected);
    let block = Uuid::new_v4();
    let asked = status().wakes;

    open(block, Uuid::from_u128(0xdead_beef));

    wait_until("took the open", |shared| shared.wakes > asked);
    assert!(content(block).is_none());
}
