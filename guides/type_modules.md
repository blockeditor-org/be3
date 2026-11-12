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

Stateless requests (`blank`, `merge3`, `merge-parts`, `describe`, `to-file`,
`search-text`, `claims`, `apply`) are answered in the step that asks, except
`merge-parts` and `search-text`, which may read parts first (below, "Merging
by parts"). `apply` is how a host that publishes commits rather than joining a
session edits a block (`be sync`, version control).

`search-text` answers a commit's searchable text for the search index
(plans/search.md): `BlockContent::search_text`, whose default answers nothing,
so a type that does not implement it is not content-searchable; a `Document`
answers `Root::search_text_in`. Each `SearchDocument` has a key stable across
edits (empty for a block that is one document), the part its text came from
if it is one, an optional name of its own and its body. The type is handed
`keys` (the documents asked for, a snippet's one) and `indexed` (parts the
index already holds text from) as sets, to spare work: a type that loads only
its first part reads only the parts asked for and not yet indexed (`Sections`
does). The host, not the type, enforces them: `Replicas::search_text` hands
the commit as the type loads it (`type-info.first-part`, `Kind::load`, which
the merges load by too), drops documents outside `keys`, drops a `source`
that is no part of the commit, clears the body of a document whose source is
in `indexed`, and refuses an answer naming a key twice or a document with
neither text nor an indexed source (`NotSearched::Type`; a failed read is
`NotSearched::Read`). Text, in any language, answers one document.

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
- **What the server holds.** `append` answers whether the server took the
  entries, `publish` answers `kept` for a commit sealed on this device alone
  (offline, or behind others in its outbox), `drained` says those commits
  reached the server, or with `refused` lists the commits it refused (set aside, or
  left in the outbox to be tried again), and
  `opening` says whether it opens on this device's own journal (`journal-kept`),
  whether the device still keeps commits of the block (`commits-kept`) and which
  refused ones it holds (`refused-commits`). The replica tells editors from these what the
  server lacks (guides/blocks/apps.md, "Sending").
- **IO.** `load`, `publish` (a compare-and-swap on `expected`), `stash` and
  `resume-session` are async imports the host completes. A replica waits on
  one answer at a time. `load` answers a commit's stored form with the parts it
  was saved in (`loaded`, which becomes a `be_block_store::Stored`), only the
  first part's bytes for a type that loads only that (`be_block::Loads`).
  `read-part` reads a range of a saved part, which a replica answering an
  editor's `Read` uses outside that one-answer rule (guides/blocks/apps.md,
  "Reading a range"). `written` carries the parts a save keeps as they were as
  `reused`, so such a replica saves without the bytes it never loaded.
  `put-part` uploads bytes as a part of the block before an operation names
  it (below, "Parts named by hash").
- **The lease, relaying and reports.** The host passes on the session's lease
  state, relays `send`s to peers or editors, and stores what the replica
  reports (status, description, history, conflicts, errors).

### Parts named by hash

An operation can carry a part by its ref instead of its bytes: the search index's
writer adding one, an edit too large to relay.
The sender calls `put-part(bytes)`, which seals the bytes under the block's
current key, uploads them and has the server hold them for the block
(`hold-objects` with `hold.until-committed`), and answers the `part-ref`
once the upload is done. Only then does it send the operation, with that ref
(a `be_block::PartRef`) in its own payload. The hold ends when a commit of the
block that uses the part publishes, or once `upload_grace` passed and the
block's journal holds no entries (an uncommitted operation may still name the
part). A type's part is put by an editor: `EditorMirror::put_part`
(`FromEditor::PutPart`, answered `ToEditor::PartPut` under its token), which
the replica turns into its `Effect::PutPart`. `LiveEdit` has no hook for a
replica to put a part of its own; the replica puts only an oversized operation's
bytes. (Chat closes a segment without one: the operation carries the segment's
bytes, which every replica keeps in memory, as it holds the operation, until the content is dropped or
reloaded from a save; there is no hook yet that turns them into a saved part, so a replica
holds a closed segment's bytes once per clone, plans/chat.md.) The
block must be on the server: a block made offline, or one whose replica has
not joined yet, is refused.

Each replica that applies the operation keeps the part in its parts as
`Part::Reused`, so whoever commits next saves a manifest reusing it, uploading
nothing (the server holds it, so `missing-objects` names none of it) and
downloading its nodes, never its chunks: its top object to learn its key, and
the rest to list the commit's objects. That needs the server, so an owner
offline cannot seal a commit reusing a part it never fetched. A type that only
keeps the part (the search index) never reads it. A type that can rebuild the
bytes from what it has puts a hash of them in the operation, rebuilds them on applying it
and keeps them when they match, answering reads of the part from
`BlockContent::reused_bytes`; when its encoding differs (another build) it
keeps nothing and reads go to `read-part`. Such a type loads only its first
part (`Loads::FirstPart`), so `encode` never needs the bytes. It takes its
parts' refs from what it loaded (`from_stored`'s `Stored::root`), never from
refs it wrote into its own bytes: after a rotation a save reseals a reused part
under a new ref.

An edit too large to relay is the other kind: the part is the operation's own
bytes, not a part of the content, so the session carries it rather than the
type (below, "Peers"). Hosts: `Peer::put_part`; be-sdk-test's store refuses.
Tests:
`be-sdk-type-guest/src/tests/an_operation_adds_a_part_by_its_hash.rs` (the
flow), be-host-stack's `a_part_an_editor_puts_is_held_for_its_block_once_the_server_has_it`
and be-server's `a_part_held_until_committed_lasts_until_a_commit_takes_it_or_its_grace_passes`.

### Whole bytes

`type-info.whole-bytes` (`BlockContent::WHOLE_BYTES`, true unless a type says otherwise) says the
stored form read whole decodes to the content, so the host copies such a block through its
bytes: Duplicate, export, `be` transfer, version control's re-listing, a revision's preview or
restore. A type that loads only its first part and keeps parts its first part cannot rebuild (chat's
closed segments) sets it false, and the host refuses those paths for a block that has parts after
its first (`has_later_parts`).

### Merging by parts

Outside a session (`Replicas::merge`, which `merge_diverged` and version
control's combine call, and `be sync`) the host merges a block with whichever
request its type has. `merge3` takes each side's whole bytes and answers
`merged`, the Rust `Merge::merge3` over whole values. A type that loads only its
first part and implements `be_block::MergeParts` registers it
(`Registration::merge_parts`, `be_block_types![T { merge_parts }]`), which sets
`type-info.merges-parts`; the host then calls `merge-parts` with each side as
`loaded` (the first part's bytes and the part list, or content not saved yet as
its bytes and no parts) and gets `merged-parts`: a `written` whose `reused`
parts the merge kept as they were, with its references and derived metadata,
and `conflicts`. `MergeParts::merge_parts` gets each side as `from_stored`
decoded it and a `PartReader`; a merged value hands unchanged parts back as
`Part::Reused`, so the commit uploads only what the merge changed. Equal refs
mean equal bytes, but not the other way round (a rotation reseals a part under
a new ref), so a merge that must know reads the bytes. The answer may reuse
only parts of the sides (`Scope::admits`); a merge that reused any other, or
whose reads failed (offline, a missing key), is not written: the read failure
comes back as the `ClientError`.

The in-session merges of a replica (a rejected seal, a head written offline)
still call the synchronous `Merge::merge3` on the values it loaded, so a type
that loads only its first part writes `merge3` over what it holds: a part
unloaded on a side is settled by refs where they settle it (a side holding the
base's part gives way) and otherwise kept as ours with a conflict, never read.
Its `merge_parts` must agree wherever `merge3` decides without a conflict
(`Sections` shares one function, `by_refs`, between them).

**`read-part` under a stateless request.** A call a module makes while serving
a stateless request has that request as its parent (`ops::guest::request()`
with `with_parent`; `be_sdk_type_guest::request_parts()` is the reader). The
host serves only `read-part` there, and only of the parts of the `loaded`
content it handed the request: it asks with `Replicas::ask_reading` or
`Modules::ask_reading` and a `Scope` (`Scope::of` the sides), refuses a read of
any other part with `OUTSIDE_REQUEST`, and refuses every other call. Any
stateless request that hands a module `loaded` content asks this way, and the
scope answers what was read (`Scope::read`, `Merged::read`, `Searched::read`) and the first
read that failed. A scope whose asker stopped waiting goes when its request
completes or its module restarts. `be sync` hands base and theirs as their
commits' first parts and ours as the file's bytes, which have no refs, so a
type settling by refs reads every part theirs changed (and the base's); it
then reads the merged commit's reused parts whole, since it becomes a file.

Tests use the types in `crates/be-sdk-type-testing` (a dev-dependency only):
`Sections`, ten sections each a part after a header of their lengths, and
`Probe`, whose merge and search read a part its content names and make a call
other than `read-part`, so a test sees the host refuse both; its search also
answers keys not asked for, a foreign source and a key twice. be-block-type-host's
`a_merge_over_parts_reads_only_the_parts_each_side_changed`,
`a_stateless_request_reads_only_parts_of_the_commits_it_was_handed` and
`search_text_reads_only_the_parts_asked_for_and_not_yet_indexed` and
`search_text_of_a_type_that_loads_whole_is_narrowed_to_the_keys_asked_for`, and
be-block-sync's `a_type_that_merges_by_parts_merges_edits_on_both_sides`.

## Peers

Peers exchange `wire::Wire`: be-block-session's `SessionMessage`, plus `Tag`
and `Incompatible`. A follower sends its `Tag` (the module's hash) before each
catch-up, and an owner with a different tag answers `Incompatible` and ignores
it, so two builds of a type never edit one session.

The session (`crates/be-block-session`) is owner sequencing with follower
rebase. The rules an agent most needs:

- Every operation the owner sequences goes to the block's journal
  (guides/blocks/client.md, "The journal") through the host's `append`: with
  others in the session at once, relayed by the same call; alone, folded and
  every `APPEND_INTERVAL` (250 ms). The owner commits at pauses: `QUIET_INTERVAL`
  (30 s) with no edits, at most `COMMIT_INTERVAL` (5 min) after it started
  owing, once it appended `JOURNAL_BYTES` (1 MiB), when it closes, and at once
  when a peer of another build greets it or a takeover leaves it owing. Each
  commit `rebase-journal`s, which starts the journal over. While a commit's
  publish is unanswered it keeps taking edits, holding them back from the
  sequence until the answer.
- A replica opening on the journal's base (`opening.journal`) loads its session
  state and replays it: as owner it continues the journal's numbering, as a
  follower it greets with that base and is sent no session state. A follower
  that takes over appends again what it applied since its last sealed head
  (`applied_ops`), or commits at once when it cannot number it.
  `replay` does the same for a reader outside the session. A journal by
  another build is not handed over: `opening.lost-edits` says it held edits,
  or that the server marked them lost since the last commit, and the replica
  reports them lost (`undo-state.lost-edits`).
- **Restarts.** At a commit, once session state passes `RESTART_STATE_BYTES`
  (1 MiB) and the content's size, with no gesture open, while alone or past
  `RESTART_CEILING` times the limit, the owner restarts the session
  (`Sealed { restart }`): every replica reloads the commit, moves what it has
  pending onto it with `LiveEdit::carry_over`, and undo stops there
  (decisions/sessions_restart_when_their_state_outgrows_the_content.md). The
  last owner to leave rebases the journal with no state, so the next session
  starts fresh too.
- An operation is sent as `be_block_session::Payload`: its bytes, or for one
  over `LARGEST_RELAYED_OPERATION` (1 MiB, a large paste) the part it was put
  as (`Payload::Part`, put with the host's `put-part` before it is sent). Its
  sender holds back what it edits after it until the part is put: the owner in
  `held`, a follower in `unsent`. It is sequenced, relayed, journaled and
  undone like any other. A replica reads the part with `read-part` before it
  applies the operation, and holds back every peer message after it until the
  read answers (`Job::Heard`); one opening on a journal reads the parts its
  entries name before it replays them. An owner counts the part's bytes
  towards `JOURNAL_BYTES`, so it commits at once, which ends what a reader
  outside the session misses: `replay` cannot read a part, so it fails and the
  host reads the head alone. No commit holds the part, so the server's hold on
  it lasts the whole upload grace. An owner that cannot put the part (offline,
  or a block the server does not have yet) commits what it shows instead. A
  follower whose put fails keeps the edit and puts it again after 5 s, then
  10, 20 and so on up to a minute (`put_retry_at`), and at once on a lease
  change (the host's `Lease` input), which is what a rejoin looks like; each
  failure is reported.
- A read that fails is tried again after 1, 2 and 4 s, with the messages
  behind it held; the fourth failure gives up on the part. A part longer than
  `LARGEST_PART` is given up on without a read, and so are a follower's held
  messages once more than `MOST_HELD_JOBS` wait behind a read. Giving up
  drops the messages that name the part. An owner reads a follower's part ten times,
  the waits growing to a minute (about 4 minutes in all), holding its whole
  queue meanwhile, then tells the sender `Resend`, which submits all it has
  pending again; a follower obeys three in a row, then reports the edit. A follower asks the
  owner to catch it up (`catch_up_later`), again after 5 s, 10 s and so on up
  to a minute until an answer applies (a `Snapshot`, a `Sealed` that reloads,
  or a lease change that resynchronizes). Once the owner's commit of the edit
  lands, the snapshot starts at a head that holds it; before that it still
  names the part, so the follower waits. Giving up is never a divergence, only
  bytes that do not decode are. A follower heeds `Accepted`, `Snapshot`,
  `Sealed`, `Replaced` and `Resend` from the owner alone and ignores `Submit`,
  `Catchup` and `Kept` (`not_heeded`), before it reads any part they name,
  since the server relays what any participant, a viewer included, sends. One
  from a sender it did not know as the owner is remembered, and it asks to be
  caught up should that sender become the owner. That the
  server does not limit `Submit` and the owner's messages to editors and the
  owner is a known gap (plans/session_message_authority.md). The server holds a
  part for the whole upload grace (a day), and longer while the block's journal holds
  entries, far longer than a lagging replica needs; a replica may read only
  objects of blocks it may view, and opens them with that block's key.
- A whole-content replacement (`Replace`, `FromFile`) wins outright: the
  replica stashes what it has not saved and publishes the replacement with
  that stash as second parent, and every editor gets a new snapshot. A follower
  that has unsaved edits when a replacement lands stashes them and hands the
  owner a `Kept`, which the owner keeps in history; the follower stays
  responsible for each stash until the owner answers `Saved`, resending it
  after a takeover.

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
