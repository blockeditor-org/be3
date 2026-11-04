use std::{cell::RefCell, rc::Rc};

use super::*;

#[test]
fn a_guest_task_awaits_an_import_and_dropping_it_cancels() {
    let heard = Rc::new(RefCell::new(Vec::new()));
    let kept = Rc::clone(&heard);
    let executor = guest::Executor::new(Box::new(move |_, started| {
        let heard = Rc::clone(&kept);
        assert_eq!(started.method, method_id("test#run"));
        guest::spawn(async move {
            let answer = guest::call::<u32, wire::Plain, _>(method_id("test#ask"), &()).await;
            heard.borrow_mut().push(answer);
            drop(guest::call::<u32, wire::Plain, _>(
                method_id("test#ask"),
                &(),
            ));
        });
        guest::Serve::Done(Outcome::returned(&()))
    }));
    let mut host = Connection::new(Side::Host);
    let run = host.start(None, method_id("test#run"), encode(&()));
    let mut outbound = Vec::new();
    executor.step(host.take_outbound(), &mut outbound).unwrap();
    for frame in outbound.drain(..) {
        host.receive(&frame).unwrap();
    }
    let received = events(&mut host);
    assert_eq!(received.len(), 2);
    let Incoming::Started(asked) = &received[1] else {
        panic!("the guest asked the host, got {received:?}");
    };
    assert!(matches!(received[0], Incoming::Completed { op, .. } if op == run));
    host.complete(asked.op, Outcome::returned(&5u32));
    executor.step(host.take_outbound(), &mut outbound).unwrap();
    assert_eq!(*heard.borrow(), vec![Ok(5)]);
    for frame in outbound.drain(..) {
        host.receive(&frame).unwrap();
    }
    let received = events(&mut host);
    let [Incoming::Started(again), Incoming::Canceled { op }] = received.as_slice() else {
        panic!("the guest asked again and gave up, got {received:?}");
    };
    assert_eq!(again.op, *op);
}
