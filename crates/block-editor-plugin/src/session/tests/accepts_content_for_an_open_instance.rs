use super::*;

#[test]
fn accepts_content_for_an_open_instance() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    accept(&mut session);
    let instance = EditorInstanceId(4);
    open(&mut session, instance);

    let answered = session.receive(content(instance));

    assert!(answered.is_empty());
    assert_eq!(session.state(), State::Running);
}
