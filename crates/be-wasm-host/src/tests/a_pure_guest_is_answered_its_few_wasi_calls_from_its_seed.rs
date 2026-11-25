use super::*;

/// Sends 16 random bytes, then the errnos of asking for its environment, the clock and writing
/// "hello\n" to standard error, and how many bytes that wrote; exits with 7 when stepped.
const GUEST: &str = r#"(module
    (import "wasi_snapshot_preview1" "random_get" (func $random (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "environ_sizes_get" (func $environ (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "clock_time_get" (func $clock (param i32 i64 i32) (result i32)))
    (import "wasi_snapshot_preview1" "fd_write" (func $write (param i32 i32 i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
    (import "be3_host" "host_send" (func $send (param i32 i32)))
    (memory (export "memory") 1)
    (data (i32.const 100) "hello\n")
    (func (export "app_start")
        (drop (call $random (i32.const 0) (i32.const 16)))
        (call $send (i32.const 0) (i32.const 16))
        (i32.store8 (i32.const 32) (call $environ (i32.const 40) (i32.const 44)))
        (i32.store8 (i32.const 33) (call $clock (i32.const 1) (i64.const 0) (i32.const 48)))
        (i32.store (i32.const 56) (i32.const 100))
        (i32.store (i32.const 60) (i32.const 6))
        (i32.store8 (i32.const 34)
            (call $write (i32.const 2) (i32.const 56) (i32.const 1) (i32.const 64)))
        (i32.store8 (i32.const 35) (i32.load8_u (i32.const 64)))
        (call $send (i32.const 32) (i32.const 4)))
    (func (export "app_step") (call $exit (i32.const 7)))
    (func (export "app_shutdown")))"#;

fn started(seed: [u8; 32]) -> (App, Vec<Vec<u8>>) {
    let mut app = Host::pure()
        .unwrap()
        .with_seed(seed)
        .load_bytes(GUEST.as_bytes())
        .expect("a pure guest may import these");
    app.start().expect("the guest starts");
    let sent = app.take_outbound();
    (app, sent)
}

#[test]
fn a_pure_guest_is_answered_its_few_wasi_calls_from_its_seed() {
    let (mut app, sent) = started([0; 32]);
    // The zero seed's stream begins as ChaCha20's under the zero key does (be-wasm-pure-wasi).
    let zero = [
        0x76, 0xb8, 0xe0, 0xad, 0xa0, 0xf1, 0x3d, 0x90, 0x40, 0x5d, 0x6a, 0xe5, 0x53, 0x86, 0xbd,
        0x28,
    ];
    assert_eq!(sent, vec![zero.to_vec(), vec![0, 52, 0, 6]]);
    assert_eq!(started([0; 32]).1, sent, "the same seed, the same answers");
    assert_ne!(started([1; 32]).1[0], sent[0], "another seed, other randomness");
    let exited = app.step().expect_err("the guest exits");
    assert!(exited.contains("exited with status 7"), "{exited}");
}
