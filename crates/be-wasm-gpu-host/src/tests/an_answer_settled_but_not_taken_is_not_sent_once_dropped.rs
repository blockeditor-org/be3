use be_wasm_ops::local::Promise;

use super::web::*;
use super::*;

#[test]
fn an_answer_settled_but_not_taken_is_not_sent_once_dropped() {
    let mut gpu = gpu();
    let instance = gpu.get_gpu();
    // An adapter is answered at once, and waits for the next poll to be taken.
    gpu.gpu_request_adapter(instance, None, Promise::new(4));
    gpu.gpu_request_adapter(instance, None, Promise::new(5));
    let settled: Vec<be_wasm_ops::local::Settled> =
        be_wasm_gpu_abi::embed::webgpu::Host::settle(&mut gpu, false, &[4]);
    let tickets: Vec<u32> = settled.iter().map(|settled| settled.ticket).collect();
    assert_eq!(tickets, vec![5]);
}
