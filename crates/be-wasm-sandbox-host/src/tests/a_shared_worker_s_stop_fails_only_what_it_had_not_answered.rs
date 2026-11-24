use std::{
    cell::Cell,
    future::poll_fn,
    task::{Poll, Waker},
};

use be_sdk_worker::Shared;
use be_sdk_worker_echo::{ECHOED, MODULE, STOP, STOPPED};

use super::*;

/// A shared worker that stops is started anew by the next `send`, which may come before the client
/// hears the stop, as the map's dispatch does: the stop fails only the ticket the stopped worker
/// had not answered, and the job sent since is answered by the worker started anew.
#[test]
fn a_shared_worker_s_stop_fails_only_what_it_had_not_answered() {
    let mut pump = Pump::echo_app();
    let (stopped, answered, sent) = pump.run(async {
        let woken = Rc::new(Cell::new(false));
        let waiting = Rc::new(RefCell::new(None::<Waker>));
        let (wakes, wakers) = (Rc::clone(&woken), Rc::clone(&waiting));
        let mut shared = Shared::<Vec<u8>, Vec<u8>>::new(MODULE, move || {
            wakes.set(true);
            if let Some(waker) = wakers.borrow_mut().take() {
                waker.wake();
            }
        });
        let first = shared.send(&STOP.to_vec());
        // Heard, and not yet taken.
        poll_fn(|context| match woken.get() {
            true => Poll::Ready(()),
            false => {
                *waiting.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
        })
        .await;
        let second = shared.send(&b"after".to_vec());
        let stopped = shared.try_receive();
        let answered = poll_fn(|context| match shared.try_receive() {
            Some(answer) => Poll::Ready(answer),
            None => {
                *waiting.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
        })
        .await;
        (stopped, answered, [first, second])
    });
    let [first, second] = sent;
    let Some((ticket, Err(why))) = stopped else {
        panic!("the stop was not the first answer: {stopped:?}");
    };
    assert_eq!(ticket, first);
    assert!(why.contains(STOPPED), "the stop says why: {why}");
    assert_eq!(answered, (second, Ok([ECHOED, b"after"].concat())));
}
