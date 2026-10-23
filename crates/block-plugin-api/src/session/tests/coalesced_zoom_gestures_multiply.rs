use super::*;

#[test]
fn coalesced_zoom_gestures_multiply() {
    let mut session = running_session();
    session.enqueue(input(Input::Zoom(2.0))).unwrap();
    session.enqueue(input(Input::Zoom(1.5))).unwrap();

    assert_eq!(session.queued_message_count(), 1);
    assert_eq!(session.next_outbound(), Some(input(Input::Zoom(3.0))));
}
