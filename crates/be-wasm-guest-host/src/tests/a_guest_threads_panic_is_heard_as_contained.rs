use std::{
    convert::Infallible,
    sync::{Mutex, PoisonError, atomic::AtomicBool},
    time::Duration,
};

use crate::thread::{Runnable, Spawn};

use super::*;

/// Each panic a hook heard: its message, and whether it was contained.
static HEARD: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());

/// A guest the host runs on a thread of its own, as a type module or a sandbox's worker is,
/// that panics in the host's code while it loads.
struct Panics;

impl Runnable for Panics {
    type Presented = Infallible;

    fn start(&mut self) -> Result<Produced<Infallible>, String> {
        unreachable!("it never loads")
    }

    fn step(&mut self, _: Vec<Vec<u8>>) -> Result<Produced<Infallible>, String> {
        unreachable!("it never loads")
    }

    fn take_wake(&mut self) -> bool {
        false
    }

    fn on_wake(&self, _: Wake) {}

    fn stop(&mut self) {}
}

/// A host's panic hook (be-host's crashes the host on a panic nothing contained) hears a panic
/// on a guest's thread as contained: it is that guest's failure, reported as one.
#[test]
fn a_guest_threads_panic_is_heard_as_contained() {
    let previous = std::panic::take_hook();
    be_wasm_panic::set_hook(Box::new(move |info| {
        let message = be_wasm_panic::message(info.payload());
        HEARD
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((message, be_wasm_panic::containing()));
        previous(info);
    }));

    let mut guest = crate::thread::spawn(
        Spawn {
            name: "a type module".to_owned(),
            wake: Arc::new(|| {}),
            waiting: Arc::new(AtomicBool::new(false)),
            interrupt: Box::new(|| {}),
        },
        || -> Result<Panics, String> { panic!("a guest thread's host bug") },
    )
    .unwrap();

    let failure = loop {
        match guest.wait(Duration::from_secs(60)) {
            Some(Received::Event(Event::Failed(failure))) => break failure,
            Some(Received::Empty) => continue,
            _ => panic!("the guest reports its failure before its thread ends"),
        }
    };
    assert!(failure.contains("a guest thread's host bug"), "{failure}");
    let heard: Vec<bool> = HEARD
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|(message, _)| message == "a guest thread's host bug")
        .map(|(_, contained)| *contained)
        .collect();
    assert_eq!(heard, vec![true], "the hook heard it once, as contained");
}
