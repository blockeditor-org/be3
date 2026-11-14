use super::*;

#[test]
fn ignores_input_for_a_screen_taken_back_but_refuses_one_never_given() {
    let mut session = ClientSession::default();
    accept(&mut session);
    let instance = EditorInstanceId(1);
    session.receive(&opening(instance)).unwrap();
    session
        .receive(&Delivery::Screens(vec![screen(ScreenId(3), instance)]))
        .unwrap();
    // The host took screen 3 back while input for it was on its way.
    session.receive(&Delivery::Screens(Vec::new())).unwrap();
    session.receive(&input(ScreenId(3))).unwrap();
    assert!(session.receive(&input(ScreenId(4))).is_err());
    assert_eq!(session.state(), State::Running);
}
