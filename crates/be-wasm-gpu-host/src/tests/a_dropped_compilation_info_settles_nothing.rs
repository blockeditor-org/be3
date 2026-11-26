use be_wasm_ops::local::Promise;

use super::web::*;
use super::*;

#[test]
fn a_dropped_compilation_info_settles_nothing() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let module = module(&mut gpu, device, "@compute @workgroup_size(1) fn main() {}");
    gpu.gpu_shader_module_get_compilation_info(module, Promise::new(4));
    gpu.gpu_shader_module_get_compilation_info(module, Promise::new(5));
    let mut settled: Vec<be_wasm_ops::local::Settled> =
        be_wasm_gpu_abi::embed::webgpu::Host::settle(&mut gpu, false, &[4]);
    settled.extend(be_wasm_gpu_abi::embed::webgpu::Host::settle(
        &mut gpu,
        true,
        &[],
    ));
    let tickets: Vec<u32> = settled.iter().map(|settled| settled.ticket).collect();
    assert_eq!(tickets, vec![5], "only the info still waited for settles");
}
