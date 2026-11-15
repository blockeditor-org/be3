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
  them: the page reads the `Seen` the worker hands over in bytes through a lock-free stack,
  and what it changes goes as commands (guides/blocks/host.md, "What the page reads").
- The page's file calls (`web/wasi.js`, `forward`) spin until `web/files.js` answers. The page
  makes none as it starts and runs any more: the host's worker keeps the host state and the
  refs database's organization list from start-up, and the page reads the copy it hands over
  (decisions/web_host_state_in_a_worker.md); the stack's worker reads everything else. What
  is left is to make a file call on the page fail loudly rather than spin (step 1), so none
  comes back unnoticed.

Other locks the page shares are std's own and would trap rather than spin if the page ever
found one held past std's short spin:

- be-host-base `host.rs`'s `UNSENT_SAVES`, which `save_file` pushes to and the host's frame
  takes from. Only be-host-runtime's instances call `save_file` today, answering an app's
  `save-file` on the page, so no worker takes it yet; a worker that saves a file would.
- be-host-stack `replies.rs`'s `WAITING`, what each reply's number calls back: the page adds
  to it and the worker takes from it as it answers. It becomes the page's alone once answers
  travel to the page in the message that hands it the `Seen` (step 1).
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

1. Give the host's worker (`web/be-worker.js`, which runs the stacks and keeps the host's
   files, be-host's `host_files`) its own memory, serialize `Seen` and commands (the protocol
   crates' own encodings), and stop binding the page's files channel: a file call on the page
   then fails loudly. be-host-runtime's two file reads on the page, `input.rs`'s `read_path`
   and `fonts.rs`'s fallbacks, read nothing on the web already. The host state's first answer and its `Write`s, and the organization
   lists, become messages too. The stack's commands are bytes already, and replies numbers
   answered with bytes, and the `Seen` is handed over in bytes, whole first and then what
   changed by key (guides/blocks/host.md, "What the page reads"): what is left is for those
   bytes to travel by `postMessage` rather than through the shared heap. A
   worker's letter to a page mailbox already travels by `postMessage` (`be_host_base::host`'s
   `letters_web.rs`), but as the address of a value in the shared heap, which the page takes
   and frees: with a memory of its own, the worker sends the letter's bytes instead. The page
   then applies the messages once, at the start of a frame (be-host's frame and each app
   runtime's), rather than taking the newest `Seen` on every read as it does now, so every
   read in a frame reads one snapshot.
   An update sends whole what is not a persistent map, and a map's changed entry whole: an open
   block's `Content` on each of its edits, the outbox and the owing lists on each change, and
   an editor's queued frames. Send content as its edit (the block's own operations or a byte
   delta), or keep a value's `Arc` identity across updates so one the worker did not replace
   is never sent again.
   `Seen` holds three stand-ins for what it carries of the worker's, kept apart only while
   another change (#686) rewrites the files they come from: `Owed` for be-block-type-host
   `replicas.rs`'s `Owing`, `Unsent` for be-block-client `refs.rs`'s `UnsentBlock`, and
   `Words` for the `&'static str` reasons and steps the worker writes. They are temporary:
   once #686 lands, give `Owing` and `UnsentBlock` serde and use them in `Seen`, and make each
   reason an owned string or an enum where it is made, so each concept has one type.
2. Drop `be_wasm_threads::Mutex` for `std::sync::Mutex` across crates/, and the page's spin in
   `web/wasi.js`.
