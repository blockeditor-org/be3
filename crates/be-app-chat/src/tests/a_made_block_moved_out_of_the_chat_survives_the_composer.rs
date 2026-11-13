use super::*;

#[test]
fn a_made_block_moved_out_of_the_chat_survives_the_composer() {
    let mut chat = Harness::new(&[]);
    let sketch = Uuid::from_u128(0x5c7c);
    made_in_chat(&mut chat, sketch, "Sketch");
    attach(&mut chat, sketch, false);

    // Moved into a folder from the files pane: its parent changes before the chat hears of it.
    let folder = Uuid::from_u128(0xf01d);
    let mut moved = chat
        .editor
        .store()
        .block(sketch)
        .expect("the sketch is known");
    moved.parent = Some(BlockParent::Block(folder));
    chat.editor.store().add_block(moved);
    chat.run();
    chat.click(&format!("chat.attaching.{sketch}.remove"));
    assert_eq!(
        chat.content().drafts_of(ME),
        [sketch],
        "removing the chip takes nothing out of the chat once the block is elsewhere"
    );
    assert!(deleted(&mut chat).is_empty());

    // The move takes it out of the chat, as the host's delete from the old parent does.
    let taken = chat
        .content()
        .drop_draft(sketch)
        .expect("a draft can be taken out");
    chat.arrive(taken);
    assert!(!chat.content().references().contains(&sketch));

    // Closing the composer deletes nothing, and the block is not a draft when it opens again.
    let again = chat.reopened();
    drop(chat);
    let mut again = again;
    again.run();
    assert!(!again.editor.shown(&format!("chat.attaching.{sketch}")));
    assert_eq!(
        again
            .editor
            .store()
            .block(sketch)
            .and_then(|info| info.parent),
        Some(BlockParent::Block(folder)),
        "the block stays where it was moved"
    );
    assert!(deleted(&mut again).is_empty());
}
