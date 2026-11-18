use super::*;

/// What a message carries is listed as the message's once its entry has an id. One whose editor
/// closed before the host answered the append is listed by the next editor that opens: the
/// message is in the log, and what it carries is still this account's draft.
#[test]
fn a_message_sent_as_its_editor_closed_still_carries_its_blocks() {
    let mut chat = Harness::new(&[]);
    let sketch = Uuid::from_u128(0x5c7c);
    made_in_chat(&mut chat, sketch, "Sketch");
    attach(&mut chat, sketch, false);
    assert_eq!(chat.content().drafts_of(ME), [sketch]);

    // The editor that sent it is gone before the answer: its entry reaches the log alone.
    chat.editor.store().close_replica(None);
    chat.editor
        .store()
        .log_entry(
            None,
            ME,
            ChatEntry::Message {
                time: MORNING,
                body: "The sketch".to_owned(),
                reply_to: None,
                attachments: vec![be_sdk_beui::be_block_content::be_block_model::BlockRef(
                    sketch,
                )],
            }
            .encode(),
            None,
        );
    let again = chat.reopened();
    drop(chat);
    assert!(
        again.content().drafts_of(ME).is_empty(),
        "the block leaves the drafts"
    );
    assert!(
        again.content().references().contains(&sketch),
        "and is listed as the message's"
    );
    let sent = again.seq(0);
    assert!(
        again
            .editor
            .shown(&format!("chat.message.{sent}.attachment.{sketch}")),
        "the message shows what it carries"
    );
    assert!(!again.editor.shown(&format!("chat.attaching.{sketch}")));
}
