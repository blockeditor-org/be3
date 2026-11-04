use super::*;

#[test]
fn an_app_runs_its_constructors_before_it_starts() {
    let source = r#"(module
        (import "env" "memory" (memory 1 4 shared))
        (import "be3_host" "host_send" (func $host_send (param i32 i32)))
        (func (export "__wasm_call_ctors")
            (i32.store8 (i32.const 16) (i32.const 1)))
        (func (export "app_start")
            (call $host_send (i32.const 16) (i32.const 1)))
        (func (export "app_shutdown"))
        (func (export "app_step"))
    )"#;
    let mut app = host().load_bytes(source.as_bytes()).unwrap();

    app.start().unwrap();

    assert_eq!(app.take_outbound(), vec![vec![1]]);
}
