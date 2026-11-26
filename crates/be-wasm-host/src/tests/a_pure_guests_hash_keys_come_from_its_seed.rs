use std::sync::Arc;

use super::*;

/// The echo guest's hash of 0 under a fresh `RandomState` (`be_wasm_guest_echo::HASH`), on a host
/// seeded with `seed`.
fn hash(seed: [u8; 32]) -> Vec<u8> {
    let waiting = Arc::default();
    let channel = Host::pure()
        .unwrap()
        .with_seed(seed)
        .spawn(
            "echo".to_owned(),
            |host| host.load_bytes(ECHO),
            Arc::new(|| {}),
            Arc::clone(&waiting),
        )
        .unwrap();
    let mut worker = be_wasm_guest_host::Worker::new(channel, waiting);
    assert_eq!(worker.queue(vec![be_wasm_guest_echo::HASH.to_vec()]), None);
    loop {
        let deadline = be_wasm_guest_host::Deadline::after(std::time::Duration::from_secs(60));
        assert!(
            worker.wait(deadline).expect("the guest runs"),
            "the guest went quiet"
        );
        if let Some(hash) = worker
            .take_received()
            .into_iter()
            .find(|frame| frame.len() == 8)
        {
            return hash;
        }
    }
}

#[test]
fn a_pure_guests_hash_keys_come_from_its_seed() {
    // std on wasip1 draws its hash keys with random_get: the same seed, the same keys.
    assert_eq!(hash([7; 32]), hash([7; 32]));
    assert_ne!(hash([7; 32]), hash([8; 32]));
}
