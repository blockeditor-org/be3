use std::{cell::RefCell, rc::Rc};

use super::*;

#[test]
fn a_guest_stream_tells_a_finished_end_from_a_canceled_one() {
    let ends = Rc::new(RefCell::new(Vec::new()));
    let kept = Rc::clone(&ends);
    let executor = guest::Executor::new(Box::new(move |_, _| {
        for _ in 0..2 {
            let ends = Rc::clone(&kept);
            guest::spawn(async move {
                let mut items = guest::stream::<u32, wire::Plain, _>(method_id("test#items"), &());
                let mut taken = Vec::new();
                while let Some(item) = items.recv().await {
                    taken.push(item);
                }
                ends.borrow_mut().push((taken, items.failed()));
            });
        }
        guest::Serve::Done(Outcome::returned(&()))
    }));
    let mut host = Connection::new(Side::Host);
    host.start(None, method_id("test#run"), encode(&()));
    let mut outbound = Vec::new();
    executor.step(host.take_outbound(), &mut outbound).unwrap();
    for frame in outbound.drain(..) {
        host.receive(&frame).unwrap();
    }
    let streams: Vec<OpId> = events(&mut host)
        .into_iter()
        .filter_map(|event| match event {
            Incoming::Started(started) if started.method == method_id("test#items") => {
                Some(started.op)
            }
            _ => None,
        })
        .collect();
    let [finished, canceled] = streams[..] else {
        panic!("the guest opened two streams, not {streams:?}");
    };
    let (finished, canceled) = (
        Producer::<u32>::new(finished),
        Producer::<u32>::new(canceled),
    );
    finished.send(&mut host, &1).unwrap();
    canceled.send(&mut host, &2).unwrap();
    finished.finish(&mut host);
    canceled.fail(&mut host, OpError::Canceled);
    executor.step(host.take_outbound(), &mut outbound).unwrap();

    let mut ends = ends.borrow().clone();
    ends.sort_by_key(|(taken, _)| taken.clone());
    assert_eq!(ends, [(vec![1], None), (vec![2], Some(OpError::Canceled))]);
}
