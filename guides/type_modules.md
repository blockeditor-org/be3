# Type modules

A block type lives in its app's *type module*: a pure wasm guest
(guides/app_libraries.md, Pure guests) holding the device's one replica of
every open block of its types (content, live session, undo stack), which
talks to its host only through typed operations. be-host's peer, `be` and
`be sync` hold every block through these modules (guides/blocks/host.md). An app names its module in its manifest's
`block_types` (guides/adding_a_block.md); the types be-host applies itself
(`crates/be-sdk-type-host`) run in process.

- `crates/be-sdk-type-api`: the protocol, the WIT world
  `be3:block-type/type-module` (`wit/*.wit`) and the Rust generated from it.
- `crates/be-sdk-type-guest`: the replica library a type module links. It holds
  a block's live session and undo stack as one deterministic state machine
  that every `Document<R>` and `Blob<K>` works with unchanged.
- `crates/be-block-type-host`: the host half every host shares (Hosts, below).

## Operations and WIT

Every protocol in the repository (apps, type modules, the GPU, the network
protocol) is written as WIT, but there is no component model underneath:
guests ride the pure-guest ABI (guides/app_libraries.md, The raw app
interface) and the WIT only decides how each function travels
(decisions/guest_calls_are_async_ops_except_local_ones.md):

- An `async func`, a `stream<T>` or a resource is an *op* of the be-wasm-ops
  kernel: postcard arguments in a frame, answered later by whoever holds the
  other `Connection`, and cancelable.
- A plain `func` returns into the caller synchronously, which only a host on
  the guest's own thread can answer, so it is allowed only in a `local`
  interface (the GPU); the generator refuses it anywhere else.

### The kernel

`crates/be-wasm-ops`: a `Connection` per side exchanges five frames (`Start`,
`Yield`, `Complete`, `Cancel`, `Credit`). Every op ends with exactly one
`Complete`, also after a `Cancel`, which is only a request: the callee answers
`Canceled` or the real result. An op started under a parent is canceled when
the parent ends. Frames are postcard with a length prefix, stable only within
one source tree.

Streams are back-pressured by a window of items and bytes; out of credit the
producer's send answers `WouldBlock` and the producer keeps the item. A host
serving many streams routes items through `Outlets`, choosing per stream
whether to keep only the latest value or a bounded queue.

`be_wasm_ops::guest` is the guest's single-threaded executor: a step takes the
host's frames, runs every task they wake until none is ready, and returns
the frames to send, so a step is deterministic. A call has happened once the
generated function returns (it queued the `Start`); dropping its `Pending`
only asks to cancel. `Pending` is `#[must_use]`: `.detach()` says nobody
waits for the answer. An import that only informs the host returns nothing
and is detached.

### The generator

`crates/be-wasm-ops-bindgen` is a proc macro over `wit-parser`
(`generate!({ path, world, ... })`). It reads `path` at `CARGO_MANIFEST_DIR`,
so the WIT files are buck `resources`, and another crate's package is a
`foreign_resources` entry named in `deps` (guides/buck2.md); see
be-sdk-api's BUCK and `lib.rs` for a crate that uses several packages. Its
options (`with`, `uses`, `copy`, `default`, `inline`, `lenient`, `ordered`,
`encodings`, `local`, `network`) are documented on the macro.

It generates, per interface, the types, and:
- for the guest, `guest::<import>` functions returning `Pending<T>` or
  `Stream<T>`, and a `Guest` trait per export taking `&self` (not statics, so
  tests can run many modules on one thread);
- for the host, `embed::<import>::Call` (one case per function, accepted with
  a `Reply` or `Producer`) and a world-wide `embed::Call`, so a host that
  serves every import matches once.

Traps:
- A `Reply` dropped unanswered answers `Unanswered`, so a forgotten answer is
  an error the guest sees rather than an op left open.
- A resource named `*-watch` with only `constructor()` and
  `next: async func() -> T` becomes a latest-value watch
  (`be_wasm_ops::Watched`), which editors follow with `editor.watch::<W>()`.
- `with` substitutes an existing Rust type for a WIT type, and nothing checks
  that their serde forms agree; use it only for byte arrays and newtype ids.
- Ids and access levels have one shape everywhere: `be3:shared`
  (`crates/be-wasm-ops/wit`) holds `uuid`, `hash` and their named aliases.
- WIT keywords are written with `%`, and a function may not share its name
  with a type of its interface.
- Every `list<u8>` travels as one run of bytes through a `be_wasm_ops::wire`
  encoding, never byte by byte through serde; code generic over such a call
  takes the encoding as a type parameter. A hand-written serde type on these
  wires uses `serde_bytes` for its byte fields.

### Local interfaces

An interface whose host runs inside the guest's own worker or thread on every
platform (the GPU device: be-wasm-gpu-shim on the web, wasmtime host functions
natively) is named in `local`. Its functions are plain wasm imports returning
their results: scalars and records of them flattened into wasm arguments,
strings and lists as pointer and length, anything else postcard. A
`list<u8>` result is copied once, straight into a vector of the right length.

### Network worlds

`network: true` generates a client and server that talk over a socket
(`crates/be-block-protocol`): only async funcs, streams and resources;
`client::<import>` functions over any `be_wasm_ops::net::Caller`, with
`net::Peer` as the usual one. A server answers through `peer.lock()`, a guard
that sends what it queued when dropped; holding it across an await blocks
every other task on that peer. A `Canceled` event may name an op already
answered, so a server ignores ones it does not know.

## The protocol

The module imports `host` and exports `replicas`. `replicas.start(entropy)`
comes first: it answers the module's types, and seeds the module's randomness,
since a pure guest has none.

Stateless requests (`blank`, `merge3`, `describe`, `to-file`, `claims`,
`apply`) are answered in the step that asks. `apply` is how a host that
publishes commits rather than joining a session edits a block (`be sync`,
version control).

Each open block is a `replica` resource; every call the module makes for it
has the replica as parent, which is how the host knows the block. What the
host owes a replica (the WIT documents each call):

- **Time.** The replica never reads a clock. The host yields its clock on
  `host.clock` whenever it moves, and stamps each `message` with `at`, since
  the clock stream is credit-limited and can arrive after a later message. A
  test moves time by yielding.
- **Opening.** The host calls `head-moved` for every head landing after the
  one in `opening`; the replica does not reconcile on opening. After a
  reconnect, `reopen` starts the replica over and the host replays what it
  kept and re-asks what was unanswered.
- **IO.** `read`, `publish` (a compare-and-swap on `expected`), `stash` and
  `resume-session` are async imports the host completes. A replica waits on
  one answer at a time.
- **The lease, relaying and reports.** The host passes on the session's lease
  state, relays `send`s to peers or editors, and stores what the replica
  reports (status, description, history, conflicts, errors).

## Peers

Peers exchange `wire::Wire`: be-block-session's `SessionMessage`, plus `Tag`
and `Incompatible`. A follower sends its `Tag` (the module's hash) before each
catch-up, and an owner with a different tag answers `Incompatible` and ignores
it, so two builds of a type never edit one session.

The session (`crates/be-block-session`) is owner sequencing with follower
rebase. The rules an agent most needs:

- An owner seals `SEAL_INTERVAL` (750 ms) after it starts owing a seal. While
  a seal's publish is unanswered it keeps taking edits, holding them back from
  the sequence until the answer.
- An edit too large to relay (over 1 MiB) is never relayed: an owner
  publishes it, and a follower stashes it and hands the owner a `Kept`. A
  follower stays responsible for each stash until the owner answers `Saved`,
  resending it after a takeover.
- A whole-content replacement (`Replace`, `FromFile`) wins outright: the
  replica stashes what it has not saved and publishes the replacement with
  that stash as second parent, and every editor gets a new snapshot.

## Editors are endpoints

A local editor follows the replica as a peer follows an owner. The host
`attach`es an editor instance (with whether it may edit) and `detach`es it;
the editor's messages are `editor::FromEditor` (operations, gestures,
replace, presence) and the replica answers `editor::ToEditor` (a snapshot on
attach, then operations and peers' presence). A replica drops edits from an
editor attached read-only. On the app protocol both directions travel
through the editor's `replica` resource (guides/blocks/apps.md, Content on the app protocol); `editor::EditorMirror` is the editor's side,
which `ContentProjection` is built on. Undo and redo are host `change`s, not
editor messages, since a shortcut may undo a block no editor shows.

## The library

`be_block_types![ChecklistContent { undo }, TextContent { undo, file }, ...]`
in a types crate defines `registrations()`, `block_types()` (the `Module`) and
`BlockTypes`, the guest, which only the crate's `module` feature exports with
`type_module!`: a types crate is also a plain library editors depend on, and
two sets of guest exports in one wasm would clash
(guides/adding_a_block.md, 1).

A plain-file form is `FileForm`. A types crate cannot implement it for
`Blob<Kind>` or `Document<Root>` (the orphan rule), so it implements
`BlobFile` on its blob kind or `DocumentFile` on its root and the library's
blanket impls do the rest. A type that takes any file (the workspace's)
sets `ANY_FILE` and is asked only after every other module
(`Modules::claim_requests`, which `be sync` and be-host's import share).

`Module`'s boundary turns each call into an input of the replica's state
machine and each of its effects (`effect.rs`) into an import call, so the
state machine never sees the protocol.

## Tests

`crates/be-sdk-type-guest/src/tests.rs` is where session behaviour is tested:
`Sim<C>` runs several devices, each with its own module and host connection,
delivers frames in order, keeps commits with compare-and-swap, runs a lease,
follows each editor with an `EditorMirror`, and moves time only through
`advance`. `quiet`, `crash` and `publish_elsewhere` inject the failures.
`Lone` is one module for looking at the calls it makes.

## Hosts

`crates/be-block-type-host` is the host half, natively under wasmtime and in
process in tests:

- `modules`: the process-wide registry of what each type is (`kind_of`), and
  `Modules`, which routes each type to its module, starts and restarts each
  `Source` (in process, wasmtime, or a web worker), and answers clocks and
  sleeps. A source that declares its types starts lazily, on the first call
  to it, so a host runs only the modules its blocks use; what a host must
  know without running one (names, icons, `plain_file`) comes from the
  manifest. `started(sources)` starts all, as `be` and `be sync` do. A
  module's types are taken only if its manifest declares them
  (`Source::declaring`).
- `replicas`: `Replicas<H: Host>`, the replica IO of
  guides/blocks/host.md (The peer and its registry) for any host.

be-host stages each app's type module beside the app. `be` carries every
app's manifest and type module inside its executable (`APP_TYPES`), so its
release stays one file (guides/bootstrap.md), and compiles them once into
`BE3_CACHE/modules`. A module's session tag is its SHA-256, so a `be` and a
be-host built from one tree share sessions. Tests run every first-party types
crate in process with `be_type_bundle::sources()`.
