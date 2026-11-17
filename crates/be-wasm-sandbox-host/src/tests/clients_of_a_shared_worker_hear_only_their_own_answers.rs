use std::{
    future::poll_fn,
    task::{Poll, Waker},
};

use be_sdk_worker::Shared;
use be_sdk_worker_echo::{ECHOED, MODULE};

use super::*;

/// A client of the shared echo worker, and the task that waits on it.
fn client() -> (Shared, Rc<RefCell<Option<Waker>>>) {
    let waiting = Rc::new(RefCell::new(None::<Waker>));
    let woken = Rc::clone(&waiting);
    let shared = Shared::new(MODULE, move || {
        if let Some(waker) = woken.borrow_mut().take() {
            waker.wake();
        }
    });
    (shared, waiting)
}

async fn answer(shared: &Shared, waiting: &RefCell<Option<Waker>>) -> Result<Vec<u8>, String> {
    poll_fn(|context| match shared.try_receive() {
        Some(answer) => Poll::Ready(answer.result),
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
        let generations = [
            second.send(b"two"),
            first.send(b"one"),
            first.send(b"three"),
        ];
        assert!(
            generations.iter().all(|generation| *generation == generations[0]),
            "every client talks to one instance"
        );
        [
            answer(&first, &first_waits).await,
            answer(&second, &second_waits).await,
            answer(&first, &first_waits).await,
        ]
    });
    assert_eq!(
        answers,
        [
            Ok([ECHOED, b"one"].concat()),
            Ok([ECHOED, b"two"].concat()),
            Ok([ECHOED, b"three"].concat()),
        ]
    );
}
