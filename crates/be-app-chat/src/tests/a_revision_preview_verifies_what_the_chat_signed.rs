use super::*;

#[test]
fn a_revision_preview_verifies_what_the_chat_signed() {
    let chat = Harness::previewing(&[(ROSA, "Signed in the chat"), (ME, "Signed by me")]);
    let rows: Vec<String> = (0..2).map(|index| chat.row(chat.id(index))).collect();
    assert_eq!(
        rows,
        [
            "Member 7e1a 09:00 Signed in the chat",
            "You 09:03 Signed by me"
        ],
        "a preview of the chat's history shows its messages as signed, not forged"
    );
}
