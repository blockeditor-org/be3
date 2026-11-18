use super::*;

/// Writes to standard error through two buffers each of 4 GiB less one byte.
const OVERSIZED: &str = r#"(module
    (import "env" "memory" (memory 1 4))
    (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
    (func (export "app_start"))
    (func (export "app_shutdown"))
    (func (export "app_step")
        (i32.store (i32.const 0) (i32.const 64))
        (i32.store (i32.const 4) (i32.const -1))
        (i32.store (i32.const 8) (i32.const 64))
        (i32.store (i32.const 12) (i32.const -1))
        (drop (call $fd_write (i32.const 2) (i32.const 0) (i32.const 2) (i32.const 32))))
)"#;

#[test]
fn a_wasi_call_naming_more_than_the_memory_is_refused() {
    let mut app = host().load_bytes(OVERSIZED.as_bytes()).unwrap();
    app.start().unwrap();
    let error = app.step().unwrap_err();
    assert!(error.contains("ran past its memory"), "{error}");
}
