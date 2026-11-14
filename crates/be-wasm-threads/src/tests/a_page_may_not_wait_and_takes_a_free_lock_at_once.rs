use super::*;

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
