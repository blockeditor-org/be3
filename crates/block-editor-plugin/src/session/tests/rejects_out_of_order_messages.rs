use super::*;

#[test]
fn rejects_out_of_order_messages() {
    let mut session = ClientSession::default();
    let responses = session.receive(Message::Input(InputBatch {
        screen: ScreenId(1),
        every_sample: false,
        events: Vec::new(),
    }));
    assert!(matches!(responses.as_slice(), [Message::Error(_)]));
    assert_eq!(session.state(), State::Failed);
}
