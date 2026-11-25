;; A pure guest that imports every WASI call a pure guest is answered (be_wasm_pure_wasi::ANSWERED),
;; run natively by be-wasm-host's tests and in the browser by be-wasm-root-web-testing's, so both
;; hosts' answers are held to the same vectors. It starts by sending the errnos of asking for its
;; environment, the clock and writing a line to standard error, and the bytes that wrote; then each
;; frame it receives is a command: [0, n] sends n random bytes, [1, status] exits with status.
(module
    (import "wasi_snapshot_preview1" "args_get" (func (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "args_sizes_get" (func (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "clock_res_get" (func (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "clock_time_get" (func $clock (param i32 i64 i32) (result i32)))
    (import "wasi_snapshot_preview1" "environ_get" (func (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "environ_sizes_get" (func $environ (param i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "fd_write" (func $write (param i32 i32 i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
    (import "wasi_snapshot_preview1" "random_get" (func $random (param i32 i32) (result i32)))
    (import "be3_host" "host_send" (func $send (param i32 i32)))
    (import "be3_host" "host_receive" (func $receive (param i32 i32) (result i64)))
    (memory (export "memory") 1)
    (data (i32.const 100) "hello from a pure guest\n")
    (func (export "app_start")
        (i32.store8 (i32.const 32) (call $environ (i32.const 40) (i32.const 44)))
        (i32.store8 (i32.const 33) (call $clock (i32.const 1) (i64.const 0) (i32.const 48)))
        (i32.store (i32.const 56) (i32.const 100))
        (i32.store (i32.const 60) (i32.const 24))
        (i32.store8 (i32.const 34)
            (call $write (i32.const 2) (i32.const 56) (i32.const 1) (i32.const 64)))
        (i32.store8 (i32.const 35) (i32.load8_u (i32.const 64)))
        (call $send (i32.const 32) (i32.const 4)))
    (func (export "app_step")
        (block $done
            (loop $next
                (br_if $done
                    (i64.lt_s (call $receive (i32.const 1024) (i32.const 16)) (i64.const 0)))
                (if (i32.eq (i32.load8_u (i32.const 1024)) (i32.const 1))
                    (then (call $exit (i32.load8_u (i32.const 1025)))))
                (drop (call $random (i32.const 2048) (i32.load8_u (i32.const 1025))))
                (call $send (i32.const 2048) (i32.load8_u (i32.const 1025)))
                (br $next))))
    (func (export "app_shutdown")))
