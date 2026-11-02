use super::*;

#[test]
fn opens_and_closes_editor_instance() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    accept(&mut session);
    let instance = EditorInstanceId(4);
    session.receive(&opening(instance)).unwrap();
    session.receive(&Delivery::Close(instance)).unwrap();
    assert_eq!(session.state(), State::Running);
    assert!(session.receive(&Delivery::Close(instance)).is_err());
}
