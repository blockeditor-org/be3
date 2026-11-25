use std::{
    future::poll_fn,
    task::{Poll, Waker},
};

use be_sdk_worker::{Shared, Ticket};
use be_sdk_worker_echo::{ECHOED, MODULE};

use super::*;

/// A client of the shared echo worker, and the task that waits on it.
#[allow(clippy::type_complexity)] // a test helper whose callers take the pair apart at once
fn client() -> (Shared<Vec<u8>, Vec<u8>>, Rc<RefCell<Option<Waker>>>) {
    let waiting = Rc::new(RefCell::new(None::<Waker>));
    let woken = Rc::clone(&waiting);
    let shared = Shared::new(MODULE, move || {
        if let Some(waker) = woken.borrow_mut().take() {
            waker.wake();
        }
    });
    (shared, waiting)
}

async fn answer(
    shared: &mut Shared<Vec<u8>, Vec<u8>>,
    waiting: &RefCell<Option<Waker>>,
) -> (Ticket, Result<Vec<u8>, String>) {
    poll_fn(|context| match shared.try_receive() {
        Some(answer) => Poll::Ready(answer),
        None => {
            *waiting.borrow_mut() = Some(context.waker().clone());
            Poll::Pending
        }
    })
    .await
}

/// Two clients of one worker module share its one instance, and each hears the answers to what it
/// sent and nothing else: not the other's, nor what the worker said as it started.
#[test]
fn clients_of_a_shared_worker_hear_only_their_own_answers() {
    let mut pump = Pump::echo_app();
    let answers = pump.run(async {
        let (mut first, first_waits) = client();
        let (mut second, second_waits) = client();
        let tickets = [
            second.send(&b"two".to_vec()),
            first.send(&b"one".to_vec()),
            first.send(&b"three".to_vec()),
        ];
        let answers = [
            answer(&mut first, &first_waits).await,
            answer(&mut second, &second_waits).await,
            answer(&mut first, &first_waits).await,
        ];
        (tickets, answers)
    });
    let ([two, one, three], answers) = answers;
    assert_eq!(
        answers,
        [
            (one, Ok([ECHOED, b"one"].concat())),
            (two, Ok([ECHOED, b"two"].concat())),
            (three, Ok([ECHOED, b"three"].concat())),
        ]
    );
}
