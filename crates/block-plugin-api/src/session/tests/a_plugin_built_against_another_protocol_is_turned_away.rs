use super::*;

#[test]
fn a_plugin_built_against_another_protocol_is_turned_away() {
    let mut session = session();
    session.start(0);
    let Message::Hello(mut hello) = hello() else {
        unreachable!()
    };
    hello.fingerprint = PROTOCOL_FINGERPRINT.wrapping_add(1);
    session.receive_frame(&encode_frame(&Message::Hello(hello)).unwrap(), 1);
    assert!(matches!(session.state(), SessionState::Failed(_)));
    assert!(matches!(
        session.next_outbound(),
        Some(Message::HelloRejected(ProtocolError {
            code: ErrorCode::DifferentProtocol,
            ..
        }))
    ));
}
