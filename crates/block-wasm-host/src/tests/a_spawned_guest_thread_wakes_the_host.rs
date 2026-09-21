use super::*;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn a_spawned_guest_thread_wakes_the_host() {
    let source = threaded_guest("(call $host_wake)");
    let mut plugin = host().load_bytes(source.as_bytes()).unwrap();
    let told = Arc::new(AtomicBool::new(false));
    plugin.on_wake({
        let told = Arc::clone(&told);
        move || told.store(true, Ordering::Release)
    });

    plugin.start().unwrap();

    let mut woken = false;
    for _ in 0..500 {
        woken |= plugin.take_wake();
        if woken && told.load(Ordering::Acquire) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("a thread woke the plugin and the host never heard about it");
}
