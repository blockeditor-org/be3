use super::*;

#[test]
fn a_message_the_server_refuses_shows_as_not_sent() {
    let mut chat = Harness::new(&[(ROSA, "Can you still see this chat?")]);
    chat.editor.store().set_online(false);
    chat.write("I think so");
    chat.click("chat.send");
    let refused = chat.id(1);
    chat.editor.advance(Duration::from_secs(40));

    chat.editor.store().refuse_kept();
    chat.run();
    assert!(
        chat.row(refused).contains("Not sent"),
        "a message whose commit the server refused stops sending"
    );
    chat.snapshot("a_message_the_server_refuses_shows_as_not_sent");
}
