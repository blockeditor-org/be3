use std::{cell::RefCell, rc::Rc};

use super::*;

#[test]
fn a_guest_stream_grants_credit_as_it_takes_items() {
    let taken = Rc::new(RefCell::new(Vec::new()));
    let kept = Rc::clone(&taken);
    let executor = guest::Executor::new(Box::new(move |_, _| {
        let taken = Rc::clone(&kept);
        guest::spawn(async move {
            let mut items = guest::stream::<u32, wire::Plain, _>(method_id("test#items"), &());
            while let Some(item) = items.recv().await {
                taken.borrow_mut().push(item);
            }
        });
        guest::Serve::Done(Outcome::returned(&()))
    }));
    let mut host = Connection::new(Side::Host);
    host.start(None, method_id("test#run"), encode(&()));
    let mut outbound = Vec::new();
    executor.step(host.take_outbound(), &mut outbound).unwrap();
    for frame in outbound.drain(..) {
        host.receive(&frame).unwrap();
    }
    let Some(Incoming::Started(started)) = events(&mut host)
        .into_iter()
        .find(|event| matches!(event, Incoming::Started(_)))
    else {
        panic!("the guest opened no stream");
    };
    let producer = Producer::<u32>::new(started.op);
    let sent = STREAM_WINDOW * 3;
    let mut next = 0;
    let mut credited = 0;
    loop {
        while next < sent && producer.send(&mut host, &next).is_ok() {
            next += 1;
        }
        if next == sent {
            producer.finish(&mut host);
            break;
        }
        executor.step(host.take_outbound(), &mut outbound).unwrap();
        for frame in outbound.drain(..) {
            host.receive(&frame).unwrap();
        }
        let events = events(&mut host);
        assert_eq!(events, [Incoming::Credited { op: started.op }]);
        credited += 1;
    }
    executor.step(host.take_outbound(), &mut outbound).unwrap();
    assert_eq!(*taken.borrow(), (0..sent).collect::<Vec<_>>());
    assert!(!host.is_serving(started.op));
    assert!(credited >= 2, "the producer waited for credit");
}
