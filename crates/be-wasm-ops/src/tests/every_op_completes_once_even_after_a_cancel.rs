use super::*;

#[test]
fn every_op_completes_once_even_after_a_cancel() {
    let mut host = Connection::new(Side::Host);
    let mut guest = Connection::new(Side::Guest);
    let op = host.start(None, method_id("test#wait"), encode(&()));
    assert!(!Side::Guest.started(op));
    deliver(&mut host, &mut guest);
    assert!(matches!(
        events(&mut guest).as_slice(),
        [Incoming::Started(Started { op: started, .. })] if *started == op
    ));
    host.cancel(op);
    host.cancel(op);
    deliver(&mut host, &mut guest);
    assert_eq!(events(&mut guest), vec![Incoming::Canceled { op }]);
    guest.complete(op, Outcome::returned(&7u32));
    guest.complete(op, Outcome::Err(OpError::Canceled));
    deliver(&mut guest, &mut host);
    let completed = events(&mut host);
    assert_eq!(
        completed,
        vec![Incoming::Completed {
            op,
            outcome: Outcome::returned(&7u32),
        }]
    );
    assert!(!host.is_pending(op));
}
