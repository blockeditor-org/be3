# Blocks on the app protocol

Part of the block stack (guides/the_new_block_stack.md). How an editor reads
and changes blocks through the host. The protocol is the `be3:app/blocks` WIT
interface (`crates/be-sdk-api/wit/blocks.wit`); every call is made under the
editor's resource, so the host knows the instance and closing the editor ends
what it opened. The host side is guides/blocks/host.md; the replica the editor
follows is guides/type_modules.md.

## Content on the app protocol

An editor is an endpoint of the block's replica ("Editors are endpoints" in
guides/type_modules.md). Everything about a block's content crosses through a
`replica` resource the editor opens per block: its own (on its first frame) and
each block it watches. `replica.frames()` streams host-to-editor frames,
back-pressured by be-wasm-ops credits; `replica.send(frame)` is answered once
the host took the frame, and the editor keeps at most a window unanswered (the
rest wait in `EditorHost`). A frame is an encoded
`be_sdk_type_guest::editor::ToEditor` or `FromEditor`, relayed unread between
the instance and the replica.

- App to host: `Operate { gesture, operation }`, `Gesture { id, gesture }`,
  `Break`, `ShowPresence`, `SendToPeers`, `Resend`, `Read` (below). Gestures
  are named by ids the instance picks, and an edit names the gesture it was made
  in, or none (guides/blocks/host.md, Undo).
  An instance asks for content by sending `Resend` when it first follows a
  block; the host attaches it then, with `editable` from `be_host_stack::access`
  (attached again when that changes, and the app is sent `SetEditable`): the
  account's access, read-only too while a discard's mark holds
  (guides/blocks/host.md).
- Host to app: `Snapshot { content_type, bytes, length, session, unsettled,
  applied }`, where `length` is the whole stored form's,
  where `applied` counts operations *this instance* sent that the snapshot
  holds (taken or refused, and across replicas reopened under it), which is
  how an app tells its own unacknowledged edits from others': `EditorMirror`
  drops that many from the front of what it sent, counting from what it was
  told landed, and rebuilds its view on the snapshot with the rest on top;
  `Operations` (each marked `mine`, with `unsettled` and, for a peer's
  operation that goes beneath them, the rebased `beneath`); `Lacking` (below,
  "Sending"); `Presence`;
  `PeerMessages`; `Range`; `GestureEnded { id, cancelled }`, the replica ending
  one of the instance's gestures, which `EditorMirror` answers with a cancel.
- `write-content` writes content outright, since it may name a block nothing
  follows (guides/blocks/host.md, "Creating, seeding and replacing").

On the app side, `editor.block_content::<C>()` is a `ContentProjection` built
on `be_sdk_type_guest::editor::EditorMirror<C>`. `operate` applies to what the
view sees and queues for the host. A `mine` operation coming back only
confirms, and only that this device's replica took the edit: it confirms at
once, before the order numbered it (it is still `unsettled` until then), so
it says nothing of whether the server holds it (below, "Sending"); another's
is applied on top, or beneath this instance's in-flight edit
(`LiveEdit::apply_beneath`). Only another instance's unsettled operation
arriving while this one has an edit in flight rebuilds the view. Applying
reports what it touched (`LiveEdit::apply_touching`), and projections run only
when something they watch was touched (guides/reactive.md). `read` answers
`Err(Pending::Loading)` until the first content and `operate` drops edits made
before then.

`editor.content_of::<C>(block)` reads and edits another block the same way;
the host opens that block's replica on the first frame, read-only unless the
account may edit it, and closes it when nothing holds it.
`editor.related_content::<C>(memo)` follows whichever block a memo names (a
database view follows its database and schema). Revision previews and blocks
still uploading are answered with a host-made `Snapshot` with no session
state. Dynamic artifacts use `EditorHost::content_of` and
`EditorHost::replace_content`. `be_sdk_beui` re-exports `be_block`.

### Reading a range

A type that loads only its first part (`be_block_content::Loads::FirstPart`, such as
`Blob`) leaves the rest of its stored form
in the store: every replica and every editor's `Snapshot` hold
only that part's bytes, and `Snapshot::length` says how long the whole is. An
editor reads the rest with `LiveContent::read_range(offset, length, done)`,
which sends `FromEditor::Read { token, offset, length }`; the replica answers
`ToEditor::Range { token, result }` from the parts it holds, unsaved ones
included, and the type module's `read-part` host call for the parts it left in
the store (guides/type_modules.md). `FromEditor::PutPart { token, bytes }`
goes the other way: the replica puts the bytes as a part of the block and
answers `ToEditor::PartPut { token, result }` with its ref, for an operation
to name (guides/type_modules.md, "Parts named by hash"). Offsets are into the whole stored form.
A blob editor uses `ContentProjection::payload()` instead, a signal of the
payload that reads it once per snapshot (so a header edit reads nothing) and
holds what a host-made snapshot already carried. Chat's editor reads a closed
segment by the range its index gives (`ChatContent::segment_ranges`, from the
header it holds: a snapshot's `length` goes stale as segments close) and checks
the bytes against the index (`read_segment`).

### Sending

An editor that shows what is still sending sends `FromEditor::FollowLacking`
(`ContentProjection::lacking`), again with each snapshot; the replica then
sends `ToEditor::Lacking`: the edits made on this device that the server lacks,
each as its operation's bytes (none past 1 MiB), so the editor maps them to
what they made (chat's messages), whichever editor made them. The first answer
has them all (`reset`), later ones what was added or failed and the ids the
server now holds. The replica records every edit made on this device, followed
or not, and keeps each until the server holds it (`be-sdk-type-guest/src/lacking.rs`):
- until the host answers the submission holding it as `held` (an edit absorbed
  into another goes with it);
- numbered offline (`kept`), until a commit holding it is published to the
  server;
- an edit too large to submit inline goes by its part, so it is followed as the
  others: it waits while its part is put, then until the submission holding it
  is answered (its bytes are not kept past 1 MiB: the editor sees only that
  something is sending);
- one the order passed over (based before a barrier) fails, until the editor
  lets it go, and so does one the server refused as this account may not edit
  the block (`submitted.refused`): the replica stops showing it, what it
  announces is never sent, and what is typed after goes on to be refused in
  turn;
- in a commit kept in this device's outbox (`publish-outcome.kept`), until the
  host says it `drained`, naming the head a drain published (the outbox
  publishes in order, so every commit kept up to it drained, and one kept after
  stays sending), or with `refused` that the server refused it: the
  edit then shows as failed, until the editor lets it go
  (`FromEditor::DismissLacking`, after sending it again or giving it up).

A replica reopened, or opened again after its editors closed, carries on with
what the last one lacked (the module keeps it for the last 64 blocks, dropping
the one carried longest first), and learns from `opening.commits-kept` whether
those kept commits drained, and from `opening.refused-commits` which commits the
server refused that the device still holds. A refused commit stays on the device
after its edit is dismissed, so a replica applies the refusal of each commit id
once (a live `drained` with `refused` always applies, and a replica carries the
ids it applied; it fails every kept or carried edit, not only those of the
refused commits); a commit it did not apply before fails what was kept, and
one it did leaves what is sent after alone. One opened on what this device
numbered offline (`opening.journal-kept`) adds its own edits it replays; a
carried edit is settled by its journal sequence when the journal holds it (by
its bytes only if it has any).

The same list carries what each edit announces (`LiveEdit::announcements`, guides/type_modules.md,
"Announcements"): they go to the host with the submission holding the edit.

What is not kept:
- A host or type module that restarts loses its list of edits. The host opens
  the new replica with `commits-kept` and `server-head`, the head it last heard
  the server hold: with commits kept, the replica loads that head beside its own
  and follows what the content adds over it as kept until they drain
  (`LiveEdit::unsent_over`, a type's own answer; none by default), when it carried
  nothing from an earlier replica. An edit the server refused no longer shows
  "Not sent", and one numbered on this device alone is followed only as the
  replayed journal says.
- A block carried over the module's 64-block limit loses its edits' status: a
  refused message in it shows as sent.
- A replica reopened after a rejoin lists only what the module carried.

In chat, Discard deletes the message with a delete edit, like any other, and
lets the failed edit go. be-sdk-test's `ContentStore::set_online`,
`refuse_kept` and `close_replica` drive this in an editor test.

## State and change callbacks

be-sdk holds app state as plain values and knows no UI framework:
`LiveContent<C>` (the projection without signals), `BlockGraph`
(`EditorHost::graph()`: commands, `info(id)`, `watch(query)`), and
`EditorHost`'s pushed and watched values. A read that may not have arrived
answers `Result<T, Unready>`. Every change is announced by a callback returning
a `Subscription` (unregistered on drop): content `on_touched`, `on_change`,
`on_revision`, `on_lacking`; graph `BlockWatch::on_change`, `BlockGraph::on_info`; host
`on_pushed`, `on_watched::<W>`.

A callback runs once per change, after the whole operation, delivery or answer
is applied, in registration order. Graph callbacks wait while
`EditorHost::defer_graph_changes_unless` says so, until `flush_graph`. Content
callbacks run inside `pump` and `operate`. A read between frames (`ready`,
`read`, `take_projected`) adopts the updates waiting and asks for a pump
(`on_due`) when it took anything a callback announces. Pushed and watched values change
while host frames are delivered, so an adapter queues them and applies them in
its next frame (`on_pumped`). A framework adapts these and nothing else:
be-sdk-beui turns each into a signal, or a pushed or watched value's announcement into a
`Trigger` (`Editor::pushed`), so beui editors never see callbacks.

The editor state over `EditorHost` is plain too (`be-sdk/src/editor_state.rs`):
`EditorCore` (content cache, view state, peers, pan and zoom, reported sizes,
placed children; `begin_frame` reads a `ShownEditor` in framework points, and
reads the host only while its `EditorHost::change_flag` is raised),
`ArtifactsCore`, `CreationCore`, `SettingsLayers<R>` and `SettingsChangeLog`.
be-sdk-beui's `Editor`, `Artifacts`, `Creation`, `Fitting` and
`LayeredSettings` wrap them with signals; what needs beui (placing children,
measuring, popovers, the settings walks, views) stays in be-sdk-beui.

## The region driver

`be_sdk::driver::Driver<R>` is the framework-neutral half of showing views: it
implements `ViewHandler` for any `R: Regions` and is what a framework registers
with `EditorHost::handle_views`. It keeps each view's `Surface` (size, age,
offset, reported damage, and on wasm the damage of recent presents so `paint`
repaints only what the target's age needs), the `Frame` accounting (a stopped
framework reports `Stopped` everywhere; window asks go on the instance's first
shown view; only the keyboard's view carries `repaint_after`), and
`HostServices` routed to the instance whose view last took input. A framework
implements `Regions` (`input`, `frame`, `framed`, `paint`, `anchored`); beui's
is `ModuleRunner` in be-sdk-beui-platform. Anchored views, floated windows and
scrims stay the framework's.

## The graph on the app protocol

An app follows what the host knows of blocks with one resource, `block-watch`,
opened on a `block-subject`: the blocks a `BlockQuery` lists, a block's undo
history, the content at its head, an artifact's state, or its own block's
version status or members (guides/apps/host_services.md, "Naming a block's
members"). `next` answers the `block-value` of the same name once it
changes; listings come from the host's mirror (`be-host-stack/src/app_graph.rs`).
On the app side `editor.watch_blocks(query)` and `editor.blocks()`, held while
anything holds the `BlockList`; be-sdk's `link.rs` keeps one watch per subject
the editor asks for and drops a value that is not of its subject.

On the host side every `be3:blocks` call goes through one dispatcher,
`be-sdk-app-host/src/blocks.rs` (`call`, `ended`, `answers`), over a `BlockStore`:
be-host-runtime's `Instances` (`instances/blocks.rs`, over be-host-stack) and
be-sdk-test's `link.rs` (over its `ContentStore`). The dispatcher keeps each
editor's watches and replicas (`EditorBlocks`): a store offers a `Subject` its
latest value (`EditorBlocks::offer`), and a waiting watch is answered it only when
it differs from what that watch was last answered. A host holds one participant
per editor and block, so of two replicas an editor opens on one block, frames go
only to the latest; when it ends while an earlier one remains, the store drops what
it queued for it and starts the earlier afresh with a snapshot
(`BlockStore::replica_resumed`; be-host sends the replica a resend on its behalf,
and relinks it first if it holds another content type). The store hears a replica
end (`replica_ended`) once no replica holds the block. The two live replicas share
that one participant, so an editor edits through its latest replica: the dispatcher
refuses a `send` through an earlier one (`Denied`), and be-host relays a resumed
replica nothing but a snapshot until one reaches it. An earlier replica of a
content type the host does not link resumes with no link. be-sdk holds one replica
per block, and opens the new one before dropping the old when the content type
changes, so the new one is the latest. The shows watch is a queue the store hands over (`take_shows`). The host's policy stays
in its store: `admit` translates a call's ids through the checkout's scope and
refuses what a preview or a host component may not do, and each method checks and
does the call or answers the error.

Full-text search is a query of its own, not a `BlockQuery`: a `search-watch`
(text and an optional `within` block) answers hits of block, document key,
snippet and match ranges, again whenever an index it read changes
(`editor.search(memo)` in be-sdk-beui; guides/blocks/host.md, "Search").

A `BlockInfo` carries type, author, parent (`BlockLocation`), name, references,
access, artifact source, `locked` (viewable but no key yet, so metadata is
empty), `deleted`, and the server's `size` and `tree_size` in bytes (the
block alone, and with everything below it; zero where the reader only knows
it exists). A size comes from that field, never from walking the children.
`out_of_reach` says the host is offline and holds no version of the block's
content whole (`Peer::held_head`), so it can be neither opened nor changed, nor have a block
added to it, until the host reconnects; the block picker refuses to make a
block there. `BlockInfo::reach` (`Open`, `Locked`, `NoAccess`),
`BlockInfo::label` and `be_sdk_beui::label_glyph` give every place that draws
a referenced block one way to show these states.

**Deleted blocks.** `deleted` is set when the mirror's walk up the parent
chain meets a link that does not hold (`Graph::deleted_root`; where it cannot
read a parent's references, not read yet or locked, it takes `BlockSummary::held`, and a chain into an
invisible block counts as live). Links to it still resolve; places that list
references mute it or list it last, and pickers never offer it.
`EditorHost::restore_block` (`block-command::restore`) lists it in its parent
again, or for a detached block puts it back where `DELETED_FROM` metadata says
(recorded by `be_host_stack::set_parent`), else into the account's Unfiled.

**Acting on a block.** Everything an app asks the host to do with an existing
block is one call, `blocks.act(block, block-command)`: undo and redo, history,
artifact, unlink, delete, move, reorder, place, restore, version control
(`version`, for the editor's own block), `share`, `leave`, `close-editor`,
`set-parent` and `define`. be-host-runtime's `BlockStore::act` matches on the
command: it checks and does `version`, `set-parent` and `define` itself and
hands the rest to be-host (`EditorAction::Command`). `move` takes a block out of
one container's content and into another's through their editors, and sets its
parent unless it moves a link; `set-parent` only sets the graph parent of a
block the app already holds in content of its own, so the two stay apart.

**Making a block.** `create-block(block, block-type, parent, defined, content,
owner-only)` makes one with the id the app picked, so it can use the block
before the host has seen it; be-sdk sends it, `set-parent` and `define` in the
order the app made them (`GraphCommand`). The host takes them only when the
account may edit the block and the parent. `owner-only` makes a block only the
account reads (guides/settings.md, Owner only).

## Presence

Presence is what a peer shows while it is there (a cursor, a selection), never
saved. `SessionMessage::Presence { kind, value }` carries one kind, sealed like
every session message. A replica sends a value only when it changed, keys
others' by `ClientId` and kind, and on hearing of a new participant shows it
what it is showing, so a late joiner sees everyone at once. Editors get
`ToEditor::Presence` and send `FromEditor::ShowPresence`; `editor.peers` and
`editor.on_peer_message` send `Resend` once, so an editor that never reads its
content still hears its peers. On the app side: `editor.show(Some(&value))` and
`editor.peers::<P>()` over a `PresenceKind` (`be_block_content::presence`). The
framework shows a `UserActive` value with a colour from `pick_free_color`
whenever the block is visible.

## Peer messages

`SessionMessage::Channel { channel, payload }` goes to one participant or all,
sealed with the block's key. A participant hears each sender's messages in
order and never its own; nothing is kept, so one not in the session never
hears it. Apps use `editor.send_to_peers(to, &message)` and
`editor.on_peer_message(|from, message| ..)` over a `ChannelKind` (postcard);
`to` is a client id from presence. The terminal is built on it
(guides/terminal.md).

## Editors with their own model

The text editor needs a `Document` that hands out positions, so a cursor stays
on its character while others type: `be_sdk_text::live_text::LiveText` is that
for any `Text` or `RichText` field (a `TextField`, or `FieldOf::rich`/`plain`).
It keeps its own copy of the sequence and marks and turns local edits into
`Change::Text` and `Change::Mark`. `LiveText::sync`, called on every change of
the document and the projection, sends them (and a `break_step` wherever the
editor ends an undo group, `finish_history_group`), asks the host to undo or redo,
and applies others' edits in place. The projection must `record()` while a
`LiveText` reads its log (`stop_recording()` after), and only one `LiveText`
per projection syncs at a time: the canvas syncs the text being edited and has
the others take in their whole field. Carets travel as presence through
`be_sdk_text::SharedCaret` (`remote_carets` turns peers' into a `TextArea`'s
`remote_cursors`).
