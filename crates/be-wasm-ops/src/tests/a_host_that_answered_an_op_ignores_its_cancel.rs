use super::*;

#[test]
fn a_host_that_answered_an_op_ignores_its_cancel() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let op = guest.start(None, method_id("test#note"), encode(&()));
    deliver(&mut guest, &mut host);
    assert!(matches!(
        events(&mut host).as_slice(),
        [Incoming::Started(_)]
    ));
    host.complete(op, Outcome::returned(&()));
    guest.cancel(op);
    deliver(&mut guest, &mut host);
    assert_eq!(
        events(&mut host),
        vec![],
        "the cancel came after the answer"
    );
    deliver(&mut host, &mut guest);
    assert_eq!(
        events(&mut guest),
        vec![Incoming::Completed {
            op,
            outcome: Outcome::returned(&()),
        }]
    );
}
