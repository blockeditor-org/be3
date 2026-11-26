use super::web::*;
use super::*;

#[test]
fn submitted_work_wakes_the_app_once_the_gpu_has_done_it() {
    let (mut gpu, wakes) = waking();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let queue = gpu.gpu_device_queue(device);
    gpu.gpu_queue_submit(queue, &[]);
    gpu.gpu_queue_on_submitted_work_done(queue, be_wasm_ops::local::Promise::new(6));
    wakes
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the done work woke the app");
    let () = web::settled_now(&mut gpu, 6);
    assert!(gpu.settle(false, &[]).is_empty(), "each ticket is answered once");
}
