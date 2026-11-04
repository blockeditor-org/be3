use super::*;

#[test]
fn rejects_screens_for_unopened_instances() {
    let mut session = ClientSession::default();
    accept(&mut session);
    let screens = Delivery::Screens(vec![screen(ScreenId(1), EditorInstanceId(9))]);
    assert!(session.receive(&screens).is_err());
}
