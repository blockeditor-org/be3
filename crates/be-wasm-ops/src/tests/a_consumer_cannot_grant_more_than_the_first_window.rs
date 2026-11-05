use super::*;

#[test]
fn a_consumer_cannot_grant_more_than_the_first_window() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let op = guest.start(None, method_id("test#numbers"), encode(&()));
    deliver(&mut guest, &mut host);
    events(&mut host);
    let producer = Producer::<u32>::new(op);
    guest.credit(op, u32::MAX, u64::MAX);
    deliver(&mut guest, &mut host);
    for number in 0..STREAM_WINDOW {
        producer.send(&mut host, &number).unwrap();
    }
    assert_eq!(
        producer.send(&mut host, &0),
        Err(WouldBlock),
        "credit beyond what was taken does not widen the window"
    );
}
