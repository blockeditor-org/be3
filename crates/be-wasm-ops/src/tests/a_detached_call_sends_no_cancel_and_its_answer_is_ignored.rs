use super::*;

#[test]
fn a_detached_call_sends_no_cancel_and_its_answer_is_ignored() {
    let executor = guest::Executor::new(Box::new(|_, _| {
        guest::Serve::Done(Outcome::Err(OpError::Unsupported))
    }));
    executor.enter(|| guest::call::<(), wire::Plain, _>(method_id("test#note"), &()).detach());
    let mut host = Connection::new(Side::Host);
    let mut outbound = Vec::new();
    executor.step(Vec::new(), &mut outbound).unwrap();
    for frame in outbound.drain(..) {
        host.receive(&frame).unwrap();
    }
    let received = events(&mut host);
    let [Incoming::Started(noted)] = received.as_slice() else {
        panic!("the host heard the call and nothing else, got {received:?}");
    };
    host.complete(noted.op, Outcome::returned(&()));
    executor.step(host.take_outbound(), &mut outbound).unwrap();
    assert!(
        outbound.is_empty(),
        "the guest neither canceled the call nor answered its completion"
    );
}
