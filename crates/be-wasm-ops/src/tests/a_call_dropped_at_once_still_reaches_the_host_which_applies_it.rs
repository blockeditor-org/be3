use super::*;

#[test]
fn a_call_dropped_at_once_still_reaches_the_host_which_applies_it() {
    let executor = guest::Executor::new(Box::new(|_, _| {
        guest::Serve::Done(Outcome::Err(OpError::Unsupported))
    }));
    executor.enter(|| {
        drop(guest::call::<(), wire::Plain, _>(
            method_id("test#note"),
            &(),
        ))
    });
    let mut host = Connection::new(Side::Host);
    let mut outbound = Vec::new();
    executor.step(Vec::new(), &mut outbound).unwrap();
    for frame in outbound.drain(..) {
        host.receive(&frame).unwrap();
    }
    let received = events(&mut host);
    let [Incoming::Started(noted), Incoming::Canceled { op }] = received.as_slice() else {
        panic!("the host heard the call and then a cancel, got {received:?}");
    };
    assert_eq!(noted.op, *op);
    assert!(
        host.is_serving(noted.op),
        "the call took effect when it was made, so the host still answers it"
    );
    host.complete(noted.op, Outcome::returned(&()));
    executor.step(host.take_outbound(), &mut outbound).unwrap();
    assert!(outbound.is_empty(), "the guest ignores the late answer");
}
