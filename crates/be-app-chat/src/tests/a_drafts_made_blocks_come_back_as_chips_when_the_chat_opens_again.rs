use super::*;

#[test]
fn a_drafts_made_blocks_come_back_as_chips_when_the_chat_opens_again() {
    let mut chat = Harness::new(&[(ROSA, "Send me the sketch")]);
    let sketch = Uuid::from_u128(0x5c7c);
    made_in_chat(&mut chat, sketch, "Sketch");
    attach(&mut chat, sketch, false);
    assert!(chat.editor.shown(&format!("chat.attaching.{sketch}")));

    // Closing the editor deletes nothing: the chat keeps the block as a draft.
    let again = chat.reopened();
    drop(chat);
    let mut again = again;
    again.run();
    assert_eq!(again.content().drafts_of(ME), [sketch]);
    assert!(again.content().references().contains(&sketch));
    assert!(deleted(&mut again).is_empty());
    assert!(
        again
            .editor
            .label(&format!("chat.attaching.{sketch}"))
            .contains("Sketch"),
        "the draft's made block shows as a chip again"
    );

    // Sent, it is the message's.
    again.click("chat.send");
    again.editor.advance(Duration::from_millis(500));
    again.run();
    let sent = again.id(1);
    assert_eq!(
        again
            .content()
            .message(sent)
            .map(|message| message.attachments),
        Some(vec![
            be_sdk_beui::be_block_content::be_block_model::BlockRef(sketch)
        ])
    );
    assert!(again.content().drafts_of(ME).is_empty());
}
