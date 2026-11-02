use std::sync::mpsc;
use std::time::Duration;

use super::*;

#[test]
fn a_settled_map_wakes_the_plugin_without_its_polling() {
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
    gpu.buffer_map_async(buffer, abi::MapMode::Read, 0, 8);
    assert_eq!(gpu.take_error(), None);
    wakes
        .recv_timeout(Duration::from_secs(30))
        .expect("the settled map woke the plugin");
    assert_eq!(gpu.buffer_map_state(buffer), abi::MapState::Mapped);
    assert_eq!(gpu.buffer_read_mapped(buffer, 0, 8), [0; 8]);
    gpu.buffer_map_async(buffer, abi::MapMode::Read, 0, 8);
    let error = gpu
        .take_error()
        .expect("a second map of a mapped buffer reports");
    assert!(error.contains("mapped"), "{error}");
}
