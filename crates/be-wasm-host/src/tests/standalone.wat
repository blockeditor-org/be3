;; A standalone app (guides/app_libraries.md, "The raw app interface"): one
;; module with no dylink.0, no SDK, no WASI, no threads and no thread-local
;; storage, which owns a memory that is not shared and paints through
;; wasi:webgpu and be3:gpu/host. be-wasm-host's tests run it on V8, and
;; :standalone-web-test (crates/be-wasm-host/BUCK) in Chromium through app.js
;; and block-gpu-shim.
;;
;; app_start asks for the adapter and a device, waiting for each through
;; [poll], takes surface 7's context and gives it a 4 by 4 rgba8unorm target.
;; The first app_step clears it to orange in a render pass, presents it and
;; sends the frame [1]; later steps send [2] and draw nothing. Each handle it
;; is given is the first of its kind, 1, which the descriptors below name.
;; Descriptors are on the wire as be_wasm_ops::wire::Plain (postcard) has them.
(module
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "get-gpu" (func $get_gpu (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu.request-adapter" (func $request_adapter (param i32 i32 i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-adapter.request-device" (func $request_device (param i32 i32 i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[poll]" (func $poll (param i32 i32 i32 i32 i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-device.queue" (func $queue (param i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-canvas-context.configure" (func $configure (param i32 i32 i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-canvas-context.get-current-texture" (func $current (param i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-texture.create-view" (func $create_view (param i32 i32 i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-device.create-command-encoder" (func $create_encoder (param i32 i32 i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-command-encoder.begin-render-pass" (func $begin (param i32 i32 i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-render-pass-encoder.end" (func $end (param i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-command-encoder.finish" (func $finish (param i32 i32 i32) (result i32)))
  (import "wasi:webgpu/webgpu@0.3.0-rc.2" "[method]gpu-queue.submit" (func $submit (param i32 i32 i32)))
  (import "be3:gpu/host@0.1.0" "surface" (func $surface (param i32) (result i32)))
  (import "be3:gpu/host@0.1.0" "resize" (func $resize (param i32 i32 i32)))
  (import "be3:gpu/host@0.1.0" "present" (func $present (param i32)))
  (import "be3_host" "host_send" (func $send (param i32 i32)))

  (memory (export "memory") 1)

  ;; None: no adapter options, device descriptor, view descriptor, encoder
  ;; descriptor or command buffer descriptor.
  (data (i32.const 16) "\00")
  ;; A render-pass-descriptor: one color attachment, on view 1, with no depth
  ;; slice or resolve target, cleared to (1, 0.5, 0, 1) and stored; no
  ;; depth-stencil attachment, occlusion query set, timestamp writes, max draw
  ;; count or label.
  (data (i32.const 48)
    "\01\01\01\00\00\01"
    "\00\00\00\00\00\00\f0\3f" "\00\00\00\00\00\00\e0\3f"
    "\00\00\00\00\00\00\00\00" "\00\00\00\00\00\00\f0\3f"
    "\01\00" "\00\00\00\00\00")
  ;; The frames it sends.
  (data (i32.const 112) "\01\02")
  ;; A gpu-canvas-configuration: device 1, rgba8unorm (21 in wasi:webgpu's
  ;; gpu-texture-format), nothing else.
  (data (i32.const 144) "\01\15\00\00\00\00\00")
  ;; What [poll] settles a ticket with: the adapter, Some(1), and the device,
  ;; Ok(1), each as a byte string.
  (data (i32.const 176) "\01\02\01\01")
  (data (i32.const 184) "\01\02\00\01")

  (global $context (mut i32) (i32.const 0))
  (global $queue (mut i32) (i32.const 0))
  (global $painted (mut i32) (i32.const 0))

  (func $expect (param $handle i32) (result i32)
    (if (i32.ne (local.get $handle) (i32.const 1)) (then unreachable))
    (local.get $handle))

  ;; Polls until the host settles `ticket`, alone, with the 4 bytes at
  ;; `expected`: [poll] answers a list of (ticket, option<bytes>).
  (func $settled (param $ticket i32) (param $expected i32)
    (if (i32.ne (call $poll (i32.const 1) (i32.const 0) (i32.const 0) (i32.const 512) (i32.const 64)) (i32.const 6))
      (then unreachable))
    (if (i32.ne (i32.load8_u (i32.const 512)) (i32.const 1)) (then unreachable))
    (if (i32.ne (i32.load8_u (i32.const 513)) (local.get $ticket)) (then unreachable))
    (if (i32.ne (i32.load (i32.const 514)) (i32.load (local.get $expected)))
      (then unreachable)))

  (func (export "app_start")
    (call $settled
      (call $request_adapter (call $expect (call $get_gpu)) (i32.const 16) (i32.const 1))
      (i32.const 176))
    (call $settled
      (call $request_device (i32.const 1) (i32.const 16) (i32.const 1))
      (i32.const 184))
    (global.set $queue (call $expect (call $queue (i32.const 1))))
    (global.set $context (call $expect (call $surface (i32.const 7))))
    (call $resize (global.get $context) (i32.const 4) (i32.const 4))
    (call $configure (global.get $context) (i32.const 144) (i32.const 7)))

  (func (export "app_step")
    (local $encoder i32)
    (local $pass i32)
    (if (global.get $painted)
      (then
        (call $send (i32.const 113) (i32.const 1))
        (return)))
    (drop (call $expect
      (call $create_view (call $expect (call $current (global.get $context))) (i32.const 16) (i32.const 1))))
    (local.set $encoder (call $expect (call $create_encoder (i32.const 1) (i32.const 16) (i32.const 1))))
    (local.set $pass (call $expect (call $begin (local.get $encoder) (i32.const 48) (i32.const 45))))
    (call $end (local.get $pass))
    (i32.store (i32.const 128) (call $expect (call $finish (local.get $encoder) (i32.const 16) (i32.const 1))))
    (call $submit (global.get $queue) (i32.const 128) (i32.const 1))
    (call $present (global.get $context))
    (global.set $painted (i32.const 1))
    (call $send (i32.const 112) (i32.const 1)))

  (func (export "app_shutdown"))
)
