use super::*;

/// What a message sent offline announces waits with it, and goes once the server takes it:
/// several taken at once go as one.
#[test]
fn a_message_kept_offline_tells_the_other_members_only_once_it_reaches_the_server() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);
    chat.editor.store().set_online(false);
    chat.send("Just got on");
    chat.send("No signal in the tunnel");
    assert!(
        chat.editor.take_announced().is_empty(),
        "nothing is told while the server lacks it"
    );

    chat.editor.store().set_online(true);
    chat.run();
    assert_eq!(
        chat.editor.take_announced(),
        [(CHAT, "2 new messages".to_owned(), Vec::new())],
        "the two told the other members as one once the server took them"
    );
}
