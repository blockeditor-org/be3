use super::*;
use text_block_types::{TextBlock, TextContent};

#[test]
fn a_paste_too_large_to_relay_is_acknowledged_to_its_editor() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, TextContent::CONTENT_TYPE);
    let mut editor = Editor::<TextContent>::attach(block);
    type_into(&mut editor, 0, "start\n");
    wait_until("typed the first line", |shared| {
        text_of(shared, block).as_deref() == Some("start\n")
    });

    let pasted = "a line of pasted text\n".repeat(100_000);
    type_into(&mut editor, 6, &pasted);
    let expected = format!("start\n{pasted}");
    wait_until("took the paste", |shared| {
        text_of(shared, block).as_deref() == Some(expected.as_str())
    });

    editor.send(block_type_guest::editor::FromEditor::Resend);
    editor.wait("was sent a snapshot holding the paste", |text| {
        TextBlock::text(text) == expected
    });
    assert_eq!(
        editor.mirror.pending(),
        0,
        "the editor that pasted would apply its paste a second time over content that already holds it"
    );
}
