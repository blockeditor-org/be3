;; A standalone plugin (guides/plugin_sdk.md, "The raw plugin interface"): one
;; module with no dylink.0, no SDK, no WASI, no threads and no thread-local
;; storage, which owns a memory that is not shared and paints through be3:gpu.
;; block-wasm-host's tests run it on wasmtime, and :standalone-web-test
;; (crates/block-wasm-host/BUCK) in Chromium through plugin.js and
;; block-gpu-shim.
;;
;; plugin_start makes surface 7 and gives it a 4 by 4 rgba8unorm target. The
;; first plugin_step clears it to orange in a render pass, presents it and
;; sends the frame [1]; later steps send [2] and draw nothing. Each handle it
;; is given is the first of its kind, 1, which the descriptors below name.
(module
  (import "be3:gpu/device@0.1.0" "[constructor]surface" (func $surface (param i32) (result i32)))
  (import "be3:gpu/device@0.1.0" "[method]surface.configure" (func $configure (param i32 i32 i32 i32)))
  (import "be3:gpu/device@0.1.0" "[method]surface.acquire" (func $acquire (param i32) (result i32)))
  (import "be3:gpu/device@0.1.0" "[method]surface.present" (func $present (param i32)))
  (import "be3:gpu/device@0.1.0" "[method]texture.create-view" (func $create_view (param i32 i32 i32) (result i32)))
  (import "be3:gpu/device@0.1.0" "create-command-encoder" (func $create_encoder (param i32 i32) (result i32)))
  (import "be3:gpu/device@0.1.0" "[method]command-encoder.begin-render-pass" (func $begin (param i32 i32 i32) (result i32)))
  (import "be3:gpu/device@0.1.0" "[method]render-pass.end" (func $end (param i32)))
  (import "be3:gpu/device@0.1.0" "[method]command-encoder.finish" (func $finish (param i32) (result i32)))
  (import "be3:gpu/device@0.1.0" "submit" (func $submit (param i32 i32)))
  (import "be3_host" "host_send" (func $send (param i32 i32)))

  (memory (export "memory") 1)

  ;; A texture-view-descriptor: no label, format, dimension or counts, every aspect.
  (data (i32.const 16) "\00\00\00\00\00\00\00\00")
  ;; A command-encoder-descriptor with no label.
  (data (i32.const 32) "\00")
  ;; A render-pass-descriptor: no label, one color attachment on view 1 with
  ;; no resolve target, cleared to (1, 0.5, 0, 1) and stored, no depth slice,
  ;; no depth-stencil attachment, occlusion query set or timestamp writes.
  (data (i32.const 48)
    "\00\01\01\01\00\00"
    "\00\00\00\00\00\00\f0\3f" "\00\00\00\00\00\00\e0\3f"
    "\00\00\00\00\00\00\00\00" "\00\00\00\00\00\00\f0\3f"
    "\00\00\00\00\00")
  ;; The frames it sends.
  (data (i32.const 112) "\01\02")

  (global $surface (mut i32) (i32.const 0))
  (global $painted (mut i32) (i32.const 0))

  (func $expect (param $handle i32) (result i32)
    (if (i32.ne (local.get $handle) (i32.const 1)) (then unreachable))
    (local.get $handle))

  (func (export "plugin_start")
    (global.set $surface (call $surface (i32.const 7)))
    ;; 17 is rgba8unorm in be3:gpu's texture-format.
    (call $configure (global.get $surface) (i32.const 4) (i32.const 4) (i32.const 17)))

  (func (export "plugin_step")
    (local $encoder i32)
    (local $pass i32)
    (if (global.get $painted)
      (then
        (call $send (i32.const 113) (i32.const 1))
        (return)))
    (drop (call $expect
      (call $create_view (call $expect (call $acquire (global.get $surface))) (i32.const 16) (i32.const 8))))
    (local.set $encoder (call $expect (call $create_encoder (i32.const 32) (i32.const 1))))
    (local.set $pass (call $expect (call $begin (local.get $encoder) (i32.const 48) (i32.const 43))))
    (call $end (local.get $pass))
    (i32.store (i32.const 128) (call $expect (call $finish (local.get $encoder))))
    (call $submit (i32.const 128) (i32.const 1))
    (call $present (global.get $surface))
    (global.set $painted (i32.const 1))
    (call $send (i32.const 112) (i32.const 1)))

  (func (export "plugin_shutdown"))
)
