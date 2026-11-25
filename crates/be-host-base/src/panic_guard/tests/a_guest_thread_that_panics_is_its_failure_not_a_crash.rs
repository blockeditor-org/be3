use std::{
    convert::Infallible,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use be_wasm_guest_host::{
    Channel, Event, Produced, Received, Wake,
    thread::{Runnable, Spawn},
};

use super::*;

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

#[test]
fn a_guest_thread_that_panics_is_its_failure_not_a_crash() {
    install();

    let mut guest = be_wasm_guest_host::thread::spawn(
        Spawn {
            name: "a type module".to_owned(),
            wake: Arc::new(|| {}),
            waiting: Arc::new(AtomicBool::new(false)),
            interrupt: Box::new(|| {}),
        },
        || -> Result<Panics, String> { panic!("a type module's host bug") },
    )
    .unwrap();

    let failure = loop {
        match guest.wait(Duration::from_secs(60)) {
            Some(Received::Event(Event::Failed(failure))) => break failure,
            Some(Received::Empty) => continue,
            _ => panic!("the guest reports its failure before its thread ends"),
        }
    };
    assert!(failure.contains("a type module's host bug"), "{failure}");
    assert!(
        take().is_none_or(|report| !report.contains("a type module's host bug")),
        "the guest's panic was reported as its failure, so it does not crash the host"
    );
}
