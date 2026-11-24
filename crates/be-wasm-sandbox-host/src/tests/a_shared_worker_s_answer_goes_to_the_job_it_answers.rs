use std::{
    future::poll_fn,
    task::{Poll, Waker},
};

use be_sdk_worker::Shared;
use be_sdk_worker_echo::{ECHOED, MODULE, SILENT, TWICE};

use super::*;

/// A shared worker's answer goes to the job it answers, not to whichever was sent first: one it
/// does not answer takes no other's answer, and a second answer to a job is dropped.
#[test]
fn a_shared_worker_s_answer_goes_to_the_job_it_answers() {
    let mut pump = Pump::echo_app();
    let (tickets, answers, in_flight) = pump.run(async {
        let waiting = Rc::new(RefCell::new(None::<Waker>));
        let woken = Rc::clone(&waiting);
        let mut shared = Shared::<Vec<u8>, Vec<u8>>::new(MODULE, move || {
            if let Some(waker) = woken.borrow_mut().take() {
                waker.wake();
            }
        });
        let tickets = [
            shared.send(&SILENT.to_vec()),
            shared.send(&TWICE.to_vec()),
            shared.send(&b"after".to_vec()),
        ];
        let mut answers = Vec::new();
        while answers.len() < 2 {
            answers.push(
                poll_fn(|context| match shared.try_receive() {
                    Some(answer) => Poll::Ready(answer),
                    None => {
                        *waiting.borrow_mut() = Some(context.waker().clone());
                        Poll::Pending
                    }
                })
                .await,
            );
        }
        (tickets, answers, shared.in_flight())
    });
    let [silent, twice, after] = tickets;
    assert_eq!(
        answers,
        [
            (twice, Ok([ECHOED, TWICE].concat())),
            (after, Ok([ECHOED, b"after"].concat())),
        ]
    );
    assert_eq!(in_flight, 1, "the job it did not answer is still in flight: {silent:?}");
}
