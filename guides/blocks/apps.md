# Blocks on the app protocol

Part of the block stack (guides/the_new_block_stack.md). How an editor reads
and changes blocks through the host. The protocol is the `be3:app/blocks` WIT
interface (`crates/be-sdk-api/wit/blocks.wit`); every call is made under the
editor's resource, so the host knows the instance and closing the editor ends
what it opened. The host side is guides/blocks/host.md; the replica the editor
follows is guides/type_modules.md.

## Content on the app protocol

An editor is a follower of the block's replica ("Editors are endpoints" in
guides/type_modules.md). Everything about a block's content crosses through a
`replica` resource the editor opens per block: its own (on its first frame) and
each block it watches. `replica.frames()` streams host-to-editor frames,
back-pressured by be-wasm-ops credits; `replica.send(frame)` is answered once
the host took the frame, and the editor keeps at most a window unanswered (the
rest wait in `EditorHost`). A frame is an encoded
`be_sdk_type_guest::editor::ToEditor` or `FromEditor`, relayed unread between
the instance and the replica.

- App to host: `Operate`, `Gesture`, `ShowPresence`, `SendToPeers`, `Resend`,
  `Read` (below).
  An instance asks for content by sending `Resend` when it first follows a
  block; the host attaches it then, with `editable` from the account's access
  (sent again when that changes).
- Host to app: `Snapshot { content_type, bytes, length, session, unsettled,
  applied }`, where `length` is the whole stored form's,
  where `applied` counts operations *this instance* sent that the snapshot
  holds, which is how an app tells its own unacknowledged edits from others';
  `Operations` (each marked `mine`, with `unsettled` and, for a peer's
  operation that goes beneath them, the rebased `beneath`); `Presence`;
  `PeerMessages`; `Range`.
- `write-content` writes content outright, since it may name a block nothing
  follows (guides/blocks/host.md, "Creating, seeding and replacing").

On the app side, `editor.block_content::<C>()` is a `ContentProjection` built
on `be_sdk_type_guest::editor::EditorMirror<C>`. `operate` applies to what the
view sees and queues for the host. A `mine` operation coming back only
confirms; another's is applied on top, or beneath this instance's in-flight
edit (`LiveEdit::apply_beneath`). Only another instance's unsettled operation
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

A type that loads only its first part (`be_block::Loads::FirstPart`, such as
`Blob`) leaves the rest of its stored form
in the store: the replica, its followers and every editor's `Snapshot` hold
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
holds what a host-made snapshot already carried.

## State and change callbacks

be-sdk holds app state as plain values and knows no UI framework:
`LiveContent<C>` (the projection without signals), `BlockGraph`
(`EditorHost::graph()`: commands, `info(id)`, `watch(query)`), and
`EditorHost`'s pushed and watched values. A read that may not have arrived
answers `Result<T, Unready>`. Every change is announced by a callback returning
a `Subscription` (unregistered on drop): content `on_touched`, `on_change`,
`on_revision`; graph `BlockWatch::on_change`, `BlockGraph::on_info`; host
`on_pushed`, `on_watched::<W>`.

A callback runs once per change, after the whole operation, delivery or answer
is applied, in registration order. Graph callbacks wait while
`EditorHost::defer_graph_changes_unless` says so, until `flush_graph`. Content
callbacks run inside `pump` and `operate`; pushed and watched values change
while host frames are delivered, so an adapter queues them and applies them in
its next frame (`on_pumped`). A framework adapts these and nothing else:
be-sdk-beui turns each into a signal, so beui editors never see callbacks.

The editor state over `EditorHost` is plain too (`be-sdk/src/editor_state.rs`):
`EditorCore` (content cache, view state, peers, pan and zoom, reported sizes,
placed children; `begin_frame` reads a `ShownEditor` in framework points),
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

An app watches a `BlockQuery` with a `blocks-watch` resource, answered from
the host's mirror whenever the answer changes (`be-host-stack/src/app_graph.rs`);
on the app side `editor.watch_blocks(query)` and `editor.blocks()`, held while
anything holds the `BlockList`.

A `BlockInfo` carries type, author, parent (`BlockLocation`), name, references,
access, artifact source, `locked` (viewable but no key yet, so metadata is
empty), `deleted`, and the server's `size` and `tree_size` in bytes (the
block alone, and with everything below it; zero where the reader only knows
it exists). A size comes from that field, never from walking the children. `BlockInfo::reach` (`Open`, `Locked`, `NoAccess`),
`BlockInfo::label` and `be_sdk_beui::label_glyph` give every place that draws
a referenced block one way to show these states.

**Deleted blocks.** `deleted` is set when the mirror's walk up the parent
chain meets a link that does not hold (`Graph::deleted_root`; where it cannot
read a parent's references it takes `BlockSummary::held`, and a chain into an
invisible block counts as live). Links to it still resolve; places that list
references mute it or list it last, and pickers never offer it.
`EditorHost::restore_block` (`block-command::restore`) lists it in its parent
again, or for a detached block puts it back where `DELETED_FROM` metadata says
(recorded by `be_host_stack::set_parent`), else into the account's Unfiled.

**Graph commands.** `create-block(block, block-type, parent, defined,
content, owner-only)`, `set-parent` and `define` are sync calls with no answer:
the app picks a new block's id, so it can use the block before the host has
seen it. The host takes them only when the account may edit the block and the
parent. `owner-only` makes a block only the account reads (guides/settings.md,
Owner only).

## Presence

Presence is what a peer shows while it is there (a cursor, a selection), never
saved. `SessionMessage::Presence { kind, value }` carries one kind, sealed like
every session message. A replica sends a value only when it changed, keys
others' by `ClientId` and kind, and on hearing of a new participant shows it
what it is showing, so a late joiner sees everyone at once. Editors get
`ToEditor::Presence` and send `FromEditor::ShowPresence`; `editor.peers` and
`editor.on_peer_message` send `Resend` once, so an editor that never reads its
content still hears its peers. On the app side: `editor.show(Some(&value))` and
`editor.peers::<P>()` over a `PresenceKind` (`be_block::presence`). The
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
the document and the projection, sends them, asks the host to undo or redo,
and applies others' edits in place. The projection must `record()` while a
`LiveText` reads its log (`stop_recording()` after), and only one `LiveText`
per projection syncs at a time: the canvas syncs the text being edited and has
the others take in their whole field. Carets travel as presence through
`be_sdk_text::SharedCaret` (`remote_carets` turns peers' into a `TextArea`'s
`remote_cursors`).
