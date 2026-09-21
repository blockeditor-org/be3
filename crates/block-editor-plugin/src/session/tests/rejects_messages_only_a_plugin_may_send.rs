use super::*;

#[test]
fn rejects_messages_only_a_plugin_may_send() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    accept(&mut session);
    let responses = session.receive(Message::FrameNeeded);
    assert!(matches!(responses.as_slice(), [Message::Error(_)]));
    assert_eq!(session.state(), State::Failed);
}
