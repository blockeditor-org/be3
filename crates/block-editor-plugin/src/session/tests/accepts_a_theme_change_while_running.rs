use super::*;

#[test]
fn accepts_a_theme_change_while_running() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    accept(&mut session);
    assert_eq!(
        session.receive(Message::Theme(Theme { dark: false })),
        Vec::new()
    );
    assert_eq!(session.state(), State::Running);
}
