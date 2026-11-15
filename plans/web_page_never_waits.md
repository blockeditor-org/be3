# The web page never waits

Depends on: none.

The owner's rule is that nothing spins on a lock. A worker waits for real (`Atomics.wait`), but
a browser traps `memory.atomic.wait` on a page's thread, so the page must never wait at all:
it has to be built so that nothing it needs is ever held by another thread. be-host on the web
is not built that way yet. The page and the block stack's worker (`web/be-worker.js`) run Rust
in one shared memory, and the page reaches the worker's state synchronously, so two waits
remain on the page (guides/running/web.md, "Files and offline"):

- `be_wasm_threads::Mutex::lock` spins on the page. The locks it covers are the ones the page
  and the stack's worker both take: be-host-stack's statics (`changes::WATCHERS` and each
  watch's changes, `offline_choice::CHOICES`, `link::held`, and `stack()`, which the worker
  takes only when a reply to the page calls back into `lib.rs`, as `import_file`'s does), the
  `RefsDb` connection (`be-block-client/src/refs/sqlite.rs`), be-block-type-host's
  `REGISTRY` and `DECLARED`, and be-host-base's `notices`. The stack's own state is not among
  them: the page reads the `Seen` the worker hands over through an atomic slot, and what it
  changes goes as commands (guides/blocks/host.md, "What the page reads").
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
  worker keeps up to date with messages. A reactive read changes when its message lands, as
  it changes now when the worker's handed-over `Seen` lands.
- Everything the page asks for is a `Command` (most already are); a reply comes back as a
  message to a mailbox.
- The page makes no file calls: everything that reads a file runs in a worker. A file call on
  the page fails loudly instead of spinning.
- The page starts no std thread on its own memory either (a `thread.js` worker shares its heap
  as the stack's worker does now), so it holds no lock another thread takes, and every lock is
  `std::sync::Mutex`: `be_wasm_threads::Mutex` is gone.

## Steps

1. Answer `app_graph::answer`'s `out_of_reach` in the worker, as part of `Seen`, so the page
   never holds the worker's `Peer` (`Seen::peer` goes, as `Seen::connection` can once `stop`
   closes it by a command); open the refs database only in the worker, and move the page's
   other file reads behind commands.
2. Send letters between the page and the worker with `postMessage` rather than `POSTED`.
3. Give the stack's worker its own memory, serialize `Seen` and commands (the protocol crates'
   own encodings), and stop binding the page's files channel. The handed-over `Seen` then
   becomes messages: a whole `Seen` first, then the parts that changed (each `Part` the worker
   wrote since the last), and the page's `Edit`s and replies travel as they do now.
4. Drop `be_wasm_threads::Mutex` for `std::sync::Mutex` across crates/, and the page's spin in
   `web/wasi.js`.
