use super::*;
use be_sdk_type_guest::editor::{FromEditor, ToEditor};
use be_type_text::TextContent;

/// Asks the replica to put `bytes` as a part of the editor's block and waits for its answer.
fn put(editor: &mut Editor<TextContent>, token: u64, bytes: &[u8]) -> Result<be_block::PartRef, String> {
    editor.send(FromEditor::PutPart {
        token,
        bytes: bytes.to_vec(),
    });
    let id = editor.id;
    loop {
        for message in editor.take_received() {
            if let ToEditor::PartPut { token: put, result } = message
                && put == token
            {
                return result;
            }
        }
        crate::wait_until(|shared| shared.editor_frames.contains_key(&id).then_some(()));
    }
}

#[test]
fn a_part_an_editor_puts_is_held_for_its_block_once_the_server_has_it() {
    let mut harness = Harness::start();
    harness.connect();
    let address = harness.address();
    let segment: Vec<u8> = (0..64 * 1024u32)
        .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();

    let block = Uuid::new_v4();
    open(block, TextContent::CONTENT_TYPE);
    let mut editor = Editor::<TextContent>::attach(block);
    type_into(&mut editor, 0, "an index\n");
    wait_until("typed", |shared| {
        text_of(shared, block).as_deref() == Some("an index\n")
    });
    flush();
    let part = put(&mut editor, 1, &segment).expect("the part is put");
    let read = runtime().block_on(async {
        harness
            .outside_peer()
            .await
            .read_part(part, 0, segment.len() as u64)
            .await
    });
    assert_eq!(
        read.expect("the block holds the part, so another device reads it"),
        segment
    );
    drop(editor);

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    let made_offline = Uuid::new_v4();
    open(made_offline, TextContent::CONTENT_TYPE);
    let mut editor = Editor::<TextContent>::attach(made_offline);
    type_into(&mut editor, 0, "made offline\n");
    let refused = put(&mut editor, 2, &segment).expect_err("nothing can hold a part offline");
    assert!(
        refused.contains("does not have this block yet"),
        "a block the server does not have yet refuses at once: {refused}"
    );
    drop(editor);
    harness.start_server(&address);
}
