use super::*;

/// A block open here and in step with the server, which another device writes outside its
/// session (a plain save over the head this device holds): the replica moves to what was
/// written, as a fast-forward, with nothing marked as a conflict.
#[test]
fn a_note_written_outside_its_session_is_taken_as_written() {
    let harness = Harness::start();
    harness.connect();
    let note = Uuid::new_v4();
    create(
        note,
        be_type_text::TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Note"),
        Some(be_type_text::TextBlock::of("hello\nthere\n").encode()),
        false,
    );
    open(note, be_type_text::TextContent::CONTENT_TYPE);
    wait_until("opened the note", |shared| text_of(shared, note).is_some());
    type_text(note, 0, "Hi\n");
    flush_and_send();

    runtime().block_on(async {
        let peer = harness.outside_peer().await;
        let head = peer
            .remote_head(note)
            .await
            .expect("the note's head is read");
        peer.save(note, &be_type_text::TextBlock::of("Oh\nhello\nthere\n"), head)
            .await
            .expect("the other device saves the note");
    });
    wait_until("took what the other device wrote", |shared| {
        text_of(shared, note).as_deref() == Some("Oh\nhello\nthere\n")
    });
}
