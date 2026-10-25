use super::*;

fn yielded(received: &[Incoming]) -> Vec<u32> {
    received
        .iter()
        .filter_map(|event| match event {
            Incoming::Yielded { item, .. } => decode(item),
            _ => None,
        })
        .collect()
}

#[test]
fn a_stream_runs_no_further_ahead_than_its_consumer_grants() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let op = guest.start(None, method_id("test#items"), encode(&()));
    deliver(&mut guest, &mut host);
    events(&mut host);
    let producer = Producer::<u32>::new(op);
    let sent = STREAM_WINDOW + 3;
    for item in 0..sent {
        producer.send(&mut host, &item);
    }
    assert_eq!(producer.waiting(&host), 3);
    producer.finish(&mut host);
    deliver(&mut host, &mut guest);
    let received = events(&mut guest);
    assert_eq!(yielded(&received), (0..STREAM_WINDOW).collect::<Vec<_>>());
    assert!(guest.is_pending(op), "the end waits behind the held items");
    guest.credit(op, 2);
    deliver(&mut guest, &mut host);
    deliver(&mut host, &mut guest);
    assert_eq!(
        yielded(&events(&mut guest)),
        vec![STREAM_WINDOW, STREAM_WINDOW + 1]
    );
    guest.credit(op, 5);
    deliver(&mut guest, &mut host);
    deliver(&mut host, &mut guest);
    let received = events(&mut guest);
    assert_eq!(yielded(&received), vec![STREAM_WINDOW + 2]);
    assert!(matches!(received.last(), Some(Incoming::Completed { op: ended, .. }) if *ended == op));
}
