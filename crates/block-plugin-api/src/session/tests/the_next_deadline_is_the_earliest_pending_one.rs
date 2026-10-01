use super::*;

#[test]
fn the_next_deadline_is_the_earliest_pending_one() {
    let mut session = session();
    assert_eq!(session.next_deadline(), None);
    session.start(3);
    assert_eq!(
        session.next_deadline(),
        Some(3 + REQUEST_TIMEOUT_MILLISECONDS)
    );
    session.receive_frame(&encode_frame(&hello()).unwrap(), 4);
    session.next_outbound();
    assert_eq!(session.next_deadline(), None);
    session.enqueue_request(1, screens(1), 20).unwrap();
    session.next_outbound();
    session.enqueue_request(2, screens(2), 10).unwrap();
    assert_eq!(
        session.next_deadline(),
        Some(10 + REQUEST_TIMEOUT_MILLISECONDS)
    );
    session.receive(Message::Acknowledged { request_id: 2 }, 11);
    assert_eq!(
        session.next_deadline(),
        Some(20 + REQUEST_TIMEOUT_MILLISECONDS)
    );
}
