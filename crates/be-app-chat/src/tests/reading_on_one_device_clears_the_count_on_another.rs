use be_sdk_beui::be_block::Root;
use be_sdk_beui::beui::vec2;
use be_type_chat::{ChatReads, ChatReadsContent};

use super::*;

#[test]
fn reading_on_one_device_clears_the_count_on_another() {
    let lines: Vec<(Uuid, String)> = (0..12)
        .map(|line| match line % 2 {
            0 => (ROSA, format!("Line {line} of the minutes")),
            _ => (ME, format!("Noted, line {line}")),
        })
        .collect();
    let lines: Vec<(Uuid, &str)> = lines
        .iter()
        .map(|(author, body)| (*author, body.as_str()))
        .collect();
    let mut laptop = Harness::new(&lines);
    assert!(
        !laptop.editor.shown("chat.unread"),
        "a chat opened at its newest message is read"
    );

    let last = laptop.id(11);
    laptop
        .editor
        .scroll(format!("chat.message.{last}"), vec2(0.0, -400.0));
    laptop.run();
    for body in ["Two more things", "Who takes the notes next week?"] {
        laptop.arrive(
            laptop
                .content()
                .send(CHAT, Message::new(ROSA, MORNING + 40 * MINUTE, body))
                .1,
        );
    }
    assert!(
        laptop
            .editor
            .label("chat.unread")
            .ends_with("2 new messages")
    );
    laptop.snapshot("a_chat_scrolled_back_counts_the_messages_since");

    // The phone, at the newest message, reads the chat: its write reaches the laptop's
    // owner-only settings as any synced edit does.
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
    let newest = laptop.content().newest().expect("the chat has messages");
    laptop
        .editor
        .edit::<ChatReadsContent>(Some(reads), &ChatReads::set_read(CHAT, &newest));
    laptop.run();
    assert!(
        !laptop.editor.shown("chat.unread"),
        "reading on the phone clears the laptop's count"
    );
}
