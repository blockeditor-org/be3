use super::*;

#[test]
fn a_queued_outlet_holds_back_what_it_cannot_send_and_fails_when_full() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let op = guest.start(None, method_id("test#numbers"), encode(&()));
    deliver(&mut guest, &mut host);
    events(&mut host);
    let mut numbers = Outlets::default();
    numbers.insert(Producer::<u32>::new(op), "numbers", Backlog::Queue(2));
    for number in 0..STREAM_WINDOW + 2 {
        numbers.send(&mut host, &number, |_| true).unwrap();
    }
    assert!(
        numbers.send(&mut host, &0, |_| true).is_err(),
        "a full queue refuses more"
    );
    deliver(&mut host, &mut guest);
    assert_eq!(events(&mut guest).len(), STREAM_WINDOW as usize);
    guest.credit(op, STREAM_WINDOW, 0);
    deliver(&mut guest, &mut host);
    events(&mut host);
    assert_eq!(numbers.end(&mut host, op, None), Some("numbers"));
    deliver(&mut host, &mut guest);
    assert_eq!(
        events(&mut guest),
        [
            Incoming::Yielded {
                op,
                item: encode(&STREAM_WINDOW)
            },
            Incoming::Yielded {
                op,
                item: encode(&(STREAM_WINDOW + 1))
            },
            Incoming::Completed {
                op,
                outcome: Outcome::returned(&())
            },
        ],
        "ending flushes what was held back, in order, before it finishes"
    );
}
