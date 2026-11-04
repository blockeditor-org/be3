use super::*;

#[test]
fn outlets_hold_back_no_more_bytes_than_their_limit() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let first = guest.start(None, method_id("test#items"), encode(&()));
    let second = guest.start(None, method_id("test#items"), encode(&()));
    deliver(&mut guest, &mut host);
    events(&mut host);
    let item = vec![7u8; STREAM_WINDOW_BYTES as usize];
    let held = encode(&item).len();
    let mut outlets = Outlets::within(3 * held);
    outlets.insert(Producer::<Vec<u8>>::new(first), 1, Backlog::Queue(64));
    outlets.insert(Producer::<Vec<u8>>::new(second), 2, Backlog::Queue(64));
    outlets
        .send(&mut host, &item, |_| true)
        .expect("each outlet's window takes the first item");
    outlets
        .send(&mut host, &item, |route| *route == 1)
        .expect("one item is held back");
    outlets
        .send(&mut host, &item, |route| *route == 2)
        .expect("a second, on the other outlet");
    outlets
        .send(&mut host, &item, |route| *route == 2)
        .expect("a third reaches the limit");
    assert_eq!(outlets.behind_bytes(), 3 * held);
    assert_eq!(
        outlets.send(&mut host, &item, |route| *route == 1),
        Err(WouldBlock),
        "the limit is shared by every outlet, though each queue has room"
    );
    deliver(&mut host, &mut guest);
    events(&mut guest);
    guest.credit(first, 1, held as u64);
    deliver(&mut guest, &mut host);
    for event in events(&mut host) {
        if let Incoming::Credited { op } = event {
            outlets.credited(&mut host, op);
        }
    }
    assert_eq!(
        outlets.behind_bytes(),
        2 * held,
        "an item sent on credit no longer counts"
    );
}
