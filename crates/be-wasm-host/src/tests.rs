use super::*;

use be_wasm_gpu_abi::device as abi;

fn host() -> Host {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    Host::new(device, queue).unwrap()
}

fn escaped(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{byte:02x}")).collect()
}

fn buffer_descriptor() -> Vec<u8> {
    be_wasm_ops::encode(&abi::BufferDescriptor {
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
            (import "env" "memory" (memory 1 4))
            (import "be3:gpu/device@0.1.0" "create-buffer" (func $create_buffer (param i32 i32) (result i32)))
            (import "be3:gpu/device@0.1.0" "take-error" (func $take_error (param i32 i32) (result i32)))
            (import "be3_host" "host_send" (func $host_send (param i32 i32)))
            (import "be3_host" "host_receive" (func $host_receive (param i32 i32) (result i64)))
            (data (i32.const 0) "{descriptor}")
            (global $length i32 (i32.const {length}))
            (func (export "app_start"))
            (func (export "app_shutdown"))
            (func (export "app_step") {body})
        )"#,
        descriptor = escaped(&descriptor),
        length = descriptor.len(),
    )
}

/// A root library: the memory it imports, its table and stack pointer, and what the app above
/// it reaches through it.
fn root() -> &'static str {
    r#"(module
        (import "env" "memory" (memory 2 4))
        (import "be3_host" "host_send" (func $host_send (param i32 i32)))
        (table (export "__indirect_function_table") 2 funcref)
        (elem (i32.const 1) $double)
        (global $stack (export "__stack_pointer") (mut i32) (i32.const 4096))
        (global (export "counter") i32 (i32.const 2048))
        (func $double (export "double") (param i32) (result i32)
            (i32.mul (local.get 0) (i32.const 2)))
        (func (export "send") (param i32 i32) (call $host_send (local.get 0) (local.get 1)))
    )"#
}

fn root_identity() -> String {
    be_wasm_dylink::identity(root().as_bytes())
}

/// An app linked against `root()`, which calls a function of the root's through its table, counts
/// in the root's data and reads its own relocated data, and sends 42 + 5 when every one held.
fn linked_app(identity: &str) -> String {
    r#"(module
        (import "env" "memory" (memory 1 4))
        (import "env" "__indirect_function_table" (table 0 funcref))
        (import "env" "__stack_pointer" (global $stack (mut i32)))
        (import "env" "__memory_base" (global $memory_base i32))
        (import "env" "__table_base" (global $table_base i32))
        (import "env" "send" (func $send (param i32 i32)))
        (import "GOT.mem" "counter" (global $counter (mut i32)))
        (import "GOT.mem" "own_value" (global $own_value (mut i32)))
        (import "GOT.func" "double" (global $double (mut i32)))
        (type $doubling (func (param i32) (result i32)))
        (global (export "own_value") i32 (i32.const 64))
        (global (export "__be3_lib_0_{IDENTITY}") i32 (i32.const 0))
        (global (export "__be3_dylink_256_2_1_0") i32 (i32.const 0))
        (func (export "__wasm_apply_data_relocs")
            (i32.store offset=64 (global.get $memory_base) (i32.const 5)))
        (func (export "app_start")
            (i32.store (global.get $counter) (i32.add (i32.load (global.get $counter)) (i32.const 1)))
            (if (i32.ne (i32.load (i32.const 2048)) (i32.const 1)) (then unreachable)))
        (func (export "app_shutdown"))
        (func (export "app_step")
            (i32.store8 offset=200 (global.get $memory_base)
                (i32.add
                    (call_indirect (type $doubling) (i32.const 21) (global.get $double))
                    (i32.load (global.get $own_value))))
            (call $send (i32.add (global.get $memory_base) (i32.const 200)) (i32.const 1)))
    )"#
    .replace("{IDENTITY}", identity)
}

const ECHO: &[u8] = include_bytes!(env!("GUEST_ECHO_WASM"));

fn echo_frames(
    worker: &mut be_wasm_guest_host::Worker<
        be_wasm_guest_host::thread::Threaded<std::convert::Infallible>,
    >,
    wanted: &[&[u8]],
) -> Result<Vec<Vec<u8>>, String> {
    let mut received = Vec::new();
    while !wanted
        .iter()
        .all(|frame| received.iter().any(|got: &Vec<u8>| got == frame))
    {
        let deadline = be_wasm_guest_host::Deadline::after(std::time::Duration::from_secs(60));
        if !worker.wait(deadline)? {
            return Err(format!("the guest sent only {received:?}"));
        }
        received.extend(worker.take_received());
    }
    Ok(received)
}

mod a_code_cache_entry_that_is_not_whole_or_not_the_modules_is_refused;
mod a_code_cache_entry_v8_refuses_costs_only_the_compile;
mod a_guest_creates_a_buffer_and_reports_its_handle;
mod a_guest_pointer_past_its_memory_is_refused;
mod a_guest_reads_a_frame_the_host_queued;
mod a_guest_that_declares_garbage_collected_types_is_refused;
mod a_guest_that_grows_its_memory_is_read_where_it_grew;
mod a_guest_that_never_reaches_the_gpu_opens_no_device;
mod a_guest_that_reaches_the_gpu_without_a_device_is_told_why;
mod a_guest_that_reads_past_its_memory_traps;
mod a_guest_that_would_have_threads_is_refused;
mod a_library_that_is_not_what_it_is_named_is_refused;
mod a_module_from_bytes_compiles_once_and_each_instance_runs_on_its_own;
mod a_module_from_bytes_imports_the_memory_it_defines;
mod a_module_from_bytes_is_capped_at_the_memory_each_instance_is_given;
mod a_pure_guest_echoes_and_wakes_on_a_thread_of_its_own;
mod a_pure_guests_randomness_is_chacha20s_stream_from_its_seed;
mod a_pure_guest_may_not_have_two_memories;
mod a_pure_guest_may_not_import_more_of_wasi_than_it_is_answered;
mod a_pure_guest_may_not_outgrow_its_memory_limit;
mod a_pure_guest_that_hangs_is_interrupted;
mod a_pure_guest_that_panics_reports_its_panic_message;
mod a_standalone_app_with_a_memory_of_its_own_paints_through_the_gpu;
mod a_type_module_answers_on_a_thread_of_its_own;
mod a_wasi_call_naming_more_than_the_memory_is_refused;
mod an_app_linked_against_a_library_runs_on_it;
mod an_app_runs_its_constructors_before_it_starts;
mod an_app_runs_on_a_thread_of_its_own;
mod an_app_runs_on_when_another_is_interrupted;
mod an_app_that_panics_reports_its_panic_message;
mod an_interrupted_app_stops_while_it_runs;
mod an_interrupted_host_loads_no_app;
mod bytes_that_are_not_a_pure_guests_module_do_not_compile;
mod every_wasi_call_a_pure_guest_is_answered_is_defined;
