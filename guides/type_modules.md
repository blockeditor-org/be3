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
  It holds a block's live session (owner sequencing, follower rebase, takeover, seal
  timing, `seal`/`rekey`) and its undo stack (`be_block::history::History`) as one
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

- **Time.** `Tick { now }` (milliseconds of the host's `be_client::Clock`, which
  `Modules::new` takes; an editor's frames are stamped with the frame clock's
  instant, `Clock::at` places it on that clock) before any event once
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
- **The lease.** `Lease { session, resynchronize }` whenever the server's session
  state changes (`resynchronize` after the host fell behind the broadcast).
  `ReportClean` is the heartbeat with `clean_at`; `ClaimLease` is sent once per
  generation when the owner is gone from the participants, and when the host
  sends `Claim` (a lapsed lease, taken by a terminal host) once the replica has
  caught up. An owner told another client holds the lease becomes a follower. On `Close` the replica
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

Peers exchange `wire::Wire`: be-session's `SessionMessage`, `Tag` and
`Incompatible`. A follower sends `Tag` (its `SessionTag`) ahead of each `Catchup`;
an owner answers another tag with `Incompatible` and then ignores that peer's
submits and catch-ups, and the follower marks itself incompatible and diverged, so
two builds of a type never edit one session. The session itself is owner
sequencing, follower rebase and `apply_beneath`, the 1 MiB relay limit (a follower's
queued edits are absorbed into pieces no larger), `Snapshot`/`Sealed`/`reload`,
`Replaced`, `Moved`, `Kept`/`Saved`, takeover with `Sequencer::continuing`, presence
and channel messages. Its rules: an owner that owes a seal seals `SEAL_INTERVAL`
(750 ms) after it started owing; a rejected seal reconciles and seals once more;
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
be-session `Lease`, follows each attached editor with an `EditorMirror`, and moves
time only through `advance`. `quiet` holds back `Moved` so a seal is rejected;
`crash` drops a device without its seal; `publish_elsewhere` is a publish from
outside the session, as be sync makes.

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
