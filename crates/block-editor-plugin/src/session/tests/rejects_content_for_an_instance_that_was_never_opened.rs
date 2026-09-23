use super::*;

#[test]
fn rejects_content_for_an_instance_that_was_never_opened() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    accept(&mut session);

    session.receive(content(EditorInstanceId(9)));

    assert_eq!(session.state(), State::Failed);
}
