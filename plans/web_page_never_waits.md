# The web page never waits

Depends on: none.

The owner's rule is that nothing spins on a lock. A worker waits for real (`Atomics.wait`), but
a browser traps `memory.atomic.wait` on a page's thread, so the page must never wait at all:
it has to be built so that nothing it needs is ever held by another thread. be-host on the web
is not built that way yet. The page and the block stack's worker (`web/be-worker.js`) run Rust
in one shared memory, and the page reaches the worker's state synchronously, so two waits
remain on the page (guides/running/web.md, "Files and offline"):

- `be_wasm_threads::Mutex::lock` spins on the page. The locks it covers are the ones the page
  and the stack's worker both take: be-host-stack's `Shared` (read by most of `lib.rs`'s public
  functions, from `node` and `query` to `status` and `content`) and its statics (`stack()`,
  `HELD_CLOCK`, `changes::WATCHERS`, `offline_choice::CHOICES`, `link::held`), the
  `RefsDb` connection (`be-block-client/src/refs/sqlite.rs`), be-block-type-host's
  `REGISTRY` and `DECLARED`, and be-host-base's `notices`.
- The page's file calls (`web/wasi.js`, `forward`) spin until `web/files.js` answers. The page
  opens the refs database itself (`be-host/src/main.rs`, `workspace_cache`, through
  `be_host_stack::refs_in`) and reads settings and kept state through WASI.

Other locks the page shares are std's own and would trap rather than spin if the page ever
found one held past std's short spin:

- be-host-base `host.rs`'s `POSTED` and `POSTED_COUNT`, which the stack's worker takes to post a
  letter to a page mailbox.
- The `Peer`'s locks, which the page takes through `be_host_stack::offline_peer`
  (`app_graph::answer`, `out_of_reach`).
- The channels' insides: a tokio `mpsc` receiver can park while a send is half done.
- wasi-libc's `malloc`, which locks the one heap the page and the worker share.

The last one is why this cannot be done lock by lock: while the page and another thread run
Rust in one memory, any allocation can find the heap held. The page has to share no memory with
a thread that runs beside it.

## The end state

- The stack's worker is an instance with a memory of its own, started from the same module.
  The page and the worker talk only by `postMessage`.
- The page keeps what its synchronous API reads (`Seen`: the graph, open blocks' content and
  history, status, contacts, pairing, conflicts, search results) as its own copy, which the
  worker keeps up to date with messages, and which the page applies at the start of a frame.
  A reactive read changes when its message lands, as a signal does now.
- Everything the page asks for is a `Command` (most already are); a reply comes back as a
  message to a mailbox.
- The page makes no file calls: everything that reads a file runs in a worker. A file call on
  the page fails loudly instead of spinning.
- `be_wasm_threads::Mutex` is gone and every lock is `std::sync::Mutex`; `initialize_page`
  makes a lock the page finds held panic with a clear message instead of trapping.

## Steps

1. Split `Shared` into what only the worker uses and `Seen`, and make every page function in
   `be-host-stack/src/lib.rs` read `Seen`, which the worker hands over as a whole value after
   each pass and the page takes without a lock. Native hosts go the same way, so host tests
   cover it. The page functions that write `Shared` (`want_versions`, `take_editor_frames`,
   `keep_editor_frames`, `reset`, `try_anyway`, the `HELD_CLOCK` pair) become commands or
   handed-over queues.
2. Answer `app_graph::answer`'s `out_of_reach` in the worker, as part of `Seen`, so the page
   never holds the worker's `Peer`; open the refs database only in the worker, and move the
   page's other file reads behind commands.
3. Send letters between the page and the worker with `postMessage` rather than `POSTED`.
4. Give the stack's worker its own memory, serialize `Seen` and commands (the protocol crates'
   own encodings), and stop binding the page's files channel.
5. Drop `be_wasm_threads::Mutex` for `std::sync::Mutex` across crates/, and the page's spin in
   `web/wasi.js`.

`web/files.js` also polls every millisecond (`setTimeout`) where a browser has no
`Atomics.waitAsync`; a `MessagePort` from each caller would wake it instead.
