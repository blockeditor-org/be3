use std::sync::Arc;

use be_wasm_pure_wasi::vectors;

use super::*;

/// What the shared guest (pure_wasi.wat) sends from its start, and the random bytes it draws for
/// `draws`, on a host seeded with `seed`; then why it stopped once told to exit with 7.
fn run(seed: [u8; 32], draws: &[u8]) -> (Vec<u8>, Vec<u8>, String) {
    let waiting = Arc::default();
    let channel = Host::pure()
        .unwrap()
        .with_seed(seed)
        .spawn(
            "pure wasi".to_owned(),
            |host| host.load_bytes(include_str!("pure_wasi.wat").as_bytes()),
            Arc::new(|| {}),
            Arc::clone(&waiting),
            be_wasm_guest_host::thread::catch,
        )
        .unwrap();
    let mut worker = be_wasm_guest_host::Worker::new(channel, waiting);
    let frames: Vec<Vec<u8>> = draws.iter().map(|length| vec![0, *length]).collect();
    assert_eq!(worker.queue(frames), None);
    let wanted = 4 + draws.iter().map(|length| *length as usize).sum::<usize>();
    let mut received: Vec<Vec<u8>> = Vec::new();
    while received.iter().map(Vec::len).sum::<usize>() < wanted {
        let deadline = be_wasm_guest_host::Deadline::after(std::time::Duration::from_secs(60));
        assert!(
            worker.wait(deadline).expect("the guest runs"),
            "the guest went quiet"
        );
        received.extend(worker.take_received());
    }
    let started = received.remove(0);
    let drawn = received.concat();
    assert_eq!(worker.queue(vec![vec![1, 7]]), None);
    let stopped = loop {
        let deadline = be_wasm_guest_host::Deadline::after(std::time::Duration::from_secs(60));
        match worker.wait(deadline) {
            Err(stopped) => break stopped,
            Ok(woke) => assert!(woke, "the guest never exited"),
        }
    };
    (started, drawn, stopped)
}

#[test]
fn a_pure_guests_randomness_is_chacha20s_stream_from_its_seed() {
    // Its environment is empty, it has no clock, and its line of 24 bytes reached the log.
    let (started, drawn, stopped) = run([0; 32], &[10, 54, 60, 8, 60]);
    assert_eq!(started, [0, 52, 0, 24]);
    assert_eq!(vectors::hex(&drawn[..64]), vectors::ZERO_BLOCK_0);
    assert_eq!(vectors::hex(&drawn[64..128]), vectors::ZERO_BLOCK_1);
    assert!(stopped.contains("exited with status 7"), "{stopped}");
    let (_, drawn, _) = run(vectors::ONE, &[64, 64]);
    assert_eq!(vectors::hex(&drawn[64..]), vectors::ONE_BLOCK_1);
}
