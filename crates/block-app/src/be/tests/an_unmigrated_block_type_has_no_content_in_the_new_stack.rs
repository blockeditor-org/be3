use super::*;

#[test]
fn an_unmigrated_block_type_has_no_content_in_the_new_stack() {
    use block::Block;

    assert_eq!(
        content_type_for(block_client::blocks::counter::Counter::TYPE_ID),
        Some(CounterContent::CONTENT_TYPE)
    );
    assert_eq!(
        content_type_for(block_client::blocks::text::TextDocument::TYPE_ID),
        None
    );

    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, Uuid::from_u128(0xdead_beef));
    std::thread::sleep(Duration::from_millis(200));

    assert!(content(block).is_none());
}
