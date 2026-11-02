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
    let mut sent = 0;
    while producer.send(&mut host, &sent).is_ok() {
        sent += 1;
    }
    assert_eq!(sent, STREAM_WINDOW, "the producer is told to stop");
    assert!(!producer.ready(&host));
    deliver(&mut host, &mut guest);
    assert_eq!(
        yielded(&events(&mut guest)),
        (0..STREAM_WINDOW).collect::<Vec<_>>()
    );
    guest.credit(op, 2, 0);
    deliver(&mut guest, &mut host);
    assert_eq!(events(&mut host), [Incoming::Credited { op }]);
    for item in [sent, sent + 1] {
        producer.send(&mut host, &item).unwrap();
    }
    assert_eq!(producer.send(&mut host, &(sent + 2)), Err(WouldBlock));
    producer.finish(&mut host);
    deliver(&mut host, &mut guest);
    let received = events(&mut guest);
    assert_eq!(yielded(&received), vec![sent, sent + 1]);
    assert!(matches!(received.last(), Some(Incoming::Completed { op: ended, .. }) if *ended == op));
}
