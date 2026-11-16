use super::*;

#[test]
fn a_page_may_not_wait_and_a_worker_may() {
    thread::spawn(|| {
        initialize_page();
        assert!(!may_wait(), "a page may not wait");
    })
    .join()
    .expect("the page's thread finishes");
    thread::spawn(|| {
        initialize_worker();
        assert!(may_wait(), "a worker may wait");
    })
    .join()
    .expect("the worker's thread finishes");
}
