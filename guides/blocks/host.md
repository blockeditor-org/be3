# How the host holds content

Part of the block stack (guides/the_new_block_stack.md). The host is the peer,
not the app: an app never sees a key, the connection or the server. It is
handed content and graph answers and hands back operations and graph commands
(guides/blocks/apps.md). The code is `crates/be-host-stack`.

## The peer and its registry

The host's peer names no content type. Every type runs in a type module
(guides/type_modules.md): an app's own, named by its manifest's `block_types`,
staged beside the app as `NAME.wasm` with its `.cwasm` and `NAME.sha256` (the
tag of its sessions), one instance per app; and the host's own types
(`be_sdk_type_host`'s `be_block_types!`: settings, repository, checkout), run
in process as its `BlockTypes`.

A type's name, icon, color, child edits and press come from the declaring
manifest (or `be_host_stack::host_block_types`), held by discovery
(`editors/app/discovery.rs`), so a type is known before its module starts; an
app that declares a taken type or edits an undeclared one is refused. The
registry (`be_block_type_host::modules`, sources listed in
`be-host-stack/src/types.rs`) starts an app's module only once a block of its
types needs it, so `be_host_stack::kind_of` is `None` until then and the worker
asks `Replicas::kind`, which starts the module. Merging, describing and
validating are requests to the module (`Replicas::merge`, by parts for a type
that merges by parts, guides/type_modules.md "Merging by parts"; `describe`). A block
whose type no module has has no content in the host (`be_host_stack::provided`).

## Blocks saved by another build

Every save stamps the derived `SCHEMA` metadata with `schema_fingerprint::<C>()`
(a hash of `BlockContent::schema()`: the model's `Kind::describe`, or `blob N`
from `BlobKind::SCHEMA`). `be_host_stack::staleness` calls an unfrozen block (every block, for now)
stale when its replica could not read it (`Status::unreadable`) or its stamp is
another schema's; no stamp is not stale. A stale block's frame shows
`StaleNotice` (`surfaces::StaleBlock`) with Reset (`be_host_stack::reset`, the
type's default content), Delete, and Try anyway (`try_anyway`, for the
session). A block the host holds itself is reset without asking, with a notice.

The host deletes `app-cache/` when its build id changes and keeps `be-objects/`
and the refs database.

## The worker and its replicas

`be-host-stack/src/worker.rs` is the peer's loop. `be_block_type_host::Replicas`
(shared with `be`; be-host's `Host` is `replicas.rs`, where reports land in
`Shared`) is the host half of every open block's replica: it joins the session,
watches the block, constructs the `replica` with the head, answers its calls
with the real primitives (`Peer::seal` then `publish_sealed`, `Peer::stash`,
resume, the lease, relays sealed with the block key, `sleep-until`) and keeps
what it reports (`report-status`, which carries the undo state too, `described`, `conflict`).

A module runs on its own thread natively, in a web worker on the web
(`WebGuest`), and in process in unit tests (`InProcess`); the worker never
waits on one. A module that stops (a panic, or a step past `STEP_TIMEOUT`) is
restarted and its replicas reopened from their last heads, editors attached and
messages since the last seal replayed. `native.rs` and `web.rs` run the loop
over a `FileStore` (on the web in the origin private file system).

Nothing in the worker runs on a timer: it waits on commands and the
connection's broadcast, with one deadline for the autosave a dirty session is
owed. `status().wakes` counts wakes, and a test asserts an idle peer never
wakes.

Tests wait on the stack with `be_host_stack::wait_until`, which fails at the
caller after `WAIT_LIMIT` without the stack saying anything (or `WAIT_CAP` in
all) instead of waiting forever: tests share one stack in turn, so one endless
wait would hang the whole target. A busy machine slows the stack without
silencing it, so the limit counts silence, not time. A stack on a driven
clock (`Clock::driven`, which be-host-testing's harness uses) also has its
waits move the clock on to the stack's next reconnect, and all the stack's
time with it, so a failed connect never strands on a clock nobody moves. A
test that moves its clock itself uses `Clock::manual`, which no wait moves.
Commit times and an embedded server's history follow `be_block_commit::now_milliseconds`, which a
test pins with `pin_now` to step history a minute at a time (be-host pins it to
its wall clock while that is pinned, so `drive pause` moves it).

The host refuses an `Operate`, a replacing `write-content` and graph commands
for a block the account may only read (`Instances::editable` asks the
mirror's access); it does not rely on the app's `editable()`.

## Connections

`accounts.rs` is the account client: it connects to `{server}/api/be`
(be-server accepts any path; the suffix matters only behind a routing proxy),
registers, signs in and out, lists and creates organizations, and handles
invites. `link.rs` holds one connection per signed-in account and server until
it closes; sign-in hands it over (`link::adopt`) and others borrow it
(`link::signed_in`), reconnecting only once it closed. A connection keeps the
clock it was opened on, so a worker on a test's `Clock::manual()` opens its
own.

The server keeps one open organization per connection, so only the worker
opens one, after the last worker stopped (`Link::open_organization`). Opening
it again drops the connection's watches and listings, so anything needing the
open organization's escrow key (inviting, creating an agent) goes through the
worker's `Peer`.

The refs database is one per device, `be-refs.sqlite3` beside `be-objects/`
(`be_host_stack::refs_in`, `/be-host` on the web). The worker builds its `Peer`
once per workspace, with `Peer::cached` when the database has a session for
that server, account and workspace (so the graph and open blocks come up with
no network), and hands every later connection to it with `attach`.

## Disconnects, offline and stopping

`flush()` seals and waits until every open block is kept on the device (in its
journal or as a local commit), not for the server, so closing a host whose
server stopped answering does not wait for it; `stop()` and organization
changes go through it, because a session merely dropped loses everything since
the last autosave. What a flush kept is sent as the outbox drains.

A dropped connection (`Connection::closed`, not the broadcast, which outlives
the socket) leaves content on screen; the worker backs off (`reconnect_delay`,
cut short by `retry_now`), reconnects, reloads the graph and rejoins every open
block. Meanwhile it keeps serving from the same `Peer` (the loop's `Line`:
`Up` or `Down`), and every open replica is handed a session it owns
(`Replicas::disconnected`), so a follower takes over what it had not confirmed,
and each is asked to seal at once, locally, since reconnecting reopens it from
its head.

- **Opening early.** A block this device knows opens from its local head as a
  follower of a placeholder owner (`JOINING_OWNER`) if joining takes longer
  than `EARLY_OPEN`; when the join lands (`Io::Joined`) the replica is reopened
  with the real session, replaying what happened meanwhile. A seal asked of a
  joining replica with unconfirmed edits first gives it a session it owns, so
  flush and stop never wait for a join.
- **Opening offline.** A block opened while down gets `OFFLINE_CLIENT` owning a
  made-up session (`Open::made_up`): `Publish` becomes `Peer::seal` (a local
  commit in the outbox), `Resume` treats the local head as remote, and
  heartbeats, claims and relays are skipped, and the journal is the device's
  alone. It opens on the newest head whose content the device holds
  (`Peer::held_head`, guides/blocks/client.md), not the server's newest, which
  it may never have read. The same happens while the block
  has outbox commits, and while the host keeps (`Replicas::keep_all`, used by
  flush and stop).
- **Draining on connect.** The worker drains the outbox in a task
  (`Tasks::sync`), merges each `Diverged` block (`Replicas::merge_diverged`:
  `Peer::reconcile`, then the module's three-way merge, published with the
  local commit as an extra parent), and only then rejoins open blocks. It
  drains again after restoring unsent edits, after keeping, and every
  `OUTBOX_RETRY` while something waits.
- **Session calls off the loop.** Heartbeats, relays, claims, leaves, unwatches
  and resynchronizing joins are answered in tasks (`Io::*`). They are written
  the moment they are made, so a leave and the next join reach the server in
  order; an answer older than the replica's session is dropped.
- **Locked messages.** A relayed message is unsealed on the spot; one whose key
  is missing (`ClientError::Locked`, first time: `Peer::first_search_for`)
  fetches keys in a task, and that block's later events wait behind it.
- **Stopping**: `be_host_stack::stop`, `stop_and_wait`. Tests stop as a crash
  would with `crash`: the worker ends without keeping the open blocks, so only
  what the device had written survives.

`be_host_stack::status()` reports `connected`, `syncing`, `waiting` (outbox
operations), `unsent`, `conflicts`/`conflicted`, `kept`, `connecting` and
`unsaved` (what closing or switching would lose: edits not yet in the device's
journal, `Replicas::counts`' third number, plus queued commands and imports; an
edit that is journaled but not yet committed is durable and not counted). The
close and switch guards flush, then ask only while `unsaved` is above zero, and
the "Saving changes" toast shows only while it stays above zero.

Not built yet: a closing web tab cannot wait for `flush()`, so it asks first
(`beforeunload`) while anything is unsaved (`Replicas::counts`).

**Create, define, move and duplicate** are kept on the device and wait in the
outbox, online as offline (`Peer::keep_creation`, `keep_definition`,
`keep_move`), and the loop starts a drain for them; the loop never waits on the
server for them. What follows such a block waits behind it: its publishes stay
in the outbox while a creation, move or define of it waits
(`Peer::publish_sealed`), a block made under it waits behind its parent's
creation (`Peer::create_block`), `Peer::summary` answers from the device until
the server knows it, and a listing the server answers is laid over with what the
outbox adds or moves (`Peer::listed`). A block opened before its creation is
sent opens as offline does; once a drain sends it, the replica seals what it
owns and joins (`Replicas::created`). A command that needs the server to know a
block waits first for a drain to send what this device made of it
(`Tasks::sent`; an export, and a version command in the lane, waits for the
whole outbox, since it reads a tree).

**Commands that need the server.** Those that need only the server and no open
block (share, list access, read a head, leave, approve pairing, members, block
agents; `Command::served`) run in a lane beside the loop
(`worker/served.rs`): one at a time, in the order sent, each in a task, so a
server that stopped answering holds up only the lane. They count as unsynced,
not busy (`Shared::serving`). Offline, sharing, leaving and approving a device
are refused to the caller, who shows it where the person acted; they are not
kept for later and are not the host's error. Order: a loop command waits while
the lane holds a block it needs (`Command::needs`); one that is not order-safe
(`Command::order_safe`: it may reach blocks it does not name, as a version
command or an import reaches a tree) waits until the lane is empty; and none
overtakes a command deferred before it. An open block whose key the lane
rotates is resealed once that command ends. A version command (merging
included) is split: the loop seals the open replicas, and the rest runs in the
lane (`Served::version`), which waits for the outbox to be sent and then reads
and writes the server between steps, reaching the replicas and type modules
through the loop (`version::Reach`, `LoopCalls`: describing, applying and
merging content, moving an open replica to a head it published), so a server
that stopped answering holds up only the lane. A merge reads its sides from the
server in the lane first, so the loop finds them on the device.

## The graph mirror

`be-host-stack/src/graph.rs` mirrors the part of the server's graph something
in the host is looking at: a `Node` per block with type, author, parent,
access, references, unsealed `BlockMetadata`, `locked` (no key opens it) and
the server's `size` and `tree_size`. The host never lists the whole
organization.

The stack tells the host what changed rather than being polled: it pushes a
`Changes` notice (the graph, contacts, storage, stale blocks, search answers,
notification modes, and the blocks whose content changed) into every
`ChangeWatch` (`be-host-stack/src/changes.rs`) as it happens, merged until the
watcher takes it, and the first change after a take wakes the host; a new
watch, and every watch when the stack starts or stops, hears everything.
be-host's core, each app runtime's instances, the compositor and the input
remapper each keep one, take it once a frame, and read again only what it names.

The account's storage against its quota is not in the graph: while an editor
watches `StorageUsageWatch`, be-host turns on `be_host_stack::watch_storage`,
which follows the server's `account.storage-changed` stream (opened again on
reconnecting) and publishes each value; a stream that ends while connected
sets the stack's error.

Everything asks the mirror through `Query` (`Roots`, `Detached`, `Shared`,
`Children`, `References`, `Backrefs`, `Parents`, `Block`, `Subtree`), and
asks again when a change notice says the graph changed. Askers declare what
they want with `be_host_stack::want(owner, queries)` (`Owner::Editor` per app
instance, `Owner::Open` for open blocks' parents, `Owner::Host`,
`Owner::Version`).

- **Listings.** The graph turns what is wanted into server `Listing`s, expands
  `Parents` and `Subtree` as blocks arrive, and fetches new ones together in a
  task (`spawn_listings`), so a slow listing never holds up an open or an
  edit. The loop's other background requests are `worker::Tasks` too.
- **Dropping.** A listing nobody wants is dropped with every block only it
  held, and `unlist`ed ahead of that round's new listings.
- **Readiness.** A query answers only once its listings loaded
  (`be_host_stack::ready`), so an editor never sees a loading folder as empty.
- **Announcements** keep listings current; reconnecting or falling behind
  re-lists everything. The mirror keeps the newest revision per block and drops
  anything older, so a late announcement never undoes a newer reply.
- **Local changes** (`create`, `set_parent`, `define`) apply to the mirror at
  once and are pending for that block: the mirror ignores the server's echoes
  until the change is settled, then refreshes. An open block's references come
  from its session's content, so a new child is listed before it is sealed.

## Metadata

`be_block::BlockMetadata` is two maps from key UUID to CBOR: `defined` (chosen
by someone: `NAME`, `LOCAL_ID`, the artifact source) and `derived` (said by the
content: `INTRINSIC_NAME`, `THUMBHASH`, `SCHEMA`). Keys are typed constants
(`MetaKey::defined`/`derived`), so a new key, a third party's included, needs
no format change.

A value holding a `BlockRef` is a reference the peer publishes with the block,
so backrefs and `KnowExists` see it and a parent holds what its metadata points
to (its settings, an edit kind). Such blocks are not children: apps get only
what the content refers to (`BlockInfo::references`), so a folder never lists
its settings or tags.

Metadata travels with content: every commit carries it, and `Publish` carries
it sealed with the block's data key, so the server stores it in the step that
moves the head. A save takes `defined` from the head it builds on and
recomputes `derived` (`be_block::derived_metadata`); a merge merges `defined`
key by key (`BlockMetadata::merge_defined`). Unknown defined keys ride along;
unknown derived keys are dropped on a new save, since they describe old
content. `Peer::define` publishes a commit of the current content with new
metadata, or uses `SetMetadata` on a block with no content yet;
`be_host_stack::define` and `set_name` apply it to the mirror first, and
`describe_implicitly` keeps an open block's derived metadata current until it
is saved.

## Search

Full-text search over names and content (`be-host-stack/src/search.rs`; what is
left: plans/search.md). The server never reads content, so clients build the
index and search it locally.

- **One index per root block** (a workspace, My files): a block of
  `be_block_graph::SEARCH_INDEX_TYPE` under the root, named by its
  `SEARCH_INDEX` metadata, so it is held by the root and never listed among its
  children. Its access is exactly the root's, whatever its own authorship says
  (`BlockNode::is_search_index`, `level_below`), so a block shared alone never
  reads its tree's text through it. The server holds to the type and parent,
  since it cannot read metadata: an index lies under a top-level block, one held
  per block, never moves under another block (it would read that tree's text as
  the new parent's), takes no grants of its own, and making or dropping it needs
  Edit on the root. It rotates with the root's subtree like any block below it.
- **A block that leaves the tree** (moved to another root, trashed, its text
  deleted) is not masked yet: its text stays in the old root's index, which
  every viewer of that root can read, and only `show` hides it by access. Treat
  a moved or trashed block's old text as still readable by that root's viewers
  (plans/deferred/search.md, "Limits accepted").
- **The content** is `be_block::search_index::SearchIndexContent`, a
  `be_lib_search::Index` kept as the block's parts: the header first (all a
  replica loads, `Loads::FirstPart`), then every part the header names, read by
  range. Two sides merge by parts (`MergeParts`, the union `Index::merge`
  makes); it is registered with the host's own types (`be-sdk-type-host`).
- **Indexing.** A head this device's session moves (`Host::status` with
  `owner`) queues the block (`Searching::sealed`). Each round of the worker's
  loop takes the queued blocks whose root it may edit, reads each one's name
  from the mirror and its documents' text at its head with `search-text`
  (`Replicas::search_text`; a type that answers nothing is found by its name),
  and hands each index one batch in a task, which applies it to the index's
  newest head and seals the commit into the outbox: it is saved offline-first,
  published by the next drain, and two devices' batches meet as a diverged
  block that merges by parts. A workspace (`WORKSPACE_TYPE` at the top) with no
  index gets one as soon as a device that may edit it sees it in the mirror:
  made in a task, then named in the root's metadata from the loop once the
  root's open replica has sealed its edits, as a define command is, so the
  define never lands under a moving session. The loop takes one block a round
  and asks only a type whose module already runs: starting one, or retrying a
  define each round, would stall the opens and edits behind it. A document's
  text is indexed up to `MAX_TEXT` (1 MiB) bytes. Blocks whose index has no
  head on this device yet are parked until it has one, not queued each round.
  The host is busy (`Searching::busy`, which tests settle on) while a workspace
  it may edit wants an index and has none, while one is being made or named, and
  after a define is sent until the root's summary is read again, so the size the
  files pane shows is the server's with the define in it; a define still in the
  outbox after a drain that ended since is not waited for. Naming is tried at most
  eight times, and a refused define gives up on the root with an error shown, until
  this account's access to it changes; a root whose index could not be made is tried
  again when the mirror next changes. A root that was left or can no longer be edited
  drops its index and the blocks waiting for it.
  `Searched::complete` is true when nothing this device could index is queued,
  being made or written; an edit on a root it cannot edit is never queued.
- **Catching up.** When this device is connected and may edit a root whose
  index is named, `start_catch_ups` asks `Peer::changed_since(root, watermark)`
  (the index's `Index::watermark`) once per connection: the blocks it lists
  (commits from devices that never index, imports, blocks moved in) are queued
  like a sealed block, read from what the call said where the mirror lacks them
  (`Searching::caught`, a type's module started for them), the blocks that left
  are masked (`Batch::removed_blocks`, at the time of the catch-up; a block that
  comes back is stamped after the mask), and the watermark moves with that batch
  once everything the catch-up queued is indexed. A block listed before its
  content was published is caught by the next catch-up, which is after the next
  connection. The host is busy while one is due or running. A search that is
  shown is shown again whenever the mirror moves, so a hit whose block left what
  the search is within drops out at once.
- **Searching.** `be_host_stack::want_searches(owner, asks)` names the searches
  an owner shows and `searched` answers each (a change notice says when one
  changed). A search runs in a task over the index of every root the account
  views in the mirror (or of `within`'s root), reading only the parts it needs
  with `Peer::read_part`, and again whenever one of those indexes' heads moves.
  The loop then cuts the shown hits' snippets (`Query::snippet`) from their
  documents' text (`search-text` with the hit's key), kept per document and
  head, and drops hits the mirror shows
  detached, unviewable or outside `within`; it lists each hit block
  (`Owner::Search`) and shows a hit only once the mirror holds it.
- **Opening a hit.** Quick open (`OpenableBlock::at`) and the file tree's hit
  rows (`EditorHost::open_block_at`) open a hit's block with a
  `search-location`: its document key and the words its first match covers.
  be-host keeps it (`reveals::Reveals`) until the editor showing that block
  is open, a few seconds at most, then sends it `editor-event.reveal`; a later
  open of the block, or its editor closing, lets go of it. be-sdk hands it on
  as `Pushed::Reveal` and `EditorHost::take_reveal`. The text app, once its
  content has loaded and its saved position is back, selects the first place
  the words occur as whole words, compared as the index folds them
  (`be_lib_search::fold`); an editor that supports no location ignores it.
- **Apps** watch a search with `search-watch` (`Editor::search` in
  be-sdk-beui); the file tree's filter lists the hits under "In content", and
  be-host's quick open (`found_blocks` in `compositor/palette.rs`) lists them
  after the blocks whose names match.

## Creating, seeding and replacing

- **Creating.** `create-block` carries the first content, written as the first
  commit before any later command for the block; with none the worker writes
  the type's blank content (`Kind::blank`), so a host or app can make a block of
  a type whose format it does not own. Apps pick the new id themselves.
  `be_sdk_database::create_database` makes a schema and a database together.
- **Seeding.** `editor.seed_content` (`write-content` without `replace`) writes
  a first commit, and is ignored for a block that has content or is open.
- **Replacing.** `editor.replace_content` (`write-content` with `replace`)
  becomes the replica's `Change::Replace`, or a direct publish when nothing has
  the block open. The replica publishes a commit (the bytes take the chunked
  upload, not the session) and the replacement wins: unsaved work is stashed
  as a parent of a later head, and followers reload (`SessionMessage::Replaced`
  from a follower). Image, audio and PDF "replace" and compiled logic use it.

## Exporting and importing

A block file (`be_block_client::transfer`, what `be download` writes) holds a
tree. host-services' `export-block` lists the tree a level at a time and fills
each block's bytes from its open session, or its head with the journal
replayed (`Modules::readable`), so it works for types the host does not know
(they read the head alone).

`import-blocks(parent)` takes any file: non-`.block` files go to
`be_host_stack::import_file`, which picks a type the way `be sync` does
(guides/sync.md); a `.block` file is `be_host_stack::import`ed with fresh ids
swapped into content and references. The tree is in the mirror at once and
uploads in a task beside the loop (`worker/uploads.rs`): `Peer::queue_import`
mints keys and writes commits into the outbox (so an offline or interrupted
import is sent by the next drain), then `upload_import` sends everything
without waiting on each answer. Commands on a block still uploading wait for
it, and anything that rekeys a subtree waits for all of it. Import straight
into the target folder: moving a tree into another workspace rekeys every
block in it.

## Children, duplication and holding

Moving a block into a container, deleting a child, replacing it or moving it
among its siblings asks the container's content to change: `be_host_stack::change_child` sends the
container's replica `Change::Child`, and the type's `Root::child_edit` turns a
`be_block::ChildChange` into an ordinary edit. An editor asks for a reorder with
`EditorHost::reorder_block(block, parent, after)` (`BlockCommand::Reorder`);
be-host queues it behind any move of the same block still under way
(`queue_reorder` in its main.rs) and sends it only to a type whose manifest
says `children.reorder`. `be_host_stack::duplicate`
copies what the source's replica shows (or its head) into a new id before
anything opens the copy. `be_host_stack::hold` keeps a block open for the host
(settings, the shell's session).

A block being placed counts as held by its parent (`Graph::placing`) until the parent's
references take it in. Should they drop it again while it still names the parent, it reads as
deleted: the mirror notes it (`Graph::take_dropped`) and the worker logs it with what it knows
of the parent (`Shared::log_dropped`), which a host test's failure shows in the app's log.

## Undo

Undo lives in the block's replica, the one place that sees every edit from this
device, whichever editor made it. A type registered with `undo` keeps
`be_block::history::History` (`HISTORY_STEPS` deep): steps sent within 750 ms
of the last, by the frame-clock time each relayed frame is stamped with, may
absorb into one, and `be_host_stack::undo`/`redo` send `Change::Undo`/`Redo`.
An editor groups a gesture into one step with `FromEditor::Gesture`
(`ContentProjection::begin_gesture`, `commit_gesture`, `cancel_gesture`; a
cancel reverts and leaves no step), and ends its burst with `break_step`
(`Gesture::Break`), so its next edit starts a step however soon it comes. A text
editor breaks where its core ends a group: at word boundaries (so quick typing
undoes a word at a time), a paste, a caret move, formatting, blur; never for a
peer's edit. A break leaves the editor's own open gesture alone, and another
editor's burst. Apps call `act(block, block-command::undo)`
and watch a block's `history` with a `block-watch`. A session restart (guides/type_modules.md, Peers)
drops every step, since none applies to the reloaded content: `block-history`
says `restarted-at`, and the frame answers an undo with nothing left by saying
"Can't undo past here" with a button that sends `block-command::history` to
show version history at that commit (the shell gets `show-request::history`).
`lost-edits` says edits of the block made with another build were lost (its
journal held some this build cannot replay, or the server marked them lost since
the last commit); the frame says so.
