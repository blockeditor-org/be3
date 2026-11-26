//! A guest that drives wasi:webgpu's generated guest bindings (guides/type_modules.md, "Local
//! interfaces") against a real host: an async func and a future settled through `settle`, a
//! fallible call answering both ways, and options of lists passed flat. Its first step sends
//! `DONE`, or panics saying what went wrong.

use be_wasm_gpu_abi::guest::webgpu as gpu;
use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_guest_abi::{Flow, Guest, Waker};

pub const DONE: &[u8] = b"done";

/// Polls, waiting, until `ticket` has its value.
fn settled<T>(mut ticket: be_wasm_ops::local::Ticket<T>) -> T {
    for _ in 0..16 {
        gpu::settle(true);
        if let Some(value) = ticket.take() {
            return value;
        }
    }
    panic!("ticket {} never settled", ticket.ticket());
}

fn run() {
    let instance = gpu::get_gpu();
    // What settles is a handle's value, which its owner takes over.
    let adapter = settled(instance.request_adapter(&None)).expect("the host has an adapter");
    let adapter = gpu::GpuAdapter::from_handle(adapter);
    let device =
        settled(adapter.request_device(&None)).expect("a device with nothing asked for is given");
    let device = gpu::GpuDevice::from_handle(device);
    let queue = device.queue();
    let buffer = device.create_buffer(&web::GpuBufferDescriptor {
        size: 16,
        usage: web::GpuBufferUsage::COPY_DST,
        mapped_at_creation: None,
        label: Some("written".into()),
    });

    // A fallible call, both ways: the error comes back through `[returned]`.
    queue
        .write_buffer_with_copy(&buffer, 0, &[1, 2, 3, 4], None, None)
        .expect("four bytes are written");
    let refused = queue
        .write_buffer_with_copy(&buffer, 0, &[1, 2, 3, 4], Some(2), Some(8))
        .expect_err("eight bytes from the third of four are refused");
    assert_eq!(refused.kind, web::WriteBufferErrorKind::OperationError);
    assert!(!refused.message.is_empty());

    // Options of lists, flat: none, an empty list, and a range past its offsets.
    let encoder = device.create_command_encoder(&None);
    let pass = encoder.begin_compute_pass(&None);
    pass.set_bind_group(0, None, None, None, None)
        .expect("no offsets are none");
    pass.set_bind_group(0, None, Some(&[]), None, None)
        .expect("an empty list of offsets is none");
    let past = pass
        .set_bind_group(0, None, Some(&[256, 512]), Some(1), Some(4))
        .expect_err("four offsets from the second of two are refused");
    assert_eq!(past.kind, web::SetBindGroupErrorKind::RangeError);
    pass.end();
    let commands = encoder.finish(&None);
    queue.submit(&[&commands]);

    // A future of work done, and one of a device lost.
    settled(queue.on_submitted_work_done());
    let lost = device.lost();
    device.destroy();
    let info = settled(lost);
    let reason = gpu::GpuDeviceLostInfo::from_handle(info).reason();
    assert_eq!(reason, web::GpuDeviceLostReason::Destroyed);
    assert!(!gpu::waiting(), "nothing waits once all settled");
}

pub struct Probe;

impl Guest for Probe {
    fn start(_waker: Waker, _outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self)
    }

    fn step(
        &mut self,
        _inbound: Vec<Vec<u8>>,
        _woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        run();
        outbound.push(DONE.to_vec());
        Ok(Flow::Finished)
    }
}

be_wasm_guest_abi::guest!(Probe);
