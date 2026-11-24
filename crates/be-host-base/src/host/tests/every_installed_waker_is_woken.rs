use std::sync::mpsc;

use super::*;

/// Each host, or test, in a process installs its own waker: a later one does not take an
/// earlier one's place, and a wake reaches both.
#[test]
fn every_installed_waker_is_woken() {
    let (first, woke_first) = mpsc::channel();
    let (second, woke_second) = mpsc::channel();
    let first = Mutex::new(first);
    let second = Mutex::new(second);
    install_waker(move || {
        let _ = first
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(());
    });
    install_waker(move || {
        let _ = second
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(());
    });
    wake();
    assert_eq!(woke_first.try_recv(), Ok(()));
    assert_eq!(woke_second.try_recv(), Ok(()));
}
