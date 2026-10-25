# Type modules

A block type is moving out of the app into a plugin's *type module*: a pure wasm
module that holds the device's one replica of every open block of its types (content,
live session, undo stack) and talks to its host only through messages. Two crates
define it, and a third hosts it. block-app's peer, `be` and `be sync` host every
block through them (guides/the_new_block_stack.md, "How the app hosts content";
"Hosts" below): each plugin that brings types names its module in its manifest's
`block_types` (guides/adding_a_block.md), and the types the app applies itself
(`crates/host-types`) run in process.

- `crates/block-type-api`: the messages between a host and a type module. No GUI types.
- `crates/block-type-guest`: the replica library a be-model-based type module links.
  It holds a block's live session (applying the server's order, rebasing its own
  unconfirmed edits, catching up, sealing as the lease holder, `seal`/`rekey`)
  and its undo stack (`be_block::history::History`) as one
  deterministic state machine, and every `Document<R>` and `Blob<K>` works with it
  unchanged. Its `Sim` tests (`src/tests.rs`) are where session behaviour is tested:
  a simulated host with any number of devices, a lease, commits and editors.

## The protocol

A pure guest has no entropy of its own, and a type module needs ids for the
items its child edits add. The host sends `Entropy { seed }` first, and on
wasm32-unknown-unknown the library is getrandom's custom backend, a generator
seeded from it (`entropy.rs`; buck sets the `getrandom_backend` cfg for that plan).
be-store's sealing (`vault`), be-commit's `CommitStore` and be-session's `resume`
are default features a guest's dependencies leave off, so a module imports only
the four `be3_host` functions.

The module is a state machine: the host calls it with an `Event` and gets back
`Command`s. Frames are postcard with a 4-byte big-endian length in front
(`encode_frame`/`decode_frame`); block-plugin-api frames the same way but with bincode.

A host gives each open block a `ReplicaId` and addresses it in every event. Some
events are stateless requests answered by `Command::Reply { id }`: `Blank`, `Merge3`
(a `None` side is the type's default; `conflicts: 0` is clean), `Describe` (name,
derived metadata, references in an organization), `ToFile` (a `PlainFile`: the
bytes, the extension and media type the type gives them, and the name of the file
they came from when the content keeps one), `Claims` (which of the
module's types takes a file by name and first bytes, or with `directory` a new
directory), and `Apply` (a `Child` or `FromFile` change made to content bytes,
or to the type's default for `None`, answered `Content`, or `Changed(false)` when
it does not apply), which is how a host that publishes commits rather than
joining a session edits a block: be sync, and block-app's version control when it
lists a checkout in its parent (`Replicas::apply`, then `describe` and
`publish_encoded`). The replica-addressed requests
are `Change` (`Child`, `FromFile`, `Replace`, `Undo`, `Redo`), `Content` (the visible
bytes, for export and duplication) and `Seal` (`force` is a rekey: publish even when
clean). A request for a replica that is not open is answered `Unknown`.

What the host owes a replica:

- **Time.** `Tick { now }` (milliseconds of the frame clock) before any event once
  the clock has moved, and again at or after the latest `WakeAt { at }`. The replica
  never reads a clock, so a test advances time by sending ticks.
- **Opening.** `Open` carries the head the host just read, the session's lease
  state, the client id and a `SessionTag` (the module's hash). The replica reads the
  head itself (`Read`) and queues everything until it arrives. The host must send
  `Head { Moved }` for every head that lands after the one in `Open` (including the
  remote head, when `Open` carried a local head it differs from), since the replica
  does not reconcile on opening.
- **IO answers.** `Read` -> `Event::Read`, `Publish` -> `HeadEvent::Published` or
  `Rejected { head }` (an unchanged save is `Published` with the same head), `Stash`
  -> `Stashed { commit }`, `Resume` -> `Event::Resumed` (the host fetches both
  histories and runs `be_session::resume`; the replica cannot see history). Each
  carries the `Token` of its command. A replica waits on one answer at a time and
  queues every other event behind it.
- **Publishing.** `Publish { expected, merging, content }` is a compare-and-swap on
  `expected`, whose commit is the first parent. `merging: Some { base, ours }` adds
  `ours` as a second parent and asks the host to merge defined metadata
  (`BlockMetadata::merge_defined` of `base`, `expected`, `ours`); otherwise defined
  metadata comes from `expected`. `Stash { parent }` writes a commit that moves no
  head. `Written` carries references unresolved and derived metadata; the host
  resolves, seals and uploads.
- **The order.** `Submit { id, base, barrier, payload }` goes to the server
  sealed (`Peer::submit`); the server's `Sequenced` comes back as
  `Event::Sequenced` for every submission, this replica's own included.
  `Catchup { origin }` is answered `Caught` with the server's `SessionLog`
  (`Peer::catch_up`, which opens the payloads and the seal's state).
- **The lease.** `Lease { client, session, resynchronize }` whenever the
  server's session state changes; `resynchronize` after the host fell behind
  the broadcast or reconnected (a reconnect keeps the replica and gives it its
  new `client`), and the replica catches up again. `ReportClean` is the
  heartbeat with the `SessionSeal`; `ClaimLease` is sent once per generation
  when the holder is gone from the participants, and when the host sends
  `Claim` (a lapsed lease, taken by a terminal host) once the replica has
  caught up. On `Close` the replica waits (up to 5 s) for its own edits to come
  back ordered, seals what it owes and answers `Release`, after which the host
  leaves the session; events for it until then still go to it. A `Seal`
  request is answered without waiting on the server (a host may block on it):
  a holder with edits not yet ordered publishes what it shows, edits and all,
  without reporting that as a seal, and seals the order on top later.
- **Relaying.** `Send { to }` is opaque bytes for `One(Peer)`, `Peers` (every other
  participant), `One(Editor)` or `One(Host)`. The host seals and relays peer bytes
  and delivers them as `Message { from: Peer(client) }`.
- **Reports.** `Status` (head, owner: holds the lease, submitted: every edit
  made here has gone to the server, clean: every edit made here has come back
  ordered and nothing is owed, owes a seal,
  incompatible, diverged, unreadable), `Described` (after the visible content changed, for the graph mirror),
  `History` (can undo or redo), `Conflict` (a merge kept both sides), `Error`. Each is
  sent when it changes.

## Peers

The session is the server's order (guides/the_new_block_stack.md,
"be-session"). A replica submits each edit once nothing of its own is in
flight, and queues what it types meanwhile in `unsent`, absorbed into pieces no
larger than the 1 MiB submission limit, which it submits as one edit once the
first comes back. Each sequenced entry is a `wire::Entry`:

- `Operation`: decoded and rebased onto the operations numbered between its
  `base` and itself, then applied; the author compares it with its own copy and
  takes the order's version when they differ. One based below its `floor`, one
  that no longer decodes, and one `LiveEdit::rebase` returns `None` for are
  passed over by every replica alike; an author whose edit fell below the floor
  of a trim submits it again, and one that was dropped otherwise loses it.
- `Reload { commit, head, replace, state }`: the content becomes the commit (read
  through the host) with `state` adopted. Without `replace` every replica then
  replays the operations numbered after the entry's `base`; with it, none, and
  the entry is a barrier: every replica drops its own unconfirmed edits, and
  the server drops whatever was based before it. `head` says the commit is the
  block's head; otherwise it is a stash that every replica remembers
  (`keep`) until the holder seals it as a second parent, so its history stays.
- `Keep { commit }`: a stash to keep the same way.

What changes content outside the order goes in as a `Reload`. The holder's
reconcile publishes the merge and submits it with `head`; while that reload is
outstanding it does not seal. An edit too large to submit is stashed with the
replica's other queued edits on top of `confirmed` and submitted as a
`Reload`; the replica drops those edits from `unsent` when the entry comes
back. A whole-content replacement (`Change::Replace`, `FromFile`,
`FromEditor::Replace`) from any replica stashes what that replica has not seen
sealed, stashes the replacement on top of it and submits it as a barrier
`Reload`, so the replacement wins outright; a replica that loses its own work
to the barrier (edits not yet ordered, or ordered after the replacer's base)
stashes what it showed and submits a `Keep`. Every editor is sent a snapshot
after a reload.

A seal is the holder's: `SEAL_INTERVAL` (750 ms) after it started owing, or at
once after a reload or a `Keep`, it publishes `confirmed` on the head it last
sealed, with the first remembered stash as a second parent, reports the
`SessionSeal` and relays `SessionMessage::Sealed`. A rejected seal reconciles
and seals once more; a head moved from elsewhere reconciles the holder, which
merges unsaved work into whatever it fast-forwards or merges onto and stashes
its side when the merge conflicts. A head `Open` named that the session's seal
does not hold (written offline) is merged in the same way once the replica
holds the lease.

The first participant's `SessionTag` is the session's: a replica whose tag
differs is incompatible and diverged and refuses to edit or seal, and the
server refuses its submissions, so two builds of a type never edit one
session.

## Editors are endpoints

A local editor follows the local replica as a replica follows the server's order. The host sends
`Attach { editor, editable }` when an editor instance starts following a block
(again to change `editable`) and `Detach` when it stops; the editor's messages arrive
as `Message { from: Editor(instance) }` and are `editor::FromEditor`: `Operate`,
`Gesture` (`Begin`, `Commit`, `Cancel`), `Replace`, `ShowPresence`, `SendToPeers`,
`Resend`. A replica drops edits from an editor attached read-only. It answers with
`editor::ToEditor`:

- `Snapshot { bytes, session, unsettled, applied }` on attach, on `Resend`, and to
  every editor after the content was replaced outright. `bytes` and `session` are the
  settled content, `unsettled` the replica's own unconfirmed operations on top of it, and `applied`
  how many of that editor's own operations the snapshot already holds.
- `Operations`, every operation since the last message, each with `mine`, the number
  of operations still unsettled after it, and for a peer's operation that goes beneath
  them, the unsettled operations as rebased (`beneath`).
- `Presence` and `PeerMessages` from the peers.

On the plugin protocol both directions travel in `EditorMessage::Replica` frames
(guides/the_new_block_stack.md, "Content on the plugin protocol"). The replica keeps no log either:
the replica keeps `projected`, a `Settling` of what its editors hold, lands every
journaled change in it and snapshots when one cannot land. `editor::EditorMirror` is
the editor's side, which `ContentProjection` is built on: the settled content, the
pending operations, what each update touched, a `revision()` count and the
`Projected` log (`record`, `take_projected`) a mirrored copy such as `LiveText` reads. Undo and redo are host `Change`s, not editor
messages, since a shortcut or menu may undo a block no editor is attached to; a
gesture is an editor message because it groups that editor's own edits.

## The library

```rust
be_block_types![ChecklistContent { undo }, TextContent { undo, file }, FolderContent];
// expands to: pub fn registrations() -> Vec<Registration>,
//             pub fn block_types() -> block_type_guest::Module,
//             pub struct BlockTypes (the guest)
#[cfg(feature = "module")]
block_type_guest::abi::guest!(BlockTypes);
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
one that is only ever the root of one (a workspace); `Hello`'s `TypeInfo::plain`
says which form each type has. The order is the order `Claims` asks in. A type
that takes any file (the workspace plugin's file type) sets `ANY_FILE` on its
`BlobFile` or `DocumentFile`, and answers only a `Claims` with `fallback`: a host
asks every module without it first, then every module with it.

`Module::handle(Event) -> Vec<Command>` is the whole interface (`register` adds or
replaces a type, `knows` asks for one, as block-ui-test does for a test's own types); `Module::hello()` is
the `Hello` that lists the module's types (content type, frozen, schema fingerprint,
undo, file). The macro also defines `BlockTypes`, a `block_guest_abi::Guest` over that module
(`start` sends `Hello`; `step` decodes each event frame, handles it and encodes the
commands, `Module::step`). It does not export it: a types crate is a plain library
that editors, tests and other crates depend on, and only its `:wasm` build, with
the crate's `module` feature, calls `block_guest_abi::guest!`, since a second set of
guest exports in one wasm (an editor's own) would clash. So a type module crate is
that macro call, the export line and guest-abi's `cdylib` and BUCK set-up
(`crates/editors/checklist/types`).
`Registration::without_undo` drops a registration's undo stack, which is how
block-ui-test hosts every type (`ContentStore::register` takes a types crate's
`registrations()` for a test that needs a plugin's types it never names). The
waker is unused: the replica asks for time with `WakeAt` and the host ticks it.
The undo stack is `History<C, u64>`: `History` is generic
over its clock (`history::Moment`, for `Instant` and frame-clock milliseconds), and
`Recorder` is what a replica asks of it, with `NoHistory` for a type without undo.

## Tests

`crates/block-type-guest/src/tests.rs` is a simulated host: `Sim<C>` runs several
devices, each with its own `Module`, relays between them in order, keeps commits and
the head with compare-and-swap, answers `Resume` from the commit graph, runs a
be-session `Lease` and `Sequencer` as the server does, follows each attached
editor with an `EditorMirror`, and moves time only through `advance`. `quiet`
holds back `Moved` so a seal is rejected; `deaf` devices hear nothing from the
server or their peers; `stalled` devices' submissions wait for `unstall`;
`reconnect` gives a device a new client and resynchronizes it; `crash` drops a
device without its seal; `publish_elsewhere` is a publish from outside the
session, as be sync makes.

## Hosts

`crates/be-types-host` is the host half every host shares, natively under
wasmtime and in process in tests:

- `modules`: the registry (`kind_of`, `is_known`, `plain_of`: each type's
  `Kind`, filled from every module's `Hello` with its blank content and
  description; process-wide, since it holds only what a type is), and `Modules`,
  which routes each type to the module that said `Hello` with it
  (`module_of`, its own, so two hosts in one process with different sources
  each reach their own instance), starts each `Source` (`in_process::<G>`,
  natively `wasm(name, tag, cache, load)` on a `PureHost` thread, on the web
  `web(name, url, tag)`), restarts one that stops, ticks it and collects its
  commands. `started(sources)` waits for every `Hello`. Its stateless helpers
  (`ask`, `to_files`, `claims`, `apply`, `describe`) are what a host that holds
  no replica uses.
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
registers only those from that module's `Hello`, and no module's `Hello` takes a
type another declares.
Its session tag is the module's SHA-256, as the staged `.sha256` is, so a `be`
and an app built from one tree share sessions. Tests and examples of a host run
every first-party types crate in process with `first_party_types::sources()`
(`crates/first-party-types`), named by plugin id and declaring each module's types.
