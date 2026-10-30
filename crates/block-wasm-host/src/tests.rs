use super::*;

use block_gpu_abi::device as abi;

fn host() -> Host {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    Host::new(device, queue, None).unwrap()
}

fn escaped(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{byte:02x}")).collect()
}

fn buffer_descriptor() -> Vec<u8> {
    be_ops::encode(&abi::BufferDescriptor {
        label: Some("vertices".into()),
        size: 256,
        usage: wgpu::BufferUsages::VERTEX.bits(),
        mapped_at_creation: false,
    })
}

fn guest(body: &str) -> String {
    let descriptor = buffer_descriptor();
    format!(
        r#"(module
            (import "env" "memory" (memory 1 4 shared))
            (import "be3:gpu/device@0.1.0" "create-buffer" (func $create_buffer (param i32 i32) (result i32)))
            (import "be3:gpu/device@0.1.0" "take-error" (func $take_error (param i32 i32) (result i32)))
            (import "be3_host" "host_send" (func $host_send (param i32 i32)))
            (import "be3_host" "host_receive" (func $host_receive (param i32 i32) (result i64)))
            (data (i32.const 0) "{descriptor}")
            (global $length i32 (i32.const {length}))
            (func (export "plugin_start"))
            (func (export "plugin_shutdown"))
            (func (export "plugin_step") {body})
        )"#,
        descriptor = escaped(&descriptor),
        length = descriptor.len(),
    )
}

fn threaded_guest(body: &str) -> String {
    format!(
        r#"(module
            (import "env" "memory" (memory 1 4 shared))
            (import "wasi" "thread-spawn" (func $spawn (param i32) (result i32)))
            (import "be3:gpu/device@0.1.0" "create-buffer" (func $create_buffer (param i32 i32) (result i32)))
            (import "be3_host" "host_send" (func $host_send (param i32 i32)))
            (import "be3_host" "host_wake" (func $host_wake))
            (func (export "plugin_start") (drop (call $spawn (i32.const 0))))
            (func (export "plugin_shutdown"))
            (func (export "plugin_step")
                (call $host_send (i32.const 8) (i32.const 1)))
            (func (export "wasi_thread_start") (param i32 i32) {body})
        )"#
    )
}

fn sdk() -> &'static str {
    r#"(module
        (import "env" "memory" (memory 2 4 shared))
        (import "wasi" "thread-spawn" (func $spawn (param i32) (result i32)))
        (import "be3_host" "host_send" (func $host_send (param i32 i32)))
        (import "be3_host" "host_wake" (func $host_wake))
        (type $unary (func (param i32)))
        (table (export "__indirect_function_table") 2 funcref)
        (elem (i32.const 1) $double)
        (global $stack (export "__stack_pointer") (mut i32) (i32.const 4096))
        (global $tls (export "__tls_base") (mut i32) (i32.const 1024))
        (global (export "__tls_size") i32 (i32.const 32))
        (global (export "__tls_align") i32 (i32.const 4))
        (global (export "__be3_plugin_tls") i32 (i32.const 16))
        (global (export "counter") i32 (i32.const 2048))
        (global $heap (mut i32) (i32.const 8192))
        (data (i32.const 1024) "\64")
        (data (i32.const 1100) "\c8")
        (func (export "malloc") (param $size i32) (result i32)
            (global.get $heap)
            (global.set $heap (i32.add (global.get $heap) (local.get $size))))
        (func $double (export "double") (param i32) (result i32)
            (i32.mul (local.get 0) (i32.const 2)))
        (func (export "send") (param i32 i32) (call $host_send (local.get 0) (local.get 1)))
        (func (export "wake") (call $host_wake))
        (func (export "spawn") (param $function i32) (param $argument i32)
            (i32.store (i32.const 3000) (i32.const 12000))
            (i32.store (i32.const 3004) (i32.const 1100))
            (i32.store (i32.const 3008) (local.get $function))
            (i32.store (i32.const 3012) (local.get $argument))
            (drop (call $spawn (i32.const 3000))))
        (func (export "wasi_thread_start") (param $thread i32) (param $start i32)
            (global.set $stack (i32.load (local.get $start)))
            (global.set $tls (i32.load offset=4 (local.get $start)))
            (call_indirect (type $unary)
                (i32.load offset=12 (local.get $start))
                (i32.load offset=8 (local.get $start))))
    )"#
}

fn sdk_identity() -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(sdk().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn linked_plugin(identity: &str) -> String {
    r#"(module
        (import "env" "memory" (memory 1 4 shared))
        (import "env" "__indirect_function_table" (table 0 funcref))
        (import "env" "__stack_pointer" (global $stack (mut i32)))
        (import "env" "__memory_base" (global $memory_base i32))
        (import "env" "__table_base" (global $table_base i32))
        (import "env" "send" (func $send (param i32 i32)))
        (import "env" "spawn" (func $spawn (param i32 i32)))
        (import "env" "wake" (func $wake))
        (import "GOT.mem" "counter" (global $counter (mut i32)))
        (import "GOT.mem" "own_value" (global $own_value (mut i32)))
        (import "GOT.func" "double" (global $double (mut i32)))
        (import "GOT.func" "worker" (global $worker_slot (mut i32)))
        (type $doubling (func (param i32) (result i32)))
        (global $tls (export "__tls_base") (mut i32) (i32.const 0))
        (global (export "__tls_size") i32 (i32.const 20))
        (global (export "own_value") i32 (i32.const 64))
        (global (export "__be3_sdk_{IDENTITY}") i32 (i32.const 0))
        (global (export "__be3_dylink_256_2_1_0") i32 (i32.const 0))
        (elem (global.get $table_base) func $worker)
        (start $initialize)
        (func $initialize
            (global.set $tls (global.get $memory_base))
            (i32.store offset=16 (global.get $memory_base) (i32.const 7)))
        (func (export "__wasm_apply_data_relocs")
            (i32.store offset=64 (global.get $memory_base) (i32.const 5)))
        (func (export "plugin_initialize_tls") (param i32 i32))
        (func (export "plugin_start")
            (if (i32.ne (call_indirect (type $doubling) (i32.const 21) (global.get $double)) (i32.const 42))
                (then unreachable))
            (drop (i32.atomic.rmw.add (global.get $counter) (i32.const 1)))
            (if (i32.ne (i32.load (i32.const 2048)) (i32.const 1)) (then unreachable))
            (if (i32.ne (i32.load (global.get $own_value)) (i32.const 5)) (then unreachable))
            (if (i32.ne (global.get $tls) (i32.const 1024)) (then unreachable))
            (if (i32.ne (i32.load offset=16 (global.get $tls)) (i32.const 7)) (then unreachable))
            (if (i32.ne (i32.load (global.get $tls)) (i32.const 100)) (then unreachable))
            (call $spawn (global.get $worker_slot) (i32.add (global.get $memory_base) (i32.const 200))))
        (func (export "plugin_shutdown"))
        (func (export "plugin_step")
            (call $send (i32.add (global.get $memory_base) (i32.const 200)) (i32.const 1)))
        (func $worker (export "worker") (param $result i32)
            (i32.atomic.store8 (local.get $result)
                (i32.add
                    (i32.add
                        (i32.load offset=16 (global.get $tls))
                        (i32.load (global.get $tls)))
                    (i32.load (global.get $own_value))))
            (call $wake))
    )"#
    .replace("{IDENTITY}", identity)
}

fn settled(plugin: &mut Plugin, wanted: u8) -> Result<bool, String> {
    for _ in 0..500 {
        plugin.step()?;
        if plugin
            .take_outbound()
            .last()
            .is_some_and(|frame| frame == &[wanted])
        {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    Ok(false)
}

const ECHO: &[u8] = include_bytes!(env!("GUEST_ECHO_WASM"));

fn echo_frames(
    worker: &mut block_guest_host::Worker<
        block_guest_host::thread::Threaded<std::convert::Infallible>,
    >,
    wanted: &[&[u8]],
) -> Result<Vec<Vec<u8>>, String> {
    let mut received = Vec::new();
    while !wanted
        .iter()
        .all(|frame| received.iter().any(|got: &Vec<u8>| got == frame))
    {
        let deadline = block_guest_host::Deadline::after(std::time::Duration::from_secs(60));
        if !worker.wait(deadline)? {
            return Err(format!("the guest sent only {received:?}"));
        }
        received.extend(worker.take_received());
    }
    Ok(received)
}

mod a_dropped_plugin_ends_its_threads;
mod a_guest_creates_a_buffer_and_reports_its_handle;
mod a_guest_pointer_past_its_memory_is_refused;
mod a_guest_reads_a_frame_the_host_queued;
mod a_guest_that_never_reaches_the_gpu_opens_no_device;
mod a_guest_that_reaches_the_gpu_without_a_device_is_told_why;
mod a_plugin_linked_against_the_sdk_runs_on_it_in_every_thread;
mod a_plugin_runs_its_constructors_before_it_starts;
mod a_plugin_runs_on_a_thread_of_its_own;
mod a_plugin_runs_on_when_another_is_interrupted;
mod a_plugin_sdk_that_is_not_what_it_is_named_is_refused;
mod a_plugin_that_panics_reports_its_panic_message;
mod a_pure_guest_echoes_and_wakes_on_a_thread_of_its_own;
mod a_pure_guest_is_loaded_from_its_precompiled_module;
mod a_pure_guest_may_not_import_wasi;
mod a_pure_guest_may_not_outgrow_its_memory_limit;
mod a_pure_guest_that_hangs_is_interrupted;
mod a_pure_guest_that_panics_reports_its_panic_message;
mod a_spawned_guest_thread_may_not_reach_the_gpu;
mod a_spawned_guest_thread_shares_the_memory_it_was_given;
mod a_spawned_guest_thread_wakes_the_host;
mod a_standalone_plugin_with_a_memory_of_its_own_paints_through_the_gpu;
mod a_thread_that_trapped_wakes_the_host_to_be_told;
mod a_type_module_answers_on_a_thread_of_its_own;
mod an_interrupted_host_loads_no_plugin;
mod an_interrupted_plugin_stops_on_every_thread;
mod the_thread_local_diamond_runs_on_every_thread;
