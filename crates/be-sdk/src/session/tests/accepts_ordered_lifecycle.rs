use super::*;

#[test]
fn accepts_ordered_lifecycle() {
    let mut session = ClientSession::default();
    assert_eq!(session.hello().plugin.id, "");
    accept(&mut session);
    assert_eq!(session.state(), State::Running);

    let instance = EditorInstanceId(1);
    session.receive(&opening(instance)).unwrap();
    session
        .receive(&Delivery::Screens(vec![screen(ScreenId(3), instance)]))
        .unwrap();
    session.receive(&input(ScreenId(3))).unwrap();
    session.receive(&Delivery::Shutdown).unwrap();
    assert_eq!(session.state(), State::Closed);
}
