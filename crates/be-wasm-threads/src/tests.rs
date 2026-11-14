use std::sync::{Arc, mpsc};
use std::thread;

use super::{Mutex, initialize_page, initialize_worker, may_wait};

#[test]
fn a_thread_off_the_page_waits_for_a_held_lock() {
    let lock = Arc::new(Mutex::new(0));
    let mut held = lock.lock().expect("a fresh lock");
    let (asking, asked) = mpsc::channel();
    let waiter = thread::spawn({
        let lock = Arc::clone(&lock);
        move || {
            initialize_worker();
            assert!(may_wait(), "a worker may wait");
            asking.send(()).expect("the test listens");
            *lock.lock().expect("the holder does not panic")
        }
    });
    asked.recv().expect("the waiter starts");
    *held = 1;
    drop(held);
    assert_eq!(
        waiter.join().expect("the waiter finishes"),
        1,
        "the waiter took the lock once it was let go, and saw what its holder wrote"
    );
}

#[test]
fn a_page_may_not_wait_and_takes_a_free_lock_at_once() {
    thread::spawn(|| {
        initialize_page();
        assert!(!may_wait(), "a page may not wait");
        let lock = Mutex::new(7);
        assert_eq!(*lock.lock().expect("a fresh lock"), 7);
    })
    .join()
    .expect("the page's thread finishes");
}
