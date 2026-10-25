use super::*;

#[test]
fn an_in_process_guest_echoes_and_wakes() {
    let (mut worker, wakes) = echo();
    assert_eq!(
        worker.queue(vec![b"hello".to_vec(), block_guest_echo::WAKE.to_vec()]),
        None
    );
    let received = polled(&mut worker, 4).unwrap();
    assert_eq!(
        received,
        vec![
            block_guest_echo::STARTED.to_vec(),
            b"hello".to_vec(),
            block_guest_echo::WOKEN.to_vec(),
        ]
    );
    assert!(
        wakes.load(Ordering::SeqCst) > 0,
        "the guest's wake reached the host"
    );
    assert!(worker.settled());
}
