use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use super::*;

fn ready<F: Future>(future: F) -> Option<F::Output> {
    let mut context = Context::from_waker(Waker::noop());
    match pin!(future).poll(&mut context) {
        Poll::Ready(value) => Some(value),
        Poll::Pending => None,
    }
}

#[test]
fn async_funcs_futures_and_streams_settle_through_the_poll() {
    let mut caller = Caller::new();
    let settling = Settling::default();

    let ready_ticket = embed::shapes::glue::ready(&mut caller);
    let grow_ticket = embed::shapes::glue::shape_grow(&mut caller, 3, 4);
    let lost_ticket = embed::shapes::glue::lost(&mut caller);
    let errors_ticket = embed::shapes::glue::errors(&mut caller);
    let tickets = [ready_ticket, grow_ticket, lost_ticket, errors_ticket];
    assert_eq!(
        tickets,
        [1, 2, 3, 4],
        "each call answers a ticket of its own"
    );

    let mut ready_value = settling.ticket::<(), be_wasm_ops::wire::Plain>("ready", ready_ticket);
    let mut grown = settling.ticket::<u32, be_wasm_ops::wire::Plain>("grow", grow_ticket);
    let mut lost = settling.ticket::<String, be_wasm_ops::wire::Plain>("lost", lost_ticket);
    let mut errors =
        settling.items::<shapes::shapes::Failed, be_wasm_ops::wire::Plain>("errors", errors_ticket);
    settle(&mut caller, &settling);
    assert_eq!(ready(&mut grown), None, "nothing has settled yet");
    assert!(settling.waiting());

    let host = &mut caller.host;
    let (area, promise) = host.grown.pop().unwrap();
    host.settled.push(promise.settle(&area));
    host.settled.push(host.ready.pop().unwrap().settle(&()));
    let sender = host.errors.pop().unwrap();
    let failed = |message: &str| shapes::shapes::Failed {
        message: message.to_owned(),
    };
    host.settled.push(sender.send(&failed("first")));
    host.settled.push(sender.send(&failed("second")));
    settle(&mut caller, &settling);

    assert_eq!(ready(&mut grown), Some(7));
    assert_eq!(ready_value.take(), Some(()));
    assert_eq!(errors.try_next(), Some(failed("first")));
    assert_eq!(ready(errors.next()), Some(Some(failed("second"))));
    assert_eq!(ready(errors.next()), None, "a stream not yet ended waits");
    assert_eq!(lost.take(), None);

    let host = &mut caller.host;
    host.settled.push(sender.end());
    host.settled
        .push(host.lost.pop().unwrap().settle(&"gone".to_owned()));
    settle(&mut caller, &settling);
    assert_eq!(
        ready(errors.next()),
        Some(None),
        "an ended stream answers none"
    );
    assert!(errors.ended());
    assert_eq!(ready(&mut lost), Some("gone".to_owned()));
    assert!(!settling.waiting());
}
