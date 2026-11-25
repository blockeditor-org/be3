use super::*;

#[test]
fn a_pure_guest_may_not_import_more_of_wasi_than_it_is_answered() {
    let module = r#"(module
        (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
        (import "wasi_snapshot_preview1" "fd_read" (func (param i32 i32 i32 i32) (result i32)))
        (memory (export "memory") 1)
        (func (export "app_start"))
        (func (export "app_step"))
        (func (export "app_shutdown")))"#;
    let refused = Host::pure()
        .unwrap()
        .load_bytes(module.as_bytes())
        .err()
        .expect("a guest that reads a file is refused");
    assert!(refused.contains("fd_read"), "{refused}");
}
