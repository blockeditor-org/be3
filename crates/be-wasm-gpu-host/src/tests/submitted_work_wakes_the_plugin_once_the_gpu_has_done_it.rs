use std::sync::mpsc;
use std::time::Duration;

use super::*;

#[test]
fn submitted_work_wakes_the_plugin_once_the_gpu_has_done_it() {
    let (woken, wakes) = mpsc::channel();
    let woken = Mutex::new(woken);
    let mut gpu = gpu().waking(Arc::new(move || {
        let _ = woken.lock().unwrap().send(());
    }));
    gpu.submit(&[]);
    let ticket = gpu.on_submitted_work_done();
    wakes
        .recv_timeout(Duration::from_secs(30))
        .expect("the done work woke the plugin");
    assert!(gpu.submitted_work_done(ticket));
    assert_eq!(gpu.take_error(), None);
    gpu.submitted_work_done(ticket);
    let error = gpu
        .take_error()
        .expect("a ticket that answered true is forgotten");
    assert!(error.contains("ticket"), "{error}");
}
