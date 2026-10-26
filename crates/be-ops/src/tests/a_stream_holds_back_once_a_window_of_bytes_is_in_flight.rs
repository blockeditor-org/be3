use super::*;

#[test]
fn a_stream_holds_back_once_a_window_of_bytes_is_in_flight() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let op = guest.start(None, method_id("test#items"), encode(&()));
    deliver(&mut guest, &mut host);
    events(&mut host);
    let producer = Producer::<Vec<u8>>::new(op);
    let large = vec![7; STREAM_WINDOW_BYTES as usize / 3 + 1];
    for _ in 0..3 {
        producer.send(&mut host, &large).unwrap();
    }
    assert_eq!(
        producer.send(&mut host, &Vec::new()),
        Err(WouldBlock),
        "three items fill the window's bytes long before its items"
    );
    let oversized = vec![1; STREAM_WINDOW_BYTES as usize * 2];
    guest.credit(op, 3, 3 * encode(&large).len() as u64);
    deliver(&mut guest, &mut host);
    assert_eq!(events(&mut host), [Incoming::Credited { op }]);
    producer
        .send(&mut host, &oversized)
        .expect("an item larger than the window still goes once the window is open");
    assert!(!producer.ready(&host));
}
