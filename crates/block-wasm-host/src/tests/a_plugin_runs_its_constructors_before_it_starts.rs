use super::*;

#[test]
fn a_plugin_runs_its_constructors_before_it_starts() {
    let source = r#"(module
        (import "env" "memory" (memory 1 4 shared))
        (import "be3_host" "host_send" (func $host_send (param i32 i32)))
        (global (export "__tls_size") i32 (i32.const 0))
        (global (export "__tls_align") i32 (i32.const 1))
        (func (export "plugin_initialize_tls") (param i32 i32))
        (func (export "__wasm_call_ctors")
            (i32.store8 (i32.const 16) (i32.const 1)))
        (func (export "plugin_start")
            (call $host_send (i32.const 16) (i32.const 1)))
        (func (export "plugin_shutdown"))
        (func (export "plugin_step"))
    )"#;
    let mut plugin = host().load_bytes(source.as_bytes()).unwrap();

    plugin.start().unwrap();

    assert_eq!(plugin.take_outbound(), vec![vec![1]]);
}
