use be_wasm_ops::local::Promise;

use super::web::*;
use super::*;

#[test]
fn a_wrapped_webgpu_ticket_skips_one_still_held() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    // A device's `lost` waits for the app's whole life.
    gpu.gpu_device_lost(device, Promise::new(1));
    gpu.wind_tickets(u32::MAX - 1);
    assert_eq!(gpu.webgpu_ticket(), u32::MAX);
    assert_eq!(
        gpu.webgpu_ticket(),
        2,
        "past REFUSED and the ticket still held"
    );
    assert_eq!(gpu.webgpu_ticket(), 3);
}
