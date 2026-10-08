use super::*;

use be_block::TextContent;

#[test]
fn a_paste_too_large_to_relay_is_acknowledged_to_its_editor() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, TextContent::CONTENT_TYPE);
    wait_until("opened the text", |shared| {
        shared.blocks.contains_key(&block)
    });
    type_text(block, 0, "start\n");
    wait_until("typed the first line", |shared| {
        text_of(shared, block).as_deref() == Some("start\n")
    });

    let pasted = "a line of pasted text\n".repeat(100_000);
    type_text(block, 6, &pasted);
    let expected = format!("start\n{pasted}");
    wait_until("took the paste", |shared| {
        text_of(shared, block).as_deref() == Some(expected.as_str())
    });

    let Some(Update::Snapshot { applied, .. }) =
        content(block).map(|held| held.since(TEST_ORIGIN, None))
    else {
        panic!("the editor that pasted was not offered a snapshot");
    };
    assert_eq!(
        applied, 2,
        "the editor that pasted would apply its paste a second time over content that already holds it"
    );
}
