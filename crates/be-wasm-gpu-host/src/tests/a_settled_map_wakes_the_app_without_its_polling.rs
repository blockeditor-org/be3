use std::sync::mpsc;
use std::time::Duration;

use super::*;

#[test]
fn a_settled_map_wakes_the_app_without_its_polling() {
    let (woken, wakes) = mpsc::channel();
    let woken = Mutex::new(woken);
    let mut gpu = gpu().waking(Arc::new(move || {
        let _ = woken.lock().unwrap().send(());
    }));
    let buffer = gpu
        .create_buffer(abi::BufferDescriptor {
            label: None,
            size: 8,
            usage: wgpu::BufferUsages::MAP_READ.bits() | wgpu::BufferUsages::COPY_DST.bits(),
            mapped_at_creation: false,
        })
        .unwrap();
    let ticket = gpu.buffer_map_async(buffer, abi::MapMode::Read, 0, 8);
    assert_eq!(gpu.take_error(), None);
    wakes
        .recv_timeout(Duration::from_secs(30))
        .expect("the settled map woke the app");
    assert_eq!(gpu.poll(false).maps, [(ticket, true)]);
    assert_eq!(gpu.buffer_read_mapped(buffer, 0, 8), [0; 8]);
    let again = gpu.buffer_map_async(buffer, abi::MapMode::Read, 0, 8);
    let error = gpu
        .take_error()
        .expect("a second map of a mapped buffer reports");
    assert!(error.contains("mapped"), "{error}");
    assert_eq!(
        gpu.poll(false).maps,
        [(again, false)],
        "a map refused at once still settles, as failed"
    );
}
