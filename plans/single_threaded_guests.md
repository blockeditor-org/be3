# Single-threaded guests

Depends on: plans/host_as_platform_adapter.md, steps 1 and 2 (the sandbox API and spawning staged
apps, which an app needs to start its own worker), and plans/web_page_never_waits.md (after which
nothing on the web page shares memory with a thread).

Every guest is built for `wasm32-wasip1-threads` today: one shared memory per app, threads made
by `wasi-threads` (`thread-spawn`), and a thread-local block per module per thread. Drop that.
Each guest becomes single-threaded `wasm32-wasip1`, and work that needs another thread runs as an
instance of its own, spawned through the sandbox API, talking to its spawner by messages.

Why:

- `wasi-threads` is a legacy proposal: wasmtime treats it as experimental and it does not work
  with the component model. The threads expected to replace it ("shared-everything threads")
  have thread-local globals and spawn threads through the component model, so the machinery
  built for today's threads would mostly not carry over.
- Supporting threads costs code in every layer: the linker's rewriting of thread-local accesses
  (`GOT.tls`, guides/app_libraries.md, Thread-locals), relinking every library on every thread,
  `thread-spawn` on wasmtime (`be-wasm-host/src/threads.rs`) and in the browser
  (`be-host/web/threads.js`, `thread.js`, the locked list of thread-local blocks in `dylink.js`),
  and a V8 runtime (plans/deferred/wasm_on_v8.md) would have to implement it again.
- A shared memory only grows, so an instance keeps whatever its busiest thread needed, and its
  allocator takes a lock.

Apps that work in parallel keep doing so. Map and PDF still render on another thread; that
thread runs an instance of its own rather than shared-memory Rust threads in the app's.

What it costs:

- No `std::thread` or rayon in guests until shared-memory threads come back on a standard that
  has settled. Background work is an instance of its own.
- A result crosses between instances as a copy: a rendered PDF page at 4K is about 33 MB, a few
  milliseconds, once per render, and it is copied again when it is uploaded to the GPU anyway.
- An instance costs more memory than a thread: it shares the compiled code but has its own heap
  and its own copy of each library's data.

What does not change: the web stays cross-origin isolated, since WASI file calls reach the OPFS
worker synchronously through a `SharedArrayBuffer` and `Atomics.wait` (`web/wasi.js`,
`files.js`).

## Nothing may assume one thread

Threads are meant to come back once there is a settled standard for them, so nothing built
meanwhile may rule them out. The sandbox API, the app protocol, the library format and be-sdk
must not assume a guest has one thread: no API that only works if every call comes from the
same thread, and no state kept in a way a second thread could not reach. Step 3, which makes
guests single-threaded, records both as a decision: guests are single-threaded and work in
parallel as instances of their own, and nothing may assume one thread.

## Workers

Today two apps start a thread, each a background worker fed by a channel:

- `be-app-map/src/tiles.rs`: the tile rasterizer.
- `be-app-pdf/src/render/pdfium.rs`: PDFium, initialized and run on its own thread.

Each becomes an instance of its own app's module, spawned through the sandbox API with an entry
point of its own, receiving jobs and sending results over its channel. Natively it runs on its
own OS thread and in the browser in its own worker, so it may wait on its channel. Give be-sdk
one way to start such a worker and exchange messages with it, so the next app that needs one does
not write its own.

## Steps

1. Serve the sandbox API to apps. An app may spawn only its own staged module, at another entry
   point, with the imports the host serves that app; modules from bytes stay the root guest's
   until a plan gives apps a reason for them (the Games app's game modules would be one).
   Then a worker API in be-sdk over that spawn: an entry point, a channel, and byte
   buffers carried as messages.
2. Port the map and PDF workers to it.
3. Build every guest, and the C libraries guests link (wasi-libc, libc++, PDFium, FreeType,
   HarfBuzz, libghostty-vt), for `wasm32-wasip1`: `buck/wasm/defs.bzl`, `buck/toolchains/BUCK`
   (drop `-pthread`), `buck/constraints/BUCK`, `crates/be-lib-ghostty-vt/BUCK`,
   `rust-toolchain.toml`, and the test runner (decisions/app_tests_run_as_wasm_guests.md). Write
   the decision above.
4. Delete thread support:
   - the linker's thread-local rewriting (`be-wasm-link`'s `tls.rs`) and the loader's
     thread-local blocks (`thread_local_slot` in `be-wasm-dylink`, `be-wasm-host/src/dylink.rs`,
     `be-host/web/dylink.js`);
   - `thread-spawn` (`be-wasm-host/src/threads.rs`, `be-host/web/threads.js` and `thread.js`);
   - the thread-local diamond crates (`crates/be-tool-tls-diamond-*`) and their tests;
   - what is left of `be-wasm-threads` (`initialize_page`, `initialize_worker`, `may_wait`) and
     the page's refusal of a thread (`refuseThreads` in `be-host/web/threads.js`).
5. See whether the base library still has to be a fixed-address root
   (decisions/app_library_base_is_a_fixed_address_root.md): the reason recorded there is the
   threaded std and wasi-libc builds. Libraries could also load after start, since nothing
   shares a table between threads any more (guides/app_libraries.md, Loading).
6. Delete decisions/thread_locals_across_app_libraries_through_got_tls.md,
   and update guides/app_libraries.md, guides/apps/runtime.md,
   guides/running/web.md, guides/buck2.md and guides/testing_a_gui.md.
