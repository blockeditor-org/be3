use super::*;

#[test]
fn a_message_kept_offline_tells_the_other_members_only_once_it_reaches_the_server() {
    let mut chat = Harness::new(&[(ROSA, "Are you on the train yet?")]);
    chat.editor.store().set_online(false);
    chat.write("Running late");
    chat.click("chat.send");
    chat.editor.advance(Duration::from_secs(40));
    chat.run();
    assert!(
        chat.editor.take_notifications().is_empty(),
        "a message only this device holds tells nobody yet"
    );

    chat.editor.store().set_online(true);
    chat.run();
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    assert_eq!(
        chat.editor.take_notifications(),
        [(CHAT, "Running late".to_owned(), Vec::new())],
        "once the drain published it, the other members are told"
    );
}
