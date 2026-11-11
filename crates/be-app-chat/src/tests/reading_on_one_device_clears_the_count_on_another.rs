use be_sdk_beui::be_block::Root;
use be_type_chat::{ChatReads, ChatReadsContent};

use super::*;

const PHONE: Uuid = Uuid::from_u128(0x9407_0000_0000_4000_8000_0000_0000_0005);

#[test]
fn reading_on_one_device_clears_the_count_on_another() {
    let mut laptop = Harness::new(&[(ME, "Are the minutes up?"), (ROSA, "Minutes are up")]);
    assert!(
        !laptop.editor.shown("chat.unread"),
        "a chat shown at its newest message with the keyboard is read"
    );

    laptop.editor.blur();
    laptop.run();
    for body in ["Two more things", "Who takes the notes next week?"] {
        laptop.arrive(signed_by(
            &laptop.content(),
            ROSA,
            Message::new(ROSA, MORNING + 40 * MINUTE, body),
        ));
    }
    assert!(
        laptop
            .editor
            .label("chat.unread")
            .ends_with("2 new messages"),
        "a chat in a window without the keyboard is not read"
    );
    laptop.snapshot("a_chat_not_in_front_counts_the_messages_since");

    let reads = laptop
        .editor
        .store()
        .created()
        .into_iter()
        .find(|block| {
            laptop
                .editor
                .store()
                .block(*block)
                .is_some_and(|info| info.block_type == ChatReads::CONTENT_TYPE)
        })
        .expect("the laptop keeps where it read in its owner-only settings");
    assert!(laptop.editor.store().owner_only(reads));
    let first = laptop.id(0);
    laptop
        .editor
        .edit::<ChatReadsContent>(Some(reads), &ChatReads::set_read(CHAT, PHONE, first, &[]));
    laptop.run();
    assert!(
        laptop.editor.shown("chat.unread"),
        "the phone, behind, takes no reading back"
    );

    // The phone reads the chat, dropping the places it saw: its edit syncs to the laptop as
    // any edit does.
    let newest = laptop.content().newest().expect("the chat has messages");
    let seen = laptop
        .editor
        .content::<ChatReadsContent>(Some(reads))
        .root()
        .places(CHAT);
    laptop.editor.edit::<ChatReadsContent>(
        Some(reads),
        &ChatReads::set_read(CHAT, PHONE, newest, &seen),
    );
    laptop.run();
    assert!(
        !laptop.editor.shown("chat.unread"),
        "reading on the phone clears the laptop's count"
    );
    assert_eq!(
        laptop
            .editor
            .content::<ChatReadsContent>(Some(reads))
            .root()
            .places(CHAT)
            .into_iter()
            .map(|place| (place.device, place.message))
            .collect::<Vec<_>>(),
        [(PHONE, newest)],
        "the phone's place, further on, replaced those it saw"
    );
}
