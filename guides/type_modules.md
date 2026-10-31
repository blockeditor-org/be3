# Type modules

A block type is moving out of the app into a plugin's *type module*: a pure wasm
module that holds the device's one replica of every open block of its types (content,
live session, undo stack) and talks to its host only through typed operations. Two crates
define it, and a third hosts it. block-app's peer, `be` and `be sync` host every
block through them (guides/the_new_block_stack.md, "How the app hosts content";
"Hosts" below): each plugin that brings types names its module in its manifest's
`block_types` (guides/adding_a_block.md), and the types the app applies itself
(`crates/host-types`) run in process.

- `crates/block-type-api`: the protocol between a host and a type module, as the WIT
  world `be3:block-type/type-module` (`wit/*.wit`) and the Rust generated from it.
  No GUI types.
- `crates/block-type-guest`: the replica library a be-model-based type module links.
  It holds a block's live session (owner sequencing, follower rebase, takeover, seal
  timing, `seal`/`rekey`) and its undo stack (`be_block::history::History`) as one
  deterministic state machine, and every `Document<R>` and `Blob<K>` works with it
  unchanged. Its `Sim` tests (`src/tests.rs`) are where session behaviour is tested:
  a simulated host with any number of devices, a lease, commits and editors.

## Operations and WIT

WIT is the schema, but there is no component model underneath: everything rides the
pure-guest ABI (guides/plugin_libraries.md, "The raw plugin interface"). The WIT decides how
each function travels. A plain `func` is a direct call: a wasm import (guest to host)
or export (host to guest) of its own, run to completion on the guest's thread before
the caller goes on. An `async func`, a `stream<T>` and a resource's lifetime are
operations ("ops") of the be-ops kernel, which can be awaited and canceled.

- `crates/be-ops`: the kernel. A `Connection` (one per side, `Side::Host` or
  `Side::Guest`) has five frames: `Start { op, parent, method, args }`,
  `Yield { op, item }`, `Complete { op, outcome }`, `Cancel { op }` and `Credit { op, items, bytes }`. Either side
  starts ops, and an op id carries the starting side in its top bit. Every op ends
  with exactly one `Complete`, also after a `Cancel`, which is a request: the callee
  answers `Err(Canceled)` or the real result. An op started with a parent is canceled
  (the callee sees `Incoming::Canceled`) when the parent ends, and one started under a
  parent that already ended is answered `Canceled` by the kernel. Besides a method's own
  errors there are `OpError::Canceled`, `Unsupported` (an unknown method or arguments
  that do not decode) and `Denied`. A method's id is the FNV-1a hash of its name,
  `<package>/<interface>@<version>#<function>`; frames, args and results are postcard,
  each frame with a 4-byte big-endian length in front (`encode_frame`/`decode_frame`),
  stable only within one source tree.
  Direct calls cross the host's link to the guest's thread in the same ordered stream
  as frames, as `block_guest_abi::direct` items (`call`: a name, the parent and the
  args; `returned`: a result), which no frame can be mistaken for: the host's
  `Connection::call_direct` sends one and turns the guest's answer, matched in order,
  into the same `Incoming::Completed` an op ends with, and a sync import the guest made
  arrives as `Incoming::Called`. An empty result is `()`, and a sync func with no
  result is sent with `notify_direct`, which asks for no answer back.
  A stream is back-pressured by a `Window` of items and bytes: the side serving it
  may send while `STREAM_WINDOW` items and `STREAM_WINDOW_BYTES` bytes are not used
  up (an item larger than what is left still goes while any is left), and
  `Frame::Credit` from the consumer gives back what it took. The connection holds
  nothing back: out of credit, `Producer::send` (`Connection::yield_item`) answers
  `WouldBlock` and sends nothing, so the producer stops taking from its own source and
  keeps the item, `Producer::ready` asks first, and `Incoming::Credited { op }` says
  when room came back. A latest-value stream wraps its producer in `Latest<T>`, which
  keeps only the newest value it could not send and sends it on `credited`. The guest
  executor's `Stream` grants credit as its task takes items; a host consuming a stream
  calls `Connection::credit` itself. The other way, an import whose items the guest
  produces is an `async func` per item that the host answers once it has taken the
  item in, and the guest keeps no more than a `Window` of them unanswered.
  `be_ops::guest` is the guest's single-threaded executor: `Executor::step` takes the
  host's items, runs every task they wake until none is ready, and returns the items
  to send, so a step is deterministic and handles one item completely before the next.
  A guest calls an import inside `guest::with_parent(op, ...)` to make the call (op or
  direct) about `op`; dropping a pending call or stream cancels it.
- `crates/be-ops-bindgen`: the generator, a proc macro over `wit-parser`
  (`generate!({ path, world, ... })`, expanded at the crate root). Its
  `path` is read at `CARGO_MANIFEST_DIR`, so the crate passes the files to buck as
  `resources` (guides/buck2.md), and WIT of another crate's package as
  `foreign_resources` (`{"//crates/ui-types:wit": "crates/ui-types"}`) named in
  `deps: ["../ui-types/wit"]`, each package after the ones it uses (block-plugin-api
  lists `../ui-keys/wit` and `../ui-preferences/wit` before `../ui-types/wit`). A
  type another interface `use`s resolves to the interface that defines it, so a crate
  needs a `uses` entry for that interface too even when its WIT reaches the type
  through another package. It emits a module per interface with its types
  (records as structs, variants and enums as enums, flags, `list<u8>` as `Vec<u8>`
  with serde_bytes), deriving `Copy`, `Eq`, `Hash` and `Ord` where every field allows
  (no floats, nothing owned). The options, each a list of WIT type names unless said:
  `with` (a map, keyed by the WIT type's name in any interface) puts an existing Rust
  type in place of a WIT type whose serde data model is the same, which the
  interface's module still names as an alias: a byte array for a tuple of as many
  `u8` (uuids, commit ids, colours), a newtype id for an alias of `u64`. Nothing
  checks that the two agree (one known exception: block-type-api maps `uuid` onto
  `uuid::Uuid`, whose serde writes its 16 bytes with a length in front, which works
  because both ends of a type module's link are block-type-api), so `with` is never for
  a type with fields or cases of its own: generate it, and convert explicitly where Rust code wants another shape (a
  menu's tree, AccessKit's tree update). The one exception is a network world, whose
  client and server are both the crate that generates it: be-protocol maps
  `access-level`, `block-parent` and `escrow-key` onto `be_graph::Access`,
  `be_graph::BlockParent` and its own frozen `EscrowKey`, so the wire uses the very
  Rust types the graph and the sealed escrow bytes use, and a test of be-protocol
  holds the escrow key's bytes; `uses` (a map) takes a whole interface of another package from the crate
  generated from it (`{"be3:ui/types@0.1.0": ::ui_types::types}`); `copy` names the
  mapped types that are `Copy`; `default` the types that derive `Default` (a variant's
  or enum's first case is the default); `inline` records that are a variant case's
  fields, so `case(record)` becomes a struct variant; `lenient` records that take
  `#[serde(default)]`; `ordered` types that derive `Ord` and `Hash` although not
  `Copy`; `own_debug` types whose crate writes `Debug` itself; `local` (see below).
  WIT's own rules hold: a name that is a WIT keyword is written with `%` (`%type(uuid)`
  for a case named `type`, generated as `Type`), and a function may not share its name
  with a type of its interface, so a function that takes a `hello` is not `hello`.
  A package's dependencies may also sit in a `deps/<name>/` folder beside its files,
  which is how an extension package travels with the world that imports it. An
  interface of a package other than the world's is generated under a module named after
  its package, in the types, `guest` and `embed` alike (`shared::ids`,
  `desktop::power`, `guest::desktop::power`), so two packages may name an
  interface alike. Ids and
  access levels have one shape in every package: `be3:shared` (crates/be-ops/wit, its
  buck `foreign_resources` `//crates/be-ops:wit`) holds `uuid` (16 bytes), `hash` (32)
  and their aliases by what they name (`block-id`, `content-type-id`, `commit-id`, ...);
  an id is always a named alias, and a number one side hands out is an alias of an
  unsigned integer named `<thing>-id` in the package that hands it out. For the guest:
  `guest::<import>` functions (a `func` returns nothing and goes through a wasm import
  on wasm32-unknown-unknown and through the executor in process; an `async func` returns
  `Pending<T>`; a `stream<T>` returns `Stream<T>`), and for each export a `Guest`
  trait, a `Guest<Resource>` trait per resource and `dispatch`;
  `guest::executor(guest)` serves them, and `export_<world>!()` makes the wasm exports
  of every sync export (one per function, named like its method, plus
  `plugin_alloc`), which a module's `:wasm` build calls once. For the host: `IMPORTS`,
  the world's sync imports as `(module, name)`, which a runtime links and nothing else;
  `embed::<import>::Call`, one case per import, accepted from an op
  (`Call::accept`, with a `Reply<T>` or `Producer<T>` to answer it) or a sync call
  (`Call::accept_direct`); `accept` answers a method it does not know `Unsupported`, so a host
  with several imported interfaces asks `Call::accepts(&started)` of each first; and `embed::exports::<export>`
  functions and resource handles that return `Ticket<T>`s to decode the completion
  with. A resource's constructor is a long-lived op whose id is the handle; its async
  methods are ops with it as parent, its sync ones direct calls naming it, and
  `resource_drop` cancels it. A resource named `*-watch` whose only functions are a
  `constructor()` and `next: async func() -> T` is a latest-value watch: the guest's
  handle implements `be_ops::Watched` (its `Value`, its interface, the two method ids,
  `open()` and `next()`), and `embed::WATCHES` lists every watch the world imports as a
  `be_ops::Watching`, so a host can serve a watch by its method ids and an editor follow
  any of them with one accessor (block-editor-beui's `editor.watch::<W>()`). The shapes follow wit-bindgen's, so moving to it later
  changes little: the guest traits take `&self` of a guest instance instead of
  statics, since tests run many modules on one thread, and the constructor lives on
  `Guest` as `new_<resource>`, handed its op id.

The rule, so that native, the web and in process behave the same: a sync import
carries no result. The guest's thread answers it `()` at once and forwards it to the
host in order with the frames, since the host's state is not reachable from the
guest's worker on the web; an import that needs an answer is an `async func`, and
the generator refuses a sync import with a result. A sync export is a call the host
always awaits: every runtime runs it on the guest's thread and sends its result back
after the frames it made.

The one exception is an interface whose host runs inside the guest's own worker or
thread on every platform, as the GPU device does (block-gpu-shim on the web, wasmtime
host functions natively). `local: ["be3:gpu/device@0.1.0"]` names such imports: they
are no ops, have no async funcs, and each func, resource method, constructor and
`[resource-drop]` is a wasm import of its own that returns its result. Scalars, enums,
flags, handles and records and options of them are flattened into wasm arguments,
strings and lists of scalars or handles go as a pointer and a length, and anything
else as postcard; a result is one wasm value (`option<own<R>>` is the handle, 0 for
none) or postcard written into room the guest offers, with `[returned]` fetching one
that did not fit. A `list<u8>` result is raw bytes rather than postcard: the guest offers
no room, learns the length, and has `[returned]` write the bytes straight into a vector
of that length, so a large read is copied once into the guest. The guest gets an owner type per resource that drops it; the host a
`Host` trait, a `Caller` trait for the guest's memory and the device, `glue`
functions, and `<world>_local_imports!`, which hands a macro of the runtime's every
import's glue, module, name, `module#name` and wasm signature to link or export. A
world with other imports of its own takes a local interface from its crate with
`uses` and also names it in `local`, and generates nothing for it.

`network: true` generates a world for a client and a server that talk over a
socket instead of a guest and its host: `crates/be-protocol`'s network protocol. Such
a world only imports, has no local interfaces and no sync funcs (the generator refuses
them, since a direct call runs on a wasm guest's thread): only `async func`s,
`stream<T>`-returning funcs and resources (a constructor, async methods and streams).
It generates the types, `embed::<import>::Call` for the server as above, and
`client::<import>` for the client instead of `guest` and the exports: a function per
func that takes `caller: &(impl be_ops::net::Caller + Clone)` and returns a
`be_ops::net::Pending<T>` (an `async func`; a `Send` future of `Result<T, OpError>`)
or a `be_ops::net::Stream<T>` (`recv()`, and `futures_core::Stream`; it grants credit
as it is read like the guest's), and a struct per resource whose `new(caller, ...)`
opens it, whose methods start ops under it, whose `closed()` awaits its end and whose
drop cancels it. Dropping a `Pending` or `Stream` cancels it too, so nothing in user
code names an op. `Caller` is the stubs' seam (start an op, poll what arrives for it,
grant credit, cancel), and `be_ops::net::Peer` implements it: one side's `Connection`
behind a mutex, made with a sink it hands each outbound frame to (under its lock, so
in order; the sink must not call back into the peer), fed each inbound frame with
`receive`, which keeps what belongs to calls started through `Caller` and returns the
rest of the events (`Started`, and `Canceled`/`Credited` of what this side serves),
and `close`d when the socket goes, which ends every call `Canceled`. The client is
`Side::Guest` and the server `Side::Host`. The server answers from any thread or task
through `peer.lock()`, a guard that derefs to the `Connection` and sends what it queued
when dropped: `Call::accept(&mut peer.lock(), started)`, then in a spawned task
`reply.send(&mut peer.lock(), &value)`, `stream.send(...)` after
`peer.writable(stream.op()).await?` (which waits for credit and fails once the op is
canceled), and `peer.canceled(op).await` to stop work, or end a resource, that the
client dropped. Never hold the guard across an await. A `Canceled` event may name an op
already answered (a parent's end cancels its children when the server completes it,
and the event comes out of the next `receive`), so a server ignores one it does not
know. crates/be-ops's `network` tests run both halves over an in-memory pipe.

## The protocol

The world: the module imports `host` and exports `replicas`
(`crates/block-type-api/wit`).

A pure guest has no entropy of its own, and a type module needs ids for the
items its child edits add. The host calls `replicas.start(entropy)` first, which
answers the module's types (content type, frozen, schema fingerprint, undo, plain
form), and on wasm32-unknown-unknown the library is getrandom's custom backend, a
generator seeded from it (`entropy.rs`; buck sets the `getrandom_backend` cfg for
that plan). be-store's sealing (`vault`), be-commit's `CommitStore` and be-session's
`resume` are default features a guest's dependencies leave off, so a module imports
only `be3_host`'s functions and the world's sync imports.

Stateless requests are sync funcs of `replicas`, direct calls: `blank`, `merge3`
(a `none` side is the type's default; `conflicts: 0` is clean), `describe` (name,
derived metadata, references in an organization), `to-file` (a `plain-file`: the
bytes, the extension and media type the type gives them, and the name of the file
they came from when the content keeps one), `claims` (which of the module's types
takes a file by name and first bytes, or with `directory` a new directory), and
`apply` (a `child` or `from-file` change made to content bytes, or to the type's
default for `none`, answered with the new bytes, or none when it does not apply),
which is how a host that publishes commits rather than joining a session edits a
block: be sync, and block-app's version control when it lists a checkout in its
parent (`Replicas::apply`, then `describe` and `publish_encoded`). An unknown type
is `failure.unknown-type`, content that does not decode `failure.malformed`.

Each open block is a `replica` resource. Its methods `change` (`child`, `from-file`,
`replace`, `undo`, `redo`; false when it did not apply), `content` (the visible
bytes, for export and duplication; none while unreadable) and `seal` (`force` is a
rekey: publish even when clean) are async; the rest are direct calls. A call on a
replica that has ended is `Canceled`. Every call the module makes for a replica has
that replica as its parent, which is how the host knows the block.

What the host owes a replica:

- **Time.** The module starts `host.clock` once, a `stream<u64>` of the host's
  `be_client::Clock` in milliseconds (`Modules::new` takes it; an editor's frames are
  stamped with the frame clock's instant, which `Clock::at` places on that clock), and
  the host yields the time on it whenever the clock moved, ahead of any other frame.
  `host.sleep-until(at)` asks to be woken: the host yields a time at or after `at` and
  then completes it; the module cancels it when it wants another time. The replica
  never reads a clock, so a test moves time by yielding.
- **Opening.** The constructor's `opening` carries the head the host just read, the
  session's lease state, the client id and a `session-tag` (the module's hash). The
  replica reads the head itself (`read`) and queues everything until it arrives. The
  host must call `head-moved` for every head that lands after the one in `opening`
  (including the remote head, when it opened at a local head it differs from), since
  the replica does not reconcile on opening. `reopen(opening)` starts the replica over
  after the host reconnected or joined late; the host replays what it kept (its
  editors' messages and its own `change`s) and asks again whatever `change`,
  `content` or `seal` was still unanswered, handing the new answer to the old caller.
- **IO answers.** `read`, `publish` (`published` or `rejected` with the current head;
  an unchanged save is `published` with the same head), `stash` and `resume-session`
  (the host fetches both histories and runs `be_session::resume`; the replica cannot
  see history) are async imports, answered by completing them. A replica waits on one
  answer at a time and queues every other call behind it.
- **Publishing.** `publish(expected, merging, content)` is a compare-and-swap on
  `expected`, whose commit is the first parent. `merging: some { base, ours }` adds
  `ours` as a second parent and asks the host to merge defined metadata
  (`BlockMetadata::merge_defined` of `base`, `expected`, `ours`); otherwise defined
  metadata comes from `expected`. `stash(parent, content)` writes a commit that moves
  no head. `written` carries references unresolved and derived metadata; the host
  resolves, seals and uploads.
- **The lease.** `lease(session, resynchronize)` whenever the server's session state
  changes (`resynchronize` after the host fell behind the broadcast). `report-clean`
  is the heartbeat with `clean-at`; `claim-lease` is called once per generation when
  the owner is gone from the participants. Dropping the replica closes it: it seals
  what it owes and then completes its constructor op, after which the host leaves the
  session; calls for it until then still reach it.
- **Relaying.** `send(to, bytes)` is opaque bytes for `one(peer)`, `peers` (every
  other participant), `one(editor)` or `one(host)`. The host seals and relays peer
  bytes and delivers them as `message(peer(client), bytes)`.
- **Reports.** `report-status` (head, owner, clean, owes a seal, incompatible,
  diverged, unreadable), `described` (after the visible content changed, for the graph
  mirror), `history` (can undo or redo), `conflict` (a merge kept both sides),
  `error`. Each is sent when it changes.

## Peers

Peers exchange `wire::Wire`: be-session's `SessionMessage`, `Tag` and
`Incompatible`. A follower sends `Tag` (its `SessionTag`) ahead of each `Catchup`;
an owner answers another tag with `Incompatible` and then ignores that peer's
submits and catch-ups, and the follower marks itself incompatible and diverged, so
two builds of a type never edit one session. The session itself is owner
sequencing, follower rebase and `apply_beneath`, the 1 MiB relay limit (a follower's
queued edits are absorbed into pieces no larger), `Snapshot`/`Sealed`/`reload`,
`Replaced`, `Moved`, `Kept`/`Saved`, takeover with `Sequencer::continuing`, presence
and channel messages. Its rules: an owner that owes a seal seals `SEAL_INTERVAL`
(750 ms) after it started owing; while a seal's publish is unanswered the
replica goes on taking its editors' edits, child changes, undo and redo (they show
at once, held back from the sequence so peers hear `Sealed` before them, and are
sequenced and sealed next once the answer comes), and only what could move its
base waits; a rejected seal reconciles and seals once more;
reconcile merges unsaved work into whatever it fast-forwards or merges onto and
stashes our side when the merge conflicts; a head moved from elsewhere reconciles an
owner, and one this device published outside the session (`from_this_device`) sends
a follower's owner `Moved`, which it reloads with its unsealed operations on top.

A follower hands the owner a commit with `Kept { commit, base, edits }`: `commit` is
a stash, and `edits` (`KeptEdits`) says whether its content is to be merged in. The
follower stays responsible for every stash it sent until the owner answers `Saved`:
after a takeover it sends them again to the new owner, or as the new owner publishes
them itself. An owner remembers what it saved, so a repeated `Kept` is only answered.

An edit too large to relay never goes through the session. An owner publishes it as
a commit. A follower queues it behind whatever it has in flight, and once nothing
is, stashes its content (on top of a stash of its content as of its last applied
sequence, unless that is its sealed head) and sends `Kept` with `edits`: that
sequence and its sealed head. When the owner's log still holds every operation after
that sequence and that head is one of its seals since its last reload, it takes the
stash and replays those operations except the follower's own; otherwise it merges
the stash with the follower's ancestor as base. Either way it publishes the result
with the stash as second parent, so each edit lands once. Until `Saved` the follower
skips reloads, sending its `Kept` again each time.

A whole-content replacement (`Change::Replace`, `FromFile`, `FromEditor::Replace`)
wins outright. The replica that does it first stashes what it has not saved (an
owner's unsealed content; a follower's unconfirmed edits and the operations it
applied since its last seal, which only it may hold if the owner is gone) and
publishes the replacement with that stash as a second parent. An owner that replaced
moves its sequence on by one and drops any `Submit` based before it, and sends its
followers `Replaced`; a follower with anything unsaved stashes it and hands it to
the owner with `Kept`, which the owner publishes as the second parent of a seal. A
follower that replaced sends the owner `Replaced`, and the owner, unless it already
took that head with nothing unsaved, publishes the replacement's content again on
top of whatever it holds, the same way, so a reconcile that merged its unsaved work
into the replacement first is overruled. If the owner changes before answering, the
follower sends the new owner `Replaced` again, or as the new owner publishes its
content as an owner's replacement. A follower waiting on a replacement holds its
edits back and ignores operations and seals until the owner's `Sealed` for that
head. Every editor is sent a snapshot of the replacement.

## Editors are endpoints

A local editor follows the local replica as a peer follows an owner. The host calls
`attach(editor, editable)` when an editor instance starts following a block
(again to change `editable`) and `detach` when it stops; the editor's messages arrive
as `message(editor(instance), bytes)` and are `editor::FromEditor`: `Operate`,
`Gesture` (`Begin`, `Commit`, `Cancel`), `Replace`, `ShowPresence`, `SendToPeers`,
`Resend`. A replica drops edits from an editor attached read-only. It answers with
`editor::ToEditor`:

- `Snapshot { bytes, session, unsettled, applied }` on attach, on `Resend`, and to
  every editor after the content was replaced outright. `bytes` and `session` are the
  settled content, `unsettled` the follower's operations on top of it, and `applied`
  how many of that editor's own operations the snapshot already holds.
- `Operations`, every operation since the last message, each with `mine`, the number
  of operations still unsettled after it, and for a peer's operation that goes beneath
  them, the unsettled operations as rebased (`beneath`).
- `Presence` and `PeerMessages` from the peers.

On the plugin protocol both directions travel through the editor's `replica` resource
(guides/the_new_block_stack.md, "Content on the plugin protocol"). The replica keeps no log either:
the replica keeps `projected`, a `Settling` of what its editors hold, lands every
journaled change in it and snapshots when one cannot land. `editor::EditorMirror` is
the editor's side, which `ContentProjection` is built on: the settled content, the
pending operations, what each update touched, a `revision()` count and the
`Projected` log (`record`, `take_projected`) a mirrored copy such as `LiveText` reads. Undo and redo are host `change`s, not editor
messages, since a shortcut or menu may undo a block no editor is attached to; a
gesture is an editor message because it groups that editor's own edits.

## The library

```rust
be_block_types![ChecklistContent { undo }, TextContent { undo, file }, FolderContent];
// expands to: pub fn registrations() -> Vec<Registration>,
//             pub fn block_types() -> block_type_guest::Module,
//             pub struct BlockTypes (the guest)
#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);
```

Each entry is `Registration::of::<C>()` (`C: LiveEdit + Merge + Clone + Default`),
then `.undo::<C>()` for a type whose session keeps an undo stack (what
`undo` in a module's list) and `.file::<C>()` for one with a plain-file form
(`FileForm`: `to_file`, `from_file`, `claims`). A types crate cannot implement
`FileForm` for `Blob<Kind>` or `Document<Root>` itself (the orphan rule), so it
implements `BlobFile` on its blob kind (`header` for a file's name and bytes,
`media_type`, `claims`) or `DocumentFile` on its root (`to_file`, `file_type` for
its extension and media type, `from_file`, `claims`), and
the library's blanket impls make that the content's `FileForm`; for the same
reason a blob type's constructors are associated functions of its kind
(`ImageFile::from_file`). `be_block::plain` has the file-name and media-type
helpers they share. `directory` makes a
type's plain form a directory that takes new directories (a folder), and `root`
one that is only ever the root of one (a workspace); `TypeInfo::plain` says which
form each type has. The order is the order `claims` asks in. A type that takes any
file (the workspace plugin's file type) sets `ANY_FILE` on its `BlobFile` or
`DocumentFile`, and answers only a `claims` with `fallback`: a host asks every
module without it first, then every module with it (`Modules::claim_requests`,
which both `be sync` and block-app's import of a picked file follow, so one rule
decides what a file becomes).

`Module` implements the generated `guest::exports::replicas::Guest` (its stateless
functions are callable directly, as the types crates' tests do), and its `OpenReplica`
is the `GuestReplica`: the boundary turns each call into an input of the replica's
state machine and each of the machine's effects (`effect.rs`) into an import call
parented to the replica, feeding the answer back in as an input, so the state
machine itself never sees the protocol. `register` adds or replaces a type and
`knows` asks for one, as block-ui-test does for a test's own types; `types()` is what
`start` answers. `Module::executor()` is its `be_ops::guest::Executor`, and
`Serving` holds both, for a guest. The macro also defines `BlockTypes`, a
`block_guest_abi::Guest` over a `Serving` (`step` steps the executor). It does not export it: a types crate is a plain library
that editors, tests and other crates depend on, and only its `:wasm` build, with
the crate's `module` feature, calls `type_module!` (`block_guest_abi::guest!` and the
world's `export_type_module!`), since a second set of guest exports in one wasm (an
editor's own, or another types crate's) would clash. So a type module crate is
that macro call, the export line and guest-abi's `cdylib` and BUCK set-up
(`crates/editors/checklist/types`).
`Registration::without_undo` drops a registration's undo stack, which is how
block-ui-test hosts every type (`ContentStore::register` takes a types crate's
`registrations()` for a test that needs a plugin's types it never names). The
waker is unused: the replica asks for time with `sleep-until` and the host yields
it on the clock.
The undo stack is `History<C, u64>`: `History` is generic
over its clock (`history::Moment`, for `Instant` and frame-clock milliseconds), and
`Recorder` is what a replica asks of it, with `NoHistory` for a type without undo.

## Tests

`crates/block-type-guest/src/tests.rs` is a simulated host: `Sim<C>` runs several
devices, each with its own `Module`, executor and host `Connection`, delivers every
frame in one queue in the order it was sent, keeps commits and the head with
compare-and-swap, answers `resume-session` from the commit graph, runs a be-session
`Lease`, follows each attached editor with an `EditorMirror`, and moves time only
through `advance`, which yields it on each device's clock. `Lone` is one module
and a host connection for a test that looks at the calls a module makes. `quiet` holds back `Moved` so a seal is rejected;
`crash` drops a device without its seal; `publish_elsewhere` is a publish from
outside the session, as be sync makes.

## Hosts

`crates/be-types-host` is the host half every host shares, natively under
wasmtime and in process in tests:

- `modules`: the registry (`kind_of`, `is_known`, `plain_of`: each type's
  `Kind`, filled from the types every module's `start` answers, with its blank
  content and description; process-wide, since it holds only what a type is), and
  `Modules`, which routes each type to the module that listed it (`module_of`, its
  own, so two hosts in one process with different sources each reach their own
  instance), starts each `Source` (`in_process::<G>`, natively
  `wasm(name, tag, cache, load)` on a `PureHost` thread, on the web
  `web(name, url, tag)`) with a host `Connection` of its own, restarts one that
  stops, answers its clock and sleeps itself, and hands everything else on as
  `Heard` (`call` runs a closure on a module's connection after yielding the time).
  A source that declares its types starts lazily: `Modules::new` routes and marks
  known (`is_known`) what it declares and starts only sources that declare nothing,
  and the first `call` to a module starts it, so the app runs only the modules of
  types its blocks use. What a module's `start` answers (`kind_of`: frozen, schema,
  plain form, blank, description) is there only once it ran: `load(module)` starts it
  and awaits that, `Replicas::kind` does so for a content type, and a `Hello` heard
  later (a module a `call` started) is registered as it is drained. What a host must
  know without running a module comes from the manifest: names, icons and colors,
  and `plain_file`. `started(sources)` starts every module and waits for every
  `start`, as `be` and `be sync` do. Its stateless helpers (`ask`, `blank`,
  `merge3`, `to_files`, `claims`, `apply`, `describe`) are what a host that holds no
  replica uses.
- `replicas`: `Replicas<H: Host>`, the replica IO of
  guides/the_new_block_stack.md ("The peer and its registry") for any host.
  `Host` is what the host does with what a replica reports (its error log,
  frames for an editor, status, description, history, conflicts) and how it
  spawns IO, each an associated function of the host's shared state.

Where the modules come from: block-app stages each plugin's type module (and its
`.cwasm` and `.sha256`) beside the plugin; `be` (and so `be sync`) carries every
plugin's manifest and type module in its executable as one bundle
(`PLUGIN_TYPES`, buck/app's `type_bundle`, read by `be_cli::plugins`), so its
release stays the one file guides/bootstrap.md pins, compiles them once into
`BE3_CACHE/modules` through wasmtime's cache, and runs one module per plugin
that has one, as the app does; the manifests also name the
types its own short names (`blocks::type_name`) do not cover. A host gives each
plugin's `Source` the types its manifest declares (`Source::declaring`): `Modules`
registers only those of the types that module lists, and no module takes a type
another declares.
Its session tag is the module's SHA-256, as the staged `.sha256` is, so a `be`
and an app built from one tree share sessions. Tests and examples of a host run
every first-party types crate in process with `first_party_types::sources()`
(`crates/first-party-types`), named by plugin id and declaring each module's types.
