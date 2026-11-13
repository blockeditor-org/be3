use super::*;
use be_type_chat::{Deletion, MessageEdit, Reaction};

fn sign(account: Uuid, bytes: &[u8]) -> Signature {
    Signature(be_sdk_test::signature(account, bytes))
}

/// `body` as an edit of `message` by Rosa, signed by `signer`.
fn edit_by(chat: &Harness, message: ObjectId, body: &str, time: i64, signer: Uuid) -> MessageEdit {
    let content = chat.content();
    let edit = content.next_edit(message, ROSA, time, body);
    let of = content.hash(CHAT, message).expect("the message is recent");
    MessageEdit {
        signature: sign(signer, &edit.signed_bytes(CHAT, message, of)),
        ..edit
    }
}

#[test]
fn a_replayed_edit_a_forged_deletion_and_a_reaction_taken_back_by_someone_else_show_as_forged() {
    let sam = Uuid::from_u128(0x5a30_0000_0000_4000_8000_0000_0000_0005);
    let mut chat = Harness::new(&[
        (ROSA, "Lunch at noon?"),
        (ROSA, "Bring the cake"),
        (ROSA, "Who is in?"),
        (ROSA, "Thanks, all"),
    ]);
    let (lunch, cake, who, thanks) = (chat.id(0), chat.id(1), chat.id(2), chat.id(3));

    // Rosa edits twice, each edit naming the one before: both are hers.
    let first = edit_by(&chat, lunch, "Lunch at one?", MORNING + 30 * MINUTE, ROSA);
    chat.arrive(chat.content().put_edit(lunch, &first));
    let second = edit_by(&chat, lunch, "Lunch at two?", MORNING + 31 * MINUTE, ROSA);
    chat.arrive(chat.content().put_edit(lunch, &second));
    assert_eq!(chat.row(lunch), "Member 7e1a 09:00 edited Lunch at two?");
    // Someone with Edit puts her first edit back: its signature is hers, but it was replaced.
    chat.arrive(chat.content().put_edit(lunch, &first));
    assert_eq!(
        chat.row(lunch),
        "Member 7e1a 09:00 edited forged Lunch at one?"
    );

    // Rosa deletes a message herself: it goes without a trace.
    let content = chat.content();
    let deletion = content.deletion(thanks, ROSA, MORNING + 32 * MINUTE);
    let deletion = Deletion {
        signature: sign(ROSA, &deletion.signed_bytes(CHAT, thanks)),
        ..deletion
    };
    chat.arrive(content.put_deletion(thanks, &deletion));
    // Someone else deletes another in her name: its row stays, marked forged.
    let content = chat.content();
    let deletion = content.deletion(cake, ROSA, MORNING + 33 * MINUTE);
    let deletion = Deletion {
        signature: sign(sam, &deletion.signed_bytes(CHAT, cake)),
        ..deletion
    };
    chat.arrive(content.put_deletion(cake, &deletion));
    assert_eq!(chat.row(cake), "Member 7e1a 09:33 forged Message deleted");
    assert!(
        !chat.editor.shown(&format!("chat.message.{thanks}")),
        "a deletion its author signed leaves no row"
    );

    // Rosa reacts, and someone else takes her reaction back in her name.
    let (key, reaction) = chat
        .content()
        .reaction(who, ROSA, "heart", MORNING + 34 * MINUTE, true);
    let reaction = Reaction {
        signature: sign(ROSA, &key.signed_bytes(CHAT, &reaction)),
        ..reaction
    };
    chat.arrive(chat.content().put_reaction(&key, &reaction));
    assert_eq!(chat.row(who), "Member 7e1a 09:06 Who is in? 1");
    let (key, taken_back) =
        chat.content()
            .reaction(who, ROSA, "heart", MORNING + 35 * MINUTE, false);
    let taken_back = Reaction {
        signature: sign(sam, &key.signed_bytes(CHAT, &taken_back)),
        ..taken_back
    };
    chat.arrive(chat.content().put_reaction(&key, &taken_back));
    assert_eq!(chat.row(who), "Member 7e1a 09:06 forged Who is in?");
    chat.snapshot(
        "a_replayed_edit_a_forged_deletion_and_a_reaction_taken_back_by_someone_else_show_as_forged",
    );
}
