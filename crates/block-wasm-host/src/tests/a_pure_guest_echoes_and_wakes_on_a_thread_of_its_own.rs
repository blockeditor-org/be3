use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::*;

#[test]
fn a_pure_guest_echoes_and_wakes_on_a_thread_of_its_own() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let woken = Arc::clone(&wakes);
    let waiting = Arc::default();
    let channel = Host::pure(None)
        .unwrap()
        .spawn(
            "echo".to_owned(),
            |host| host.load_bytes(ECHO),
            Arc::new(move || {
                woken.fetch_add(1, Ordering::SeqCst);
            }),
            Arc::clone(&waiting),
        )
        .unwrap();
    let mut worker = block_guest_host::Worker::new(channel, waiting);
    assert_eq!(
        worker.queue(vec![b"hello".to_vec(), block_guest_echo::WAKE.to_vec()]),
        None
    );
    let received = echo_frames(
        &mut worker,
        &[block_guest_echo::STARTED, b"hello", block_guest_echo::WOKEN],
    )
    .unwrap();
    assert_eq!(
        received.first().map(Vec::as_slice),
        Some(block_guest_echo::STARTED),
        "what the guest sent as it started comes first"
    );
    assert!(
        wakes.load(Ordering::SeqCst) > 0,
        "the guest's wake reached the host"
    );
}
