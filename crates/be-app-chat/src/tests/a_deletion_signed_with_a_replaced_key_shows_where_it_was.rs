use super::*;

/// A deletion its author signed with a key they had replaced before the server took it leaves
/// a row where the message was, "deleted with an old key", since whoever holds the old phrase
/// could have made it. One that keeps no arrival, written outside a live session, says nothing
/// of when it came, so it answers `Changed` and removes the message without a row, as a
/// deletion with a changed key does (plans/authorship.md).
#[test]
fn a_deletion_signed_with_a_replaced_key_shows_where_it_was() {
    let lee = Uuid::from_u128(0x1ee0_0000_0000_4000_8000_0000_0000_0007);
    let mut content = conversation(&[
        (ROSA, "Lunch at noon?"),
        (lee, "I can bring dessert"),
        (lee, "And drinks"),
        (ME, "Great"),
    ]);
    let (dessert, drinks) = (content.messages()[1], content.messages()[2]);
    let unstamped = deleted_by(&content, drinks, lee, MORNING + 20 * MINUTE);
    content.apply(&unstamped.into());
    let mut chat = Harness::open(content, &[(lee, Signer::Replaced)], CHAT, false);
    assert_eq!(chat.bodies(), ["Lunch at noon?", "I can bring dessert", "Great"]);
    assert!(
        !chat.editor.shown(&format!("chat.message.{drinks}")),
        "an unstamped deletion with a replaced key has no row: {}",
        chat.row(drinks)
    );

    // The server stamps a deletion sent live.
    chat.arrive(deleted_by(
        &chat.content(),
        dessert,
        lee,
        MORNING + 30 * MINUTE,
    ));
    assert_eq!(chat.bodies(), ["Lunch at noon?", "Great"]);
    assert_eq!(
        chat.row(dessert),
        "Member 1ee0 09:30 deleted with an old key Message deleted"
    );
    chat.snapshot("a_deletion_signed_with_a_replaced_key_shows_where_it_was");
}
