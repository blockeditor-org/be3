# Type modules

A block type is moving out of the app into a plugin's *type module*: a pure wasm
module that holds the device's one replica of every open block of its types (content,
live session, undo stack) and talks to its host only through messages. Two crates
define it; nothing in block-app uses them yet.

- `crates/block-type-api`: the messages between a host and a type module. No GUI types.
- `crates/block-type-guest`: the replica library a be-model-based type module links.
  It is `be_client::Live`, the worker's `Session` (`WithHistory`/`Plain`, seal timing,
  `seal`/`rekey`) and `be_block::history::History` as one deterministic state machine,
  and every `Document<R>` and `Blob<K>` works with it unchanged.

## The protocol

The module is a state machine: the host calls it with an `Event` and gets back
`Command`s. Frames are postcard with a 4-byte big-endian length in front
(`encode_frame`/`decode_frame`); block-plugin-api frames the same way but with bincode.

A host gives each open block a `ReplicaId` and addresses it in every event. Some
events are stateless requests answered by `Command::Reply { id }`: `Blank`, `Merge3`
(a `None` side is the type's default; `conflicts: 0` is clean), `Describe` (name,
derived metadata, references in an organization), `ToFile`, `Claims` (which of the
module's types takes a file by name and first bytes). The replica-addressed requests
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

Peers exchange `wire::Wire`: be-session's `SessionMessage`, plus `Greeting { tag, since }`
in place of `Catchup`. An owner answers a greeting with another tag with
`Incompatible`, and the follower then marks itself incompatible and diverged, so two
builds of a type never edit one session. Everything else is `Live`'s behaviour,
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

These are `EditorMessage::Content` and `ContentOperations` without the host's log:
the replica keeps `projected`, a `Settling` of what its editors hold, lands every
journaled change in it and snapshots when one cannot land. `editor::EditorMirror` is
the editor's side, `ContentProjection`'s adopt-and-take logic without the reactive
parts, for a projection to be built on. Undo and redo are host `Change`s, not editor
messages, since a shortcut or menu may undo a block no editor is attached to; a
gesture is an editor message because it groups that editor's own edits.

## The library

```rust
be_block_types![ChecklistContent { undo }, TextContent { undo, file }, FolderContent];
// expands to: pub fn block_types() -> block_type_guest::Module
```

Each entry is `Registration::of::<C>()` (`C: LiveEdit + Merge + Clone + Default`),
then `.undo::<C>()` for a type whose session keeps an undo stack (what
`kind_with_history` is in the app) and `.file::<C>()` for one with a plain-file form
(`FileForm`: `to_file`, `from_file`, `claims`; implemented for the be-block text,
image, PDF, audio and file types from `be_block::plain`). The order is the order
`Claims` asks in.

`Module::handle(Event) -> Vec<Command>` is the whole interface; `Module::hello()` is
the `Hello` that lists the module's types (content type, frozen, schema fingerprint,
undo, file). The macro also defines `BlockTypes`, a `block_guest_abi::Guest` over that module
(`start` sends `Hello`; `step` decodes each event frame, handles it and encodes the
commands, `Module::step`), and exports it with `block_guest_abi::guest!`, so a type
module crate is that one macro call plus guest-abi's `cdylib` and BUCK set-up. The
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
