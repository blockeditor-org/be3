use be_block_content::LiveEdit;
use be_block_model::Document;

use super::*;
use crate::{ChatHeader, MessageEdit};

/// Writes to one key folded into one operation keep the one that wins applied: an earlier edit
/// put back after a later one, with a later clock but a lower count, is the one dropped.
#[test]
fn an_edit_put_back_in_one_burst_with_a_later_one_leaves_the_later() {
    let chat = sent(&ChatContent::default(), ADA, &["lunch at noon?"]);
    let lunch = first(&chat);
    let once = chat.next_edit(lunch, ADA, 5, "lunch at one?");
    let chat = edited(&chat, [chat.put_edit(lunch, &once)]);
    let twice = chat.next_edit(lunch, ADA, 6, "lunch at two?");

    let mut burst = chat.put_edit(lunch, &twice);
    let put_back = chat.put_edit(
        lunch,
        &MessageEdit {
            time: 50,
            ..once.clone()
        },
    );
    let rest = <Document<ChatHeader> as LiveEdit>::absorb_operation(&mut burst, put_back);
    assert!(rest.is_none(), "the burst takes it in");
    assert_eq!(
        edited(&chat, [burst]).body(lunch).as_deref(),
        Some("lunch at two?")
    );
}
