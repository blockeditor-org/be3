use std::{sync::mpsc, time::Duration};

use super::*;

#[test]
fn a_pure_guest_that_hangs_is_interrupted() {
    let host = Host::pure(None).unwrap();
    let interrupter = host.interrupter();
    let mut guest = host.load_bytes(ECHO).unwrap();
    let (woke, running) = mpsc::channel();
    guest.on_wake(move || {
        let _ = woke.send(());
    });
    guest.start().unwrap();
    guest.send(block_guest_echo::WAKE.to_vec());
    guest.send(block_guest_echo::HANG.to_vec());
    let stepping = std::thread::spawn(move || guest.step());
    running
        .recv_timeout(Duration::from_secs(60))
        .expect("the guest wakes the host just before it hangs");
    interrupter.interrupt();
    let stepped = stepping.join().unwrap();
    assert_eq!(stepped, Err(INTERRUPTED.to_owned()));
}
