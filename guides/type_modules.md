# Type modules

A block type is moving out of the app into a plugin's *type module*: a pure wasm
module that holds the device's one replica of every open block of its types (content,
live session, undo stack) and talks to its host only through messages. Two crates
define it, and a third hosts it. block-app's peer, `be` and `be sync` host every
block through them (guides/the_new_block_stack.md, "How the app hosts content";
"Hosts" below): each plugin that brings types names its module in its manifest's
`block_types` (guides/adding_a_block.md), `crates/builtin-types` holds the
first-party types not moved into their plugins yet, and the types the app applies
itself run in process.

- `crates/block-type-api`: the messages between a host and a type module. No GUI types.
- `crates/block-type-guest`: the replica library a be-model-based type module links.
  It is `be_client::Live`, the worker's `Session` (`WithHistory`/`Plain`, seal timing,
  `seal`/`rekey`) and `be_block::history::History` as one deterministic state machine,
  and every `Document<R>` and `Blob<K>` works with it unchanged.

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
derived metadata, references in an organization), `ToFile`, `Claims` (which of the
module's types takes a file by name and first bytes, or with `directory` a new
directory), and `Apply` (a `Child` or `FromFile` change made to content bytes,
or to the type's default for `None`, answered `Content`, or `Changed(false)` when
it does not apply), which is how a host that publishes commits rather than
joining a session edits a block. The replica-addressed requests
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
  `Head { Moved }` for every head that lands after the one in `Open`, since the
  replica does not reconcile on opening the way `Live::join` did.
- **IO answers.** `Read` -> `Event::Read`, `Publish` -> `HeadEvent::Published` or
  `Rejected { head }` (an unchanged save is `Published` with the same head), `Stash`
  -> `Stashed { commit }`, `Resume` -> `Event::Resumed` (the host fetches both
  histories and runs `be_session::resume`; the replica cannot see history). Each
  carries the `Token` of its command. A replica waits on one answer at a time and
  queues every other event behind it, as `Live` awaited each request inline.
- **Publishing.** `Publish { expected, merging, content }` is a compare-and-swap on
  `expected`, whose commit is the first parent. `merging: Some { base, ours }` adds
  `ours` as a second parent and asks the host to merge defined metadata
  (`BlockMetadata::merge_defined` of `base`, `expected`, `ours`); otherwise defined
  metadata comes from `expected`. `Stash { parent }` writes a commit that moves no
  head. `Written` carries references unresolved and derived metadata; the host
  resolves, seals and uploads.
- **The lease.** `Lease { session, resynchronize }` whenever the server's session
  state changes (`resynchronize` after the host fell behind the broadcast).
  `ReportClean` is the heartbeat with `clean_at`; `ClaimLease` is sent once per
  generation when the owner is gone from the participants. On `Close` the replica
  seals what it owes and answers `Release`, after which the host leaves the session;
  events for it until then still go to it.
- **Relaying.** `Send { to }` is opaque bytes for `One(Peer)`, `Peers` (every other
  participant), `One(Editor)` or `One(Host)`. The host seals and relays peer bytes
  and delivers them as `Message { from: Peer(client) }`.
- **Reports.** `Status` (head, owner, clean, owes a seal, incompatible, diverged,
  unreadable), `Described` (after the visible content changed, for the graph mirror),
  `History` (can undo or redo), `Conflict` (a merge kept both sides), `Error`. Each is
  sent when it changes.

## Peers

Peers exchange `wire::Wire`, which is be-session's `SessionMessage` encoded exactly
as `Live` sends it, so a replica and a `be_client::Live` peer share a
session. A follower sends `Wire::Tag` (its `SessionTag`, on a channel a `Live` peer
passes over) ahead of each `Catchup`; an owner answers another tag with
`Wire::Incompatible` and then ignores that peer's submits and catch-ups, and the
follower marks itself incompatible and diverged, so two builds of a type never edit
one session. Everything else is `Live`'s behaviour,
message for message: owner sequencing, follower rebase and `apply_beneath`, the 1 MiB
relay limit, `Snapshot`/`Sealed`/`reload`, `Replaced`, takeover with
`Sequencer::continuing`, presence and channel messages. The worker's rules are here
too: an owner that owes a seal seals `SEAL_INTERVAL` (750 ms) after it started
owing; a rejected seal reconciles and seals once more; reconcile merges unsaved work
into whatever it fast-forwards or merges onto and stashes our side when the merge
conflicts; a head moved from elsewhere reconciles an owner, and one this device
published outside the session (`from_this_device`) sends a follower's owner
`Replaced`.

## Editors are endpoints

A local editor follows the local replica as a peer follows an owner. The host sends
`Attach { editor, editable }` when an editor instance starts following a block
(again to change `editable`) and `Detach` when it stops; the editor's messages arrive
as `Message { from: Editor(instance) }` and are `editor::FromEditor`: `Operate`,
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
(`FileForm`: `to_file`, `from_file`, `claims`; implemented in the library's
`file.rs` for the be-block text, image, PDF, audio and file types, with
`be_block::plain`'s file-name and media-type helpers). `directory` makes a
type's plain form a directory that takes new directories (a folder), and `root`
one that is only ever the root of one (a workspace); `Hello`'s `TypeInfo::plain`
says which form each type has. The order is the order `Claims` asks in.

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
(`crates/editors/checklist/types`, `crates/builtin-types`).
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
be-session `Lease`, follows each attached editor with an `EditorMirror`, and moves
time only through `advance`. `quiet` holds back `Moved` so a seal is rejected;
`crash` drops a device without its seal; `publish_elsewhere` is a publish from
outside the session, as be sync makes.

## Hosts

`crates/be-types-host` is the host half every host shares, natively under
wasmtime and in process in tests:

- `modules`: the registry (`kind_of`, `is_known`, `plain_of`: each type's
  `Kind`, filled from every module's `Hello` with its blank content and
  description), and `Modules`, which starts each `Source` (`in_process::<G>`,
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

Where the modules come from: block-app stages `builtin-types.wasm` (and its
`.cwasm` and `.sha256`) beside its plugins; `be` (and so `be sync`) carries it
in its executable (`include_bytes!` of `BUILTIN_TYPES_WASM`, set in
`crates/be-cli/BUCK`), so its release stays the one file guides/bootstrap.md
pins, and compiles it once into `BE3_CACHE/modules` through wasmtime's cache. It
carries every plugin's manifest and type module the same way, as one bundle
(`PLUGIN_TYPES`, buck/app's `type_bundle`, read by `be_cli::plugins`), and runs one
module per plugin that has one, as the app does; the manifests also name the
types its own short names (`blocks::type_name`) do not cover. A host gives each
plugin's `Source` the types its manifest declares (`Source::declaring`): `Modules`
registers only those from that module's `Hello`, and no module's `Hello` takes a
type another declares.
Its session tag is the module's SHA-256, as the staged `.sha256` is, so a `be`
and an app built from one tree share sessions. Tests run `builtin_types` in
process.
