use super::*;

#[test]
fn rejects_out_of_order_messages() {
    let mut session = ClientSession::default();
    assert!(session.receive(&input(ScreenId(1))).is_err());
    assert_eq!(session.state(), State::AwaitingHello);
}
