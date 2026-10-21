use super::*;

#[test]
fn accepts_new_preferences_while_running() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    accept(&mut session);
    let preferences = block_plugin_api::Preferences {
        theme: block_plugin_api::ThemeKind::Light,
        ..Default::default()
    };
    assert_eq!(
        session.receive(Message::Preferences(preferences)),
        Vec::new()
    );
    assert_eq!(session.state(), State::Running);
}
