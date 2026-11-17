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

A type module is pure Rust and small. It builds for
`wasm32-unknown-unknown` and runs with no WASI (`be_wasm_host::Host::pure`),
so it links no C or C++ and no crate made for hosts. Its types crate depends
on `crates/be-block-content` (`BlockContent` and the traits beside it,
metadata, `Root`, blobs, settings kinds' `Layered`) and never on
`crates/be-block`, which holds the types the host applies itself and their
heavier dependencies, such as the search index's. What a type needs that
only be-block has moves to be-block-content, or out of the type.

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
  tests can run many modules on one thread) whose methods return
  `be_wasm_ops::guest::LocalFuture<T>`, a boxed future. An `impl Future`
  returned from a trait method gets synthesized associated types, which
  rustc's parallel front end (`-Zthreads`) numbers nondeterministically, so
  callers in other crates failed to compile (an ICE, or a wrong type). The same
  holds for any hand-written trait used across crates: return a boxed future
  from its methods, not `impl Future`;
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
platform (the GPU device: be-wasm-gpu-shim on the web, V8 fast-API functions
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
comes first: it answers the module's types, each with everything the host
keeps about it (`type-info`: its schema, plain form, blank content and that
content's description), and seeds the module's randomness, since a pure guest
has none.

Stateless requests (`merge3`, `merge-parts`, `describe`, `to-file`,
`search-text`, `claims`, `apply`) are answered in the step that asks, except
`merge-parts` and `search-text`, which may read parts first (below, "Merging
by parts"). `apply` is how a host that publishes commits rather than joining a
session edits a block (`be sync`, version control).

`search-text` answers a commit's searchable text for the search index
(guides/blocks/host.md, "Search"): `BlockContent::search_text`, whose default answers nothing,
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
  kept and re-asks what was unanswered. It keeps each local edit (an editor's
  message, a `change`) until a status covers it: `status.taken` counts the
  local edits the replica took since it opened, so a status made before it
  took the latest does not let the host drop them; `sealed-taken` says how
  many of them its head holds (all once it is clean) and `journaled-taken` how
  many the order holds, so a reopened replica is given only what neither holds
  (guides/blocks/host.md, Opening again). A replica submits local edits in the
  order it takes them, which keeps both counts exact, and one opened again skips
  any it is given that the order holds by its origin (below, "Peers"). The host
  numbers each
  opening (`opening.epoch`) and ignores a status whose `epoch` is not the
  current one: an earlier opening made it.
- **The order.** `submit` hands the session's order a submission and answers
  `held` once the server numbered it, `kept` when this device numbered it
  offline, `failed` (it may go again once the replica reopens), or `refused`
  when this account may not edit the block, which the replica gives up as one
  the order passed over; every replica hears what is numbered through
  `replica.sequenced`, in order, its own too. The host keeps what it hears in
  this device's journal, and opens a replica again (rejoining) when it fell
  behind what the server sent.
- **What the server holds.** `publish` answers `kept` for a commit sealed on this device alone
  (offline, or behind others in its outbox), `drained` says those commits
  reached the server, or with `refused` lists the commits it refused (set aside, or
  left in the outbox to be tried again), and
  `opening` says whether it opens on a journal this device numbered offline (`journal-kept`),
  whether the device still keeps commits of the block (`commits-kept`) and which
  refused ones it holds (`refused-commits`). The replica tells editors from these what the
  server lacks (guides/blocks/apps.md, "Sending").
- **IO.** `load`, `publish` (a compare-and-swap on `expected`), `stash` and
  `resume-session` are async imports the host completes. A replica waits on
  one answer at a time. `load` answers a commit's stored form with the parts it
  was saved in (`loaded`, which becomes a `be_block_store::Stored`), only the
  first part's bytes for a type that loads only that (`be_block_content::Loads`).
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
(a `be_block_content::PartRef`) in its own payload. The hold ends when a commit of the
block that uses the part publishes, or once `upload_grace` passed and the
block's journal holds no entries (an uncommitted operation may still name the
part). A type's part is put by an editor: `EditorMirror::put_part`
(`FromEditor::PutPart`, answered `ToEditor::PartPut` under its token), which
the replica turns into its `Effect::PutPart`. `LiveEdit` has no hook for a
replica to put a part of its own; the replica puts only an oversized operation's
bytes. (Chat closes a segment without one: the operation carries the segment's
bytes, which every replica keeps in memory, as it holds the operation, until the content is dropped or
reloaded from a save; there is no hook yet that turns them into a saved part, so a replica
holds a closed segment's bytes once per clone, guides/chat.md.) The
block must be on the server: a block made offline, or one whose replica has
not joined yet, is refused.

Each replica that applies the operation keeps the part in its parts as
`Part::Reused`, so whoever commits next saves a manifest reusing it, uploading
nothing (the server holds it, so `upload-urls` asks for none of it) and
downloading its nodes, never its chunks: its top object to learn its key, and
the rest to list the commit's objects. That needs the server, so a holder
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
first part and implements `be_block_content::MergeParts` registers it
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

The in-session merges of a replica (a rejected seal, a head written offline, a resume that
finds the head moved) go the same way for a type that registered `merge_parts`: the replica
(`be-sdk-type-guest/src/replica.rs`, `Wait::Merging`) polls `MergeParts::merge_parts` itself, and
each part the merge reads goes to the host as `Effect::ReadPart` (`read-part` with the replica as
its parent, which fetches a part the device lacks from the server). The merge waits while it is
read, and never resolves without its parts: a read that fails is asked again (three tries). A
merge that still cannot have a part is dropped, so the replica's jobs go on, reported once
(`error`), and the lease holder tries it again as a `Job::Reconcile` on a timer (5 s, doubling to a
minute; a replica that stops holding the lease drops the timer), for as long as it fails: a
`merge_parts` error that always recurs (a corrupt part) is retried for ever at the ceiling. Until
it resolves the holder makes no due seal (it would publish on the old base and be rejected), and
a seal asked for (`Seal` once or rekey) is answered without publishing; the work stays unsealed
in the journal. While a merge waits, the replica's jobs (an
editor's edit, a peer's operation, a close) queue behind it and then land on its result.
`merge_parts` must await only the `PartReader` reads it asks for: a merge that waits for anything
else is reported and dropped, since nothing would wake it. A type without `merge_parts` merges at
once with `Merge::merge3` on the values it loaded, so one that loads only its first part writes
`merge3` over what it holds (`Sections` settles a part unloaded on a side by refs where they
settle it, and otherwise keeps ours with a conflict, never read). `merge_parts` must agree with
`merge3` wherever `merge3` decides without a conflict (`Sections` shares one function, `by_refs`,
between them; chat's `merge3` is the union that `merge_parts` then settles by reading).

**What a replica reaches.** A module serves every block of its type and may be
another app's code, so a replica's `read-part`, and the parts its `publish` and
`stash` reuse, are confined to its reach (`replicas::reach`): the parts of what
it loaded and put, and any part sealed under a key of its block or of a block
that content is sealed in (a checkout copy's repository), as a part a peer put
and names in an edit is: a block whose content the replica was handed is
reachable whole. A part of another block named by its ref alone is
refused with `OUTSIDE_REACH`, which a module serving both blocks, or a peer who
reads both, could otherwise name to show or write one block's bytes as the
other's (guides/blocks/keys.md, Keys are bound to blocks where the server
points).

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

The server orders the session (guides/blocks/client.md, "Live sessions";
decisions/live_edits_are_ordered_by_the_server.md). A replica hands the order
each edit with the host's `submit`: a `be_block_session::Submission` (its id,
`origin` and a counter it never repeats; `base`, the last sequence it applied;
`barrier`) whose payload is a `wire::Submitted`, the entry and `through`, how many
local edits of the replica it holds. Every replica hears the order through
`replica.sequenced` and applies each entry in order, its own included; what it
hears early waits in `ahead`. Peers also exchange `SessionMessage`s outside the
order: presence and channel messages. The rules an agent most needs:

- **Entries** (`wire::Entry`): `Operation`, an operation's bytes or the part they
  were put as; `Reload`, the content becomes a commit at that place (`head`: the
  holder merged a head moved from elsewhere; otherwise a stash every replica
  remembers in `keep` until the holder seals it as a second parent), with the
  operations numbered after the entry's base applied again on top unless it
  `replace`s the content (a barrier) or `restart`s the session; `Keep`, a stash
  to keep in history. Each entry carries a `floor`, the last barrier, and every
  replica passes over an entry based below it, and one it cannot decode, alike,
  so their copies agree.
- **Own edits.** A replica applies its edit to `visible` at once and submits it
  once nothing of its own is in flight; what it types meanwhile waits in `unsent`,
  absorbed, and goes as one submission when the first comes back. An entry of
  its own coming back is compared with what it submitted; another's numbered
  first goes beneath its unconfirmed edits (`LiveEdit::apply_beneath`, told to
  editors as `Beneath`).
- **Arrival time.** The server stamps each entry with when it took it
  (`received`), and every replica fills the operation's stamp slot
  (`LiveEdit::arrival`) from it as it applies the entry, so the stamp is the
  order's, the same on every replica, journal and replay, whatever the sender
  wrote there. A replica refuses an edit made with the slot filled
  (`CLAIMED_ARRIVAL`). Entries a device numbered offline have no time: content
  then shows none, never one the edit claims. Undo goes through the same check,
  so an inverse operation leaves the stamp off. Types that keep no arrival time
  leave the hook at its default and are unchanged.
- **Sealing.** The lease holder commits at pauses: `QUIET_INTERVAL` (30 s) after
  the last edit, at most `COMMIT_INTERVAL` (5 min) after it started owing, once
  the order since the last seal passes `JOURNAL_BYTES` (1 MiB, an operation's
  part counted), at once when a stash is to be kept, and when it closes. It
  publishes `confirmed` on the head it last sealed, with the first stash in
  `keep` as a second parent; `publish`'s `sealed-through` names the last sequence
  it holds, which the host keeps with the commit (`Commit::sealed_through`), and
  `journal-state` the session state as of it: the journal's base, which the host
  hands the server with the publish (`Write::seals_journal`), so the server trims
  its journal as the seal wins the head and tells every replica
  (`replica.sealed`), which takes it once it applied that far; the host trims
  this device's journal too. With nothing to commit, the host trims with no new
  commit. It goes on applying the order while the seal is out. A rejected seal reconciles
  and seals once more; a head moved from elsewhere reconciles the holder, which
  merges unsaved work into whatever it fast-forwards or merges onto, stashes its
  side when the merge conflicts, publishes the result and submits it as a head
  `Reload`; it seals nothing while that reload is on its way. A replica made
  holder carries on from the last seal's head: nothing moves with the lease.
- **The lease.** The host hands the replica the session's membership
  (`replica.lease`, applying each membership entry as it hears it, and joining
  again on a gap), and the replica derives whether it holds the lease from it
  with the server's function (`Membership::holder`). It may lag the server's
  view by what is on its way, which at worst has two replicas seal at once, and
  the server's compare-and-swap takes one. There is nothing to renew, and a
  replica whose holder left holds it from that entry with no claim of its own.
  Each seal names the replica's `restarts` as its run of the order, so the
  server passes over a stale one. `claim` (a
  terminal host's) asks the server to write a claim, which takes the lease at
  once. A trim with no new commit (`rebase-journal`: the last one out restarting
  the session, a new build starting the journal over) reaches the server only
  from the holder.
- **Opening.** A replica opens on the journal's base (`opening.journal`) and
  applies its entries, or, with `opening.sealed-head`, applies them on the commit
  the seal was made on and takes the seal as its head once it reached the seal's
  sequence. `replay` does the same for a reader outside the session. A journal
  by another build is handed over without its entries, at its last number, and
  the replica reports them lost (`status.undo.lost-edits`); its holder starts the
  journal over.
- **Opening again.** A replica's `origin` is kept across its reopenings
  (`opening.origin`), and `opening.taken-before` says how many local edits the
  host no longer gives it, so it numbers the edits it is given again as before.
  An edit whose number is no more than the most `through` of its origin the order
  holds was numbered already (the answer was lost to a disconnect or a module
  restart) and is counted, not made twice.
- **Restarts.** At a seal, once session state passes `RESTART_STATE_BYTES`
  (1 MiB) and the content's size, with no gesture open, while alone or past
  `RESTART_CEILING` times the limit, the holder submits a restart `Reload` of
  the seal, a barrier: every replica reloads the commit, moves what it has not
  had numbered onto it with `LiveEdit::carry_over` and submits it again, and undo
  stops there (decisions/sessions_restart_when_their_state_outgrows_the_content.md).
  The last holder to leave rebases the journal with no state, so the next
  session starts fresh too.
- **Large operations.** An operation over `LARGEST_RELAYED_OPERATION` (1 MiB, a
  large paste) goes by the part it was put as (`Payload::Part`, put with the
  host's `put-part` before it is submitted); what is typed after it waits in
  `unsent`. A replica reads the part with `read-part` before it applies the
  entry, holding the order behind the read, and reads the parts a journal names
  before it replays it. A read that fails is tried again after 1 s, doubling to a
  minute, for as long as it fails, since every replica applies the entry; a part
  longer than `LARGEST_PART` is passed over by every replica unread. A put that
  fails is tried again after 5 s, doubling to a minute (`put_retry_at`). No
  commit holds the part until the holder seals, so the server's hold on it
  lasts the whole upload grace; `replay` cannot read a part, so a reader outside
  the session reads the head alone.
- **Replacements.** A whole-content replacement (`Replace`, `FromFile`) wins
  outright: the replica stashes what it shows, stashes the replacement on top and
  submits a barrier `Reload` of it, so the holder's next seal has it as second
  parent and both stay in history. A replica whose edits the barrier drops (not
  yet numbered, or numbered after the replacement's base) stashes what it showed
  and submits a `Keep`. Every editor is sent a new snapshot. The change is
  answered once the server holds the reload, not when the order brings it back:
  a host that waits on the answer may be what would hand the replica the order.
- **Who sends what.** The server takes submissions and claims only from editors,
  submissions of the session's build alone. Relayed messages are presence and
  channel messages, which any participant sends; which head
  holds which number comes from the server (`replica.sealed`), never a peer.
  Editors are trusted with the block's content and session; only viewers are
  kept out. The server holds a part for the whole
  upload grace, far longer than a lagging replica needs; a replica may read only
  objects of blocks it may view, and opens them with that block's key.

## Announcements

An edit made on this device can tell the block's members who are not connected
about itself once the server holds it (chat: each message it sends). The type
names what it tells with `LiveEdit::announcements`, asked as the edit is made,
of content that holds it: `be_block_content::Announcement`'s text, the accounts it
mentions, a `key` naming what it announces (chat: the message), unique in the
block and the same when the edit is made again, and `several`, the text for
several sent together with `{count}` for how many (chat: "{count} new
messages"). The notification is tied to the edit, never to an editor or a
replica:

- The replica keeps the announcements with the edit in its lacking list
  (guides/blocks/apps.md, "Sending"), whether an editor follows or not (with
  none, the list is only kept: nothing is told). The replica hands them to the
  host with the `submit` holding the edit, and from there the host keeps them
  durably, the text sealed under the block's key, in the refs database
  (`be-block-client/src/refs/announcements.rs`): until the server answers the
  submission; offline, beside the entry this device numbered, then, when
  the journal starts from a commit this device sealed (one of
  the last 16 its replicas published or kept, which `Replicas` remembers per
  block, or one in its outbox), beside that commit while the outbox keeps it.
  A rebase from anyone else's commit drops what waits in the journal: the edit
  never reached the server.
- The host sends one once the server holds its edit: the submission answered,
  or the commit published or settled by the drain (`Peer::settle`), so it goes
  with no replica or type module open, and after a restart. A commit the
  server refuses drops what it announces, and so does setting the block's
  commits aside (`move_to_unsent`) or giving one up (`abandon`): a message
  shown "Not sent" tells nobody (sent again, it is a new edit). Forgetting a
  block, or losing access to it, forgets its announcements.
- `Peer::send_announcements` sends what is held, after a submission the server
  took, a rebase and at the end of each outbox drain; it sends
  nothing while one drains, so what a drain publishes goes together. It marks
  each sent before it calls `notify`, putting them back only when the server
  cannot be reached, so a crash costs a notification rather than repeating
  one. `notify` names their keys, and the server pushes nothing for a call
  whose keys it has all told before: a device stopped after the server took
  the call but before it heard the answer sends it again on its next start,
  and the members hear it at most once. What waits together on one block is one notification: the text of one,
  or the newest's `several`, mentioning everyone they mention. The server
  allows `MAX_NOTIFICATIONS_PER_BLOCK` a minute per account and block, and one
  it refuses is dropped, never reported: it was about then.
- A key is announced once: one held, kept with a commit or sent is not kept
  again (the host remembers the last 256 sent per block), so an edit a host
  replays into a restarted replica (guides/blocks/host.md), which announces it
  again, is not sent again, and a replayed journal entry announces nothing.
  One still waiting in the journal is replaced by the edit made again.

What is not covered: a replica that closes before it submits an edit tells
nobody (the edit is lost too); and a submission the server took whose answer
was lost to a crash stays waiting on this device and is never sent (the server
holds the edit, but nothing tells this device so).

Tests: be-host-stack's `a_chat_message_*` (through `Peer::notify`, the server's
recipient selection and a Web Push service on loopback whose device key is the
P-256 generator, so a test opens each push and checks its text) and chat's
editor tests over be-sdk-test's store. That store keeps announcements as a
simpler host: one device, so every rebase is on its own commit; every
announced key remembered, with no cap; and announcements of a refused kept
commit dropped by block.

## Editors are endpoints

A local editor follows the replica as a replica follows the order. The host
`attach`es an editor instance (with whether it may edit) and `detach`es it;
the editor's messages are `editor::FromEditor` (operations, gestures,
replace, presence) and the replica answers `editor::ToEditor` (a snapshot,
then operations and peers' presence). A replica drops edits from an
editor attached read-only, but counts them as it counts every operation an
editor sends. `attach` carries `acknowledged`, how many of the editor's
operations the content the replica opened on holds, and the replica sends the
editor nothing of the content until `resent`: the host calls it right after
`attach`, or, for an editor still open across a reopen or restart, after it
gave the replica again what it kept of the editor's edits, so the snapshot
counts them all in the editor's own count. A replica opened again in place of one that
already told an editor that same snapshot (its block's session joined once its creation was
sent) does not tell it again: the replica keeps the hash of the snapshot each editor holds,
until operations change what it shows, and carries it across the reopen, even when the
editor is attached editable where it was read-only: editability is not in the snapshot, and
the host tells an app editor that itself (`SetEditable`, guides/blocks/apps.md). A `Resend` is always
answered. Detaching forgets the count. On the app protocol both directions travel
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

`crates/be-block-type-host` is the host half, natively on V8 (be-wasm-host)
and in process in tests:

- `modules`: the process-wide registry of what each type is (`kind_of`), and
  `Modules`, which routes each type to its module, starts and restarts each
  `Source` (in process, V8, or a web worker), and answers clocks and
  sleeps. A source that declares its types starts lazily, on the first call
  to it, so a host runs only the modules its blocks use; what a host must
  know without running one (names, icons, `plain_file`) comes from the
  manifest, and be-tool-cli's
  `a_manifest_says_which_of_its_types_have_a_plain_file` holds `plain_file`
  to the module's `type-info.plain`, which `be` and `be sync` read without
  manifests (decisions/plain_file_stays_in_the_manifest.md).
  `started(sources)` starts all, as `be` and `be sync` do. A
  module's types are taken only if its manifest declares them
  (`Source::declaring`).
- `replicas`: `Replicas<H: Host>`, the replica IO of
  guides/blocks/host.md (The peer and its registry) for any host.

be-host stages each app's type module beside the app, and `be` reads them as
be-host does, through be-host-stack's discovery (`be_host_stack::types::app_sources`):
from the apps staged beside it, or downloaded from its release by hash
(guides/apps/runtime.md, "Headless"), so no type module is compiled into it. V8
compiles each as it loads. A module's session tag is its SHA-256, so a `be` and a
be-host built from one tree share sessions. Tests run every first-party types
crate in process with `be_type_bundle::sources()`.
