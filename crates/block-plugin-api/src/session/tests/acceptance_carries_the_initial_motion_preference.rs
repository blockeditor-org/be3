use super::*;

#[test]
fn acceptance_carries_the_initial_motion_preference() {
    let mut session = session();
    session.set_motion(crate::Motion::Instant);
    session.start(0);
    session.receive_frame(&encode_frame(&hello()).unwrap(), 1);
    let Some(Message::HelloAccepted(accepted)) = session.next_outbound() else {
        panic!("expected acceptance");
    };
    assert_eq!(accepted.motion, crate::Motion::Instant);
}
