use super::*;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn a_thread_that_trapped_wakes_the_host_to_be_told() {
    let source = threaded_guest("(drop (call $create_buffer (i32.const 0) (i32.const 0)))");
    let mut plugin = host().load_bytes(source.as_bytes()).unwrap();
    let told = Arc::new(AtomicBool::new(false));
    plugin.on_wake({
        let told = Arc::clone(&told);
        move || told.store(true, Ordering::Release)
    });

    let mut failure = plugin.start().err();
    for _ in 0..500 {
        if failure.is_some() && told.load(Ordering::Acquire) {
            break;
        }
        if failure.is_none() {
            failure = plugin.step().err();
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    let failure = failure.expect("the host should be told what stopped the thread");
    assert!(failure.contains("create_buffer"), "{failure}");
    assert!(
        told.load(Ordering::Acquire),
        "a thread trapped and nothing woke the host to be told"
    );
}
