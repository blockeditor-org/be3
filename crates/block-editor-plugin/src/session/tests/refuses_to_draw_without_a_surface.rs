use super::*;

#[test]
fn refuses_to_draw_without_a_surface() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    session.receive(Message::HelloAccepted(HelloAccepted {
        version: PROTOCOL_VERSION,
        host_name: "test host".into(),
        surface: None,
        theme: Theme { dark: true },
        panes: false,
    }));
    assert_eq!(session.state(), State::Running);
    assert_eq!(session.surface(), None);
    let asked = session.receive(Message::DrawFrame);
    assert!(matches!(asked.as_slice(), [Message::Error(_)]));
    assert_eq!(session.state(), State::Failed);
}
