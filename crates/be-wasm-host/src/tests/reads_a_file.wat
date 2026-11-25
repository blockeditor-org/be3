;; A pure guest that would read a file, which no host of one answers.
(module
    (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
    (import "wasi_snapshot_preview1" "fd_read" (func (param i32 i32 i32 i32) (result i32)))
    (import "be3_host" "host_send" (func (param i32 i32)))
    (memory (export "memory") 1)
    (func (export "app_start"))
    (func (export "app_step"))
    (func (export "app_shutdown")))
