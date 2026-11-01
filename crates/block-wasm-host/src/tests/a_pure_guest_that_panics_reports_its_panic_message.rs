use std::sync::Arc;

use super::*;

#[test]
fn a_pure_guest_that_panics_reports_its_panic_message() {
    let waiting = Arc::default();
    let channel = Host::pure(None)
        .unwrap()
        .spawn(
            "echo".to_owned(),
            |host| host.load_bytes(ECHO),
            Arc::new(|| {}),
            Arc::clone(&waiting),
            block_guest_host::thread::catch,
        )
        .unwrap();
    let mut worker = block_guest_host::Worker::new(channel, waiting);
    assert_eq!(worker.queue(vec![block_guest_echo::PANIC.to_vec()]), None);
    let failure = echo_frames(&mut worker, &[b"never sent"]).unwrap_err();
    assert!(
        failure.contains(block_guest_echo::PANIC_MESSAGE),
        "the panic's message is reported: {failure}"
    );
}
