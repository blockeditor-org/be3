use super::*;

#[test]
fn a_pure_guest_may_not_import_wasi() {
    let module = r#"(module
        (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
        (memory (export "memory") 1)
        (func (export "plugin_start"))
        (func (export "plugin_step"))
        (func (export "plugin_shutdown")))"#;
    let refused = PureHost::new(None)
        .unwrap()
        .load_bytes(module.as_bytes())
        .err()
        .expect("a guest that imports wasi is refused");
    assert!(refused.contains("fd_write"), "{refused}");
}
