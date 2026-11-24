use super::*;

/// A message its author deleted is gone; one deleted with a key its author had replaced shows
/// where it was, saying so, since someone holding the old phrase may have deleted it.
#[test]
fn a_message_deleted_with_an_old_key_says_so_in_its_place() {
    let mut chat = Harness::new(&[(ROSA, "First"), (ROSA, "Second"), (ROSA, "Third")]);
    let (first, second) = (chat.seq(0), chat.seq(1));
    chat.arrive(
        ROSA,
        &ChatEntry::Delete {
            message: first,
            time: MORNING,
        },
    );
    assert!(!chat.editor.shown(&format!("chat.message.{first}")));

    chat.editor
        .store()
        .set_log_signer(ROSA, Signer::Replaced);
    chat.arrive(
        ROSA,
        &ChatEntry::Delete {
            message: second,
            time: MORNING,
        },
    );
    assert_eq!(
        chat.row(second),
        "Member 7e1a 09:03 deleted with an old key Message deleted"
    );
    assert_eq!(chat.bodies(), ["Third"]);
}
