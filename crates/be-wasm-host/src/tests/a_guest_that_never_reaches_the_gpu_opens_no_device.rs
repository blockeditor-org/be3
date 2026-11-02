use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

#[test]
fn a_guest_that_never_reaches_the_gpu_opens_no_device() {
    static OPENED: AtomicBool = AtomicBool::new(false);
    let host = Host::on_demand(
        || {
            OPENED.store(true, Ordering::SeqCst);
            Err("the test has no device".to_owned())
        },
        None,
    )
    .unwrap();
    let source = guest("(call $host_send (i32.const 0) (i32.const 1))");
    let mut plugin = host.load_bytes(source.as_bytes()).unwrap();
    plugin.start().unwrap();
    plugin.step().unwrap();

    assert_eq!(plugin.take_outbound().len(), 1);
    assert!(!OPENED.load(Ordering::SeqCst));
}
