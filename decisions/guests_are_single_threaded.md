# Guests are single-threaded, and nothing may assume one thread

Every guest (an app, the libraries apps link against, an app's workers, and an app's tests) is
built for `wasm32-wasip1`, single-threaded, as are the C and C++ libraries guests link
(wasi-libc, libc++, FreeType, HarfBuzz, PDFium, libghostty-vt). A guest has one thread, and its
memory is imported and unshared: the loaders (be-wasm-host natively, `web/app.js` in the browser)
link an app's libraries into the memory its root library imports, which is not shared. Work that needs another thread runs as an instance of its
own, a worker (`be_sdk_worker`), spawned through the sandbox API and talking to its app by
messages. Programs in the browser, such as be-host's web bundle, are not guests and stay
`wasm32-wasip1-threads`, so the page's thread support stays for them (`web/threads.js`,
`thread.js`, crates/be-wasm-threads); be-wasm-host has none.

Threads are meant to come back once a standard for them has settled, so nothing built meanwhile
may rule them out. The sandbox API, the app protocol, the library format and be-sdk may not
assume a guest has one thread: no API that works only if every call comes from the same thread,
and no state kept where a second thread could not reach it.

## Why

- `wasi-threads` (`thread-spawn`) is a legacy proposal, experimental where it is implemented, and
  it does not work with the component model. The threads expected to replace it,
  shared-everything threads, have thread-local globals and spawn threads through the component
  model, so the machinery built for today's threads would mostly not carry over.
- Threads cost code in every layer: the linker's rewriting of thread-local accesses, a
  thread-local block per module per thread, relinking every library on every thread, and
  `thread-spawn` natively and in the browser.
- A shared memory only grows, so an instance keeps whatever its busiest thread needed, and its
  allocator takes a lock.

## What it costs

- No `std::thread` or rayon in a guest. Background work is a worker.
- A result crosses between instances as a copy.
- An instance costs more memory than a thread: it shares the compiled code but has its own heap
  and its own copy of each library's data.
- Natively, an app's gpu and transport imports are V8 fast-API calls, which may not ask V8 where
  the memory is, and an unshared memory may move when it grows. be-wasm-host turns on V8's trap
  handler, under which a wasm32 memory is reserved in full with guard regions and only grows in
  place, so the calls read its backing store, which stays put (`engine::memories_stay_put`);
  without the trap handler they are ordinary V8 calls. Measured with
  `./scripts/buck run -m release //crates/be-wasm-host:example-gpu-calls` (four render-pass calls
  in a loop, on a noop device, fastest of 20 steps) in November 2026: 46.7 ns a call with the
  shared memory guests had, 240.6 ns with an unshared one and ordinary calls, and 43.1 ns with
  an unshared one and fast calls.

## Rejected

- Keeping `wasm32-wasip1-threads` for guests that start no thread: lld refuses to link an object
  compiled with atomics into a module whose memory is not shared, and a shared memory needs every
  object compiled with them, so the choice is made for every guest at once.
- Shared-memory threads between an app and its workers: the same legacy proposal, and the same
  machinery.
