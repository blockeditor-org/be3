use be_wasm_ops::local::{Promise, Sender};

use super::web::*;
use super::*;

#[test]
fn a_dropped_lost_promise_and_error_stream_hear_nothing_of_the_loss() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    gpu.gpu_device_lost(device, Promise::new(4));
    gpu.gpu_device_on_uncaptured_error(device, Sender::new(5));
    gpu.gpu_device_lost(device, Promise::new(6));
    let settled: Vec<be_wasm_ops::local::Settled> = be_wasm_gpu_abi::embed::webgpu::Host::settle(&mut gpu, false, &[4, 5]);
    assert!(settled.is_empty(), "nothing is lost yet: {settled:?}");
    gpu.gpu_device_destroy(device);
    let settled: Vec<be_wasm_ops::local::Settled> = be_wasm_gpu_abi::embed::webgpu::Host::settle(&mut gpu, true, &[]);
    let tickets: Vec<u32> = settled.iter().map(|settled| settled.ticket).collect();
    assert_eq!(
        tickets,
        vec![6],
        "only the lost promise still held hears the loss; the dropped stream gets no end"
    );
}
