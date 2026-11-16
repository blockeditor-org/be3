use be_type_chat::Deletion;

use super::*;

/// Only a message's author deletes it: a deletion someone else signed as themselves, or one
/// that keeps nothing of the recent message it removed, leaves a row marked forged where the
/// message was, rather than taking it away without a mark.
#[test]
fn a_message_deleted_by_someone_else_shows_as_forged() {
    let sam = Uuid::from_u128(0x5a30_0000_0000_4000_8000_0000_0000_0005);
    let mut chat = Harness::new(&[
        (ROSA, "Lunch at noon?"),
        (ROSA, "Bring the cake"),
        (ME, "Sounds good"),
    ]);
    let (lunch, cake) = (chat.id(0), chat.id(1));

    // Sam signs a deletion of Rosa's message as himself: his signature is good, but it is not
    // his message.
    chat.arrive(deleted_by(&chat.content(), lunch, sam, MORNING + 30 * MINUTE));
    assert_eq!(chat.row(lunch), "Member 5a30 09:30 forged Message deleted");

    // A deletion in Rosa's own name that keeps nothing of her message proves nothing of who
    // wrote it.
    let content = chat.content();
    let deletion = Deletion {
        message: None,
        ..content.deletion(cake, ROSA, MORNING + 31 * MINUTE)
    };
    let deletion = Deletion {
        signature: Signature(be_sdk_test::signature(
            ROSA,
            &deletion.signed_bytes(CHAT, cake),
        )),
        ..deletion
    };
    chat.arrive(content.put_deletion(cake, &deletion));
    assert_eq!(chat.row(cake), "Member 7e1a 09:31 forged Message deleted");
}
