use super::*;

#[test]
fn a_latest_outlet_sends_only_the_newest_value_once_it_has_room() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let op = guest.start(None, method_id("test#clock"), encode(&()));
    deliver(&mut guest, &mut host);
    events(&mut host);
    let mut clock = Outlets::default();
    clock.insert(Producer::<u64>::new(op), (), Backlog::Latest);
    let behind = u64::from(STREAM_WINDOW) + 5;
    for now in 0..behind {
        clock.send(&mut host, &now, |_| true).unwrap();
    }
    deliver(&mut host, &mut guest);
    assert_eq!(events(&mut guest).len(), STREAM_WINDOW as usize);
    guest.credit(op, STREAM_WINDOW, 0);
    deliver(&mut guest, &mut host);
    for event in events(&mut host) {
        let Incoming::Credited { op } = event else {
            panic!("only credit arrives");
        };
        assert!(clock.credited(&mut host, op));
    }
    deliver(&mut host, &mut guest);
    assert_eq!(
        events(&mut guest),
        [Incoming::Yielded {
            op,
            item: encode(&(behind - 1))
        }],
        "the times it could not send fold into the newest"
    );
}
