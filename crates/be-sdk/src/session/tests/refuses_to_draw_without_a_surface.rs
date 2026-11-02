use super::*;

#[test]
fn refuses_to_draw_without_a_surface() {
    let mut session = ClientSession::new("be3.counter", "Counter", "1");
    session.receive(&accepted(None)).unwrap();
    assert_eq!(session.state(), State::Running);
    assert_eq!(session.surface(), None);
    let draw = Delivery::Draw {
        now_micros: 0,
        wall_micros: 0,
        seen: true,
    };
    assert!(session.receive(&draw).is_err());
}
