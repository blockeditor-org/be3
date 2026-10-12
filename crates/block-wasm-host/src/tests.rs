use super::*;

use block_gpu_abi as abi;

fn host() -> Host {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    Host::new(device, queue, None).unwrap()
}

fn escaped(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{byte:02x}")).collect()
}

fn buffer_descriptor() -> Vec<u8> {
    abi::encode(&abi::BufferDescriptor {
        label: "vertices".into(),
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
            (import "be3_gpu" "create_buffer" (func $create_buffer (param i32 i32) (result i32)))
            (import "be3_gpu" "error_take" (func $error_take (param i32 i32) (result i32)))
            (import "be3_host" "host_send" (func $host_send (param i32 i32)))
            (import "be3_host" "host_receive" (func $host_receive (param i32 i32) (result i64)))
            (data (i32.const 0) "{descriptor}")
            (global $length i32 (i32.const {length}))
            (global (export "__tls_size") i32 (i32.const 0))
            (global (export "__tls_align") i32 (i32.const 1))
            (func (export "plugin_initialize_tls") (param i32 i32))
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
            (import "be3_gpu" "create_buffer" (func $create_buffer (param i32 i32) (result i32)))
            (import "be3_host" "host_send" (func $host_send (param i32 i32)))
            (import "be3_host" "host_wake" (func $host_wake))
            (global (export "__tls_size") i32 (i32.const 0))
            (global (export "__tls_align") i32 (i32.const 1))
            (func (export "plugin_initialize_tls") (param i32 i32))
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

const SDK_IDENTITY: &str = "5d1f0e3c9a7b2846c0d9e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c2d1e0f9a8b7";

fn linked_plugin() -> String {
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
    .replace("{IDENTITY}", SDK_IDENTITY)
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

mod a_guest_creates_a_buffer_and_reports_its_handle;
mod a_guest_pointer_past_its_memory_is_refused;
mod a_guest_reads_a_frame_the_host_queued;
mod a_guest_that_never_reaches_the_gpu_opens_no_device;
mod a_guest_that_reaches_the_gpu_without_a_device_is_told_why;
mod a_plugin_linked_against_the_sdk_runs_on_it_in_every_thread;
mod a_plugin_runs_its_constructors_before_it_starts;
mod a_plugin_runs_on_a_thread_of_its_own;
mod a_plugin_that_panics_reports_its_panic_message;
mod a_spawned_guest_thread_may_not_reach_the_gpu;
mod a_spawned_guest_thread_shares_the_memory_it_was_given;
mod a_spawned_guest_thread_wakes_the_host;
mod a_thread_that_trapped_wakes_the_host_to_be_told;
