use super::*;

/// A gap shows only once the message that names the missing one verifies: someone with Edit who
/// removes a message and forges a later one naming it shows no missing row.
#[test]
fn a_forged_message_naming_a_removed_one_shows_no_gap() {
    let sam = Uuid::from_u128(0x5a30_0000_0000_4000_8000_0000_0000_0005);
    let mut chat = Harness::new(&[
        (ROSA, "Morning! Is the release still on for today?"),
        (ME, "It is. I'm writing the notes now."),
    ]);
    let gone = chat.id(1);
    let forged = signed_by(
        &chat.content(),
        sam,
        Message::new(ROSA, MORNING + 20 * MINUTE, "Rosa never wrote this"),
    );
    chat.arrive(forged);
    let naming = chat.id(2);
    assert!(chat.row(naming).contains("forged"));

    chat.arrive(
        [be_sdk_beui::be_block_content::be_block_model::Change::remove(gone)]
            .into_iter()
            .collect(),
    );
    assert!(
        !chat.editor.shown(&format!("chat.missing.{gone}")),
        "a forged message's gap does not show"
    );
}
