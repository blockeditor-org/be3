use super::*;

#[test]
fn refuses_to_draw_without_a_surface() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    session.receive(Message::HelloAccepted(HelloAccepted {
        host_name: "test host".into(),
        surface: None,
        preferences: block_plugin_api::Preferences::default(),
    }));
    assert_eq!(session.state(), State::Running);
    assert_eq!(session.surface(), None);
    let asked = session.receive(Message::DrawFrame {
        now_micros: 0,
        seen: true,
    });
    assert!(matches!(asked.as_slice(), [Message::Error(_)]));
    assert_eq!(session.state(), State::Failed);
}
