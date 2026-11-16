use be_type_chat::{Deletion, Sent};

use super::*;

/// Only a message's author deletes it: a deletion someone else signed as themselves, one that
/// keeps a made-up message of its own author's in place of the one a later message cites, or
/// one that keeps nothing of the recent message it removed, leaves a row marked forged where
/// the message was, rather than taking it away without a mark.
#[test]
fn a_message_deleted_by_someone_else_shows_as_forged() {
    let sam = Uuid::from_u128(0x5a30_0000_0000_4000_8000_0000_0000_0005);
    let mut chat = Harness::new(&[
        (ROSA, "Lunch at noon?"),
        (ROSA, "Bring the cake"),
        (ROSA, "Who is in?"),
        (ME, "Sounds good"),
    ]);
    let (lunch, cake, who) = (chat.id(0), chat.id(1), chat.id(2));

    // Sam signs a deletion of Rosa's message as himself: his signature is good, but it is not
    // his message.
    chat.arrive(deleted_by(&chat.content(), lunch, sam, MORNING + 30 * MINUTE));
    assert_eq!(chat.row(lunch), "Member 5a30 09:30 forged Message deleted");

    // Sam keeps a message of his own making, signed by himself, in place of Rosa's: the reply
    // after it cites Rosa's by its hash, which his does not have.
    let content = chat.content();
    let made_up = Sent {
        author: sam,
        time: MORNING + 6 * MINUTE,
        content: [7; 32],
        ..Sent::default()
    };
    let made_up = Sent {
        signature: Signature(be_sdk_test::signature(
            sam,
            &made_up.signed_bytes(CHAT, who),
        )),
        ..made_up
    };
    let deletion = Deletion {
        author: sam,
        message: Some(made_up),
        ..content.deletion(who, sam, MORNING + 32 * MINUTE)
    };
    let deletion = Deletion {
        signature: Signature(be_sdk_test::signature(
            sam,
            &deletion.signed_bytes(CHAT, who),
        )),
        ..deletion
    };
    chat.arrive(content.put_deletion(who, &deletion));
    assert_eq!(chat.row(who), "Member 5a30 09:32 forged Message deleted");

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
