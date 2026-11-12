# API simplification

Depends on: none.

Places where many requests, WIT functions or public functions do one job.
Each step replaces them with fewer, conceptually simpler ones that behave
the same. The steps are independent unless a step says otherwise, and each
lands on its own. Step numbers stay as written when a step is done and
removed. No format is frozen, so wire and WIT changes need no migration. Line
numbers are from when this plan was written.

## 2. The shell's one-way reports are one batched call (left)

`shell.tell` and `shell-message` replaced the 28 one-way functions. What is
left of this step:

- `EditorHost`'s per-kind `take_*` queues (`be-sdk/src/host.rs`) become one
  ordered `Vec<ShellMessage>`; `outbound` (`be-sdk/src/editor_session.rs`)
  still reads each queue and orders messages by kind, so the order across
  kinds is that fixed order, not the order the editor made them in. No test
  covers the order of a pass's messages (the test of that name in
  `be-host-runtime` is about desktop host calls); add one in `be-sdk-test`.
- `bar-action` has one variant, `details`; drop the enum and make it a
  `details` message (and step 3's `child-bar` a `details` event).

## 3. The editor's one-way methods are one event call (left)

`editor.hear` and `editor-event` replaced the 14 one-way methods; the host
queues events and sends them as one list per `Outbound` batch
(`Wire::flush_hearing`), and the guest turns each into the same closure as
before (`be-sdk/src/wire.rs`). What is left: the host's internal `EditorCall`
enum (`be-host-runtime/src/calls.rs`) still has a variant per event and
`Wire::call` converts it; make `EditorEvent` the one type, so `Instances`
makes events directly. `child-bar` and `child-focused` still carry the `view`
the guest ignores in `EditorCall` (the WIT dropped it).

## 4. Block watches are one resource

`blocks.wit` has `blocks-watch(query)`, `history-watch(block)`,
`head-watch(block)`, `artifact-watch(block)` and `version-watch()`. The host
already merges them into one `Subject` enum
(`be-host-runtime/src/instances/blocks.rs`).

Replace them with:

```wit
variant block-subject { blocks(block-query), history(block-id), head(block-id), artifact(block-id), version }
resource block-watch { constructor(subject: block-subject); next: async func() -> block-value; }
```

The guest's copied `watch_X` / `take_X_watch` / reconcile blocks
(`be-sdk/src/link.rs`, `be-sdk/src/host.rs`) become one, which checks that a
value matches its subject. `show-requests` stays separate: it is a queue.

## 5. Acting on a block is one call

Today a block is changed through `blocks.act(block, block-command)`,
`blocks.version(block, version-command)`, the `act` shell message (`block-act`: a
block and a `shell-command`) and the graph calls `create-block`, `set-parent` and `define`.
The guest's `GraphCommand` (`be-sdk/src/graph.rs`) is split back into three
calls by `be-sdk/src/link.rs`.

Add `version(version-command)`, `share`, `leave`, `close-editor`,
`set-parent(block-location)` and `define(…)` to `block-command`, leaving one
`blocks.act(block, command)` plus `create-block` (which answers with an id).
`shell-command` goes. Permission checks stay a match on the command in the one
host handler (`be-host-runtime/src/instances/blocks.rs`). While there, decide
whether `set-parent` and the existing `move-command`/`reorder-command` are one
thing, and keep one.

## 7. The host's per-instance queues are one pushed stream

`be-host-runtime/src/runtime.rs` has 19 `take_*` functions that be-host polls
every frame, 14 of them per instance (`take_child_focus`,
`take_child_dismissals`, `take_child_closes`, `take_child_commits`,
`take_focus_report`, `take_artifact_watch`, `take_child_menu_picks`,
`take_artifact_outcome`, `take_created`, `take_leaving`, `take_rendered`,
`take_view_changes`, `take_bar_actions`, `take_focus_request`,
`take_view_actions`, `take_block_drags`), each with one caller in be-host.

Replace them with one `InstanceEvent` enum delivered to its owner through
`host::Desk<Instances>` (`be-host-runtime/src/instances.rs`) rather than
polled, so nothing checks each frame. Keyed queries (`take_child_focus(child)`,
`take_focus_request(region)`) become matches on the event. After step 2 most
events are a `ShellMessage` plus the instance, so do this after it.

`host::push_command` / `take_commands` / `HostCommand { RestartApp }`
(`be-host-base/src/host.rs`) is a second queue with one producer and one
consumer; it becomes a `Desk<Instances>` message too.

## 8. The stack pushes its changes

`be-host-stack/src/lib.rs` pairs getters with counters be-host polls every
frame: `content_revision`, `contacts_revision`, `storage_revision`,
`stale_revision`, `graph_revision`, and `take_touched`. Replace them with one
change notice the worker posts through the mailbox, carrying what changed
(`contacts`, `storage`, `stale`, `graph`, and the blocks whose content
changed), and drop the `seen`/`published` bookkeeping in be-host. Settings
sync compares content per block (`be-host/src/local_settings.rs`), so the
content part is a set of blocks.

`Versions` (`be-host-stack/src/version.rs`) also takes a watch for every block a
status reads and never lets go: blocks that leave a root's subtree and closed
checkouts stay watched for the life of the connection. Release them when the
change notice reports what left.

## 9. Block protocol requests that duplicate others

- `read-block` is `list-blocks(listing.block)`: the server implements it as
  `list(Listing::Block(block)).pop()`. Remove it. Make `Peer::list(listing)`
  public and remove the wrappers that only build a `Listing` (`shared`,
  `deleted`, `backrefs`, `open_settings`, `referenced`;
  `be-block-client/src/peer.rs`), which the worker takes apart again
  (`be-host-stack/src/worker.rs`). Keep `children` (about 300 callers) and
  `summary`'s offline handling as thin forms over `list`.
- `missing-objects` and `upload-urls` become one
  `upload-urls(objects) -> list<option<upload-url>>`, none for objects the
  server holds, like `download-urls`. Reserve quota only for the missing ones,
  in the same call. One round trip less on every publish.
- Remove the per-block `watch` stream, `block-event` and `head-moved`: the
  `changes` stream already reports every head change for blocks the
  connection read, and no consumer reads `head-moved.author`. `changes` gains
  the originating client so a client skips its own publishes; deletion is a
  summary with parent `detached`. The client's watch registry
  (`be-block-client/src/connection.rs`) goes.
- One convention for change streams: a stream carries the latest value and
  starts with it. Remove `storage-usage` (only `be-block-frozen` calls it);
  make the key-work stream `stream<key-work>` and remove `get-key-work`.
- Invitations and contact links are one link family. Both are a code-sealed
  card, a lookup id, and an accept that makes two accounts contacts. Replace
  `invite`/`get-invitation`/`accept-invitation` and the five
  `*-contact-link` calls with `create-link(…, kind)`, where `kind` is
  `contact(option<pending-grant>)` or `invitation{organization, email, role,
  escrow}`, plus `get-link`, `accept-link`, `list-links`, `delete-link`. The
  rule splits on `kind` (an invitation needs `AdministratorOf`). Outstanding
  invitations become listable and revocable. Update guides/blocks/keys.md.
- Agents are members: `remove-member` lets an agent's creator remove it, and
  `list-members` with a `created-by` field replaces `list-agents`. Remove
  `list-agents` and `remove-agent`. `remove-agent` never calls
  `key_work_changed` while `remove-member` does; the merged call must.
- `Peer`'s `join_session`, `leave_session`, `claim_ownership` and `heartbeat`
  only forward to `Connection` behind an always-`Ok` `Result`; callers use the
  connection, as they already do for `append`.

Update guides/blocks/server.md and guides/blocks/client.md.

## 10. GPU: one poll answers everything that settled

The gpu interface (`be-wasm-gpu-abi/wit/device.wit`) mirrors WebGPU and wgpu's
custom backend one to one, and keeps doing so: render bundle encoders, per
encoder debug markers and typed `create-*` calls stay as WebGPU has them.

What WebGPU expresses as promises (`mapAsync`, `onSubmittedWorkDone`,
`device.lost`) is polled across the boundary today: `objects::settle`
(`be-wasm-gpu-guest/src/objects.rs`) calls `poll`, then `map-state` per
pending buffer, `submitted-work-done` per ticket, and `device-lost`. That is
plumbing under the promises, not WebGPU's shape. Make `map-async` return a
ticket as `on-submitted-work-done` does, and have
`poll: func(wait: bool) -> settled` return
`record settled { maps: list<tuple<u64, bool>>, work-done: list<u64>, lost: option<device-lost-info> }`.
Remove `map-state` and its enum, `submitted-work-done` and `device-lost`. The
host queues settled events per app. The calls stay `local`
(decisions/guest_calls_are_async_ops_except_local_ones.md).

## 11. Type modules answer everything at start

- At hello the host calls `blank` and then `describe(blank)` for every type
  (`be-block-type-host/src/modules.rs`) only to fill `Kind`. Add `blank` and
  `blank-description` to `type-info` and remove the `blank` export
  (`be-sdk-type-api/wit/replicas.wit`). Update guides/type_modules.md's list of
  stateless requests.
- Remove `description.name`: no host reads it, and `derived` already carries
  the intrinsic name.
- Fold `undo-state` into the replica `status` report and remove `history`
  (`be-sdk-type-api/wit/host.wit`): both are snapshots of one replica that the
  host stores.
- Remove the manifest's `plain-file` (`be-sdk-manifest/wit/manifest.wit`): it
  repeats `type-info.plain`, and a test exists only to keep the two in step.
  The host fills it from the type registry.
- `be-sdk-type-host` registers with `Registration::of::<X>().undo::<X>()`;
  use `be_block_types!` like every other crate.

## 12. beui: one form per concept

- One focus ring: styled `Frame` takes `focus=` (drawn as an outline with an
  offset). It replaces `focus_border()`, `beui_view::reactive::focus_outline()`
  and the hand-drawn `focus_ring(..)` + accent border copies (`TileButton`,
  `SwatchButton`, tree, treemap, colour picker); `FocusRing` becomes a `Frame`
  with `focus`. Paintings change; accept them after looking.
- Key listeners: remove `on_global_key` (it is `on_key(KeyAt::Window,
  KeyPhase::Capture, …)`), make `on_scroll_key`'s "don't reveal focus" an
  option of `on_key`, and move `on_shortcut` users to actions. Update
  guides/beui_keyboard.md.
- Menus: remove the public `on_select: Callback<Vec<usize>>` from styled
  `ContextMenu`, `MenuPopup`, `MenuRows`, `MenuButton` and `SplitButton`; an
  item says what it does with `on_click` or `action`. The three product
  callers (`be-host/src/ui/onboarding.rs`, `be-host-accounts-ui/src/lib.rs`,
  `be-app-logic-grid/src/root/hotbar_ui.rs`) move to item callbacks. Path
  routing can stay inside unstyled if `MenuNodes` needs it. Separators are
  written one way.
- `StoredCell::new(x)` is the one form for `Copy` values; migrate the
  `store(Cell::new(..))` uses. A first try showed most are not plain: they hold
  non-`Copy` values (a callback, a state struct) or read the cell with
  `with`/`try_with`, so each needs its own look (`StoredCell` has `replace`,
  `take` and `set`; add what a use needs).

Update beui-demo for every component that merges or goes.

## 13. beui: `Glide` and `Selectable` leave the base layer

`Glide` and `Selectable` (`beui-view/src/primitives.rs`) are single-child
pass-through base nodes, which decisions/beui_base_layer_kept_small.md rules
out, each with a body per core (`beui-view/src/components/`,
`beui-view/src/html/`). Move them up:

- `Glide` (used only by `beui-components-unstyled/src/reorder.rs`) becomes
  unstyled code. It animates a node from where it was to where layout puts it;
  build it from what the base layer already offers, a frame's paint offset and
  the node's laid-out rect, with timing on the frame clock. Add a `Frame`
  prop only where neither is enough. `set_raised`, `hold_glide` and
  `glide_displacement` move with it.
- `Selectable` (used only by `beui-components-styled/src/selectable_text.rs`)
  becomes an unstyled component: the selection state, gestures, copy and
  select-all live there, over the base `Text`'s hit testing and its selection
  highlight. Styled `SelectableText` keeps its look.

Both cores lose their implementations; tests that use the base nodes directly
(`beui-host/src/tests.rs`, `beui-demo/src/base_pages.rs`) move to the new
components.

## 14. beui: density only sizes the tree

Density is not ready to support everywhere. Today `Space` and `Size` resolve
through `use_density()`, `use_space` and `use_size` build a dynamic prop when
the two densities differ (`resolved` in
`beui-components-styled/src/tokens.rs`), and `.px()` resolves at Compact,
so the two disagree (`RadioGroup` uses `.px()`, `SegmentedControl` uses
`use_space`).

- `Space`, `Padding`, `Size` and `Length` resolve to one value each: remove
  `resolve(density)`, `resolved` and the Comfortable tables, and keep one way
  to get pixels (`.px()`; `use_space` and `use_size` go).
- The styled `Tree` (`beui-components-styled/src/tree.rs`) keeps reading
  `use_density()` and owns its Comfortable metrics (row height, indent,
  spacing, glyph and chevron sizes) as its own table. be-app-file-tree's tree
  rows keep reading it too.
- `provide_density` (`be-sdk-beui/src/beui_frame.rs`) stays: it is how a
  phone frame reaches the tree.
- Update guides/beui/components.md: density sizes the tree only.

## 15. Host, CLI and test harness

- `BeuiTest` (`be-sdk-test/src/beui.rs`): make `take_calls` public as
  `take_shell_messages(pick)` beside `take_host_calls`, and remove the
  one-line `take_*` wrappers that only pick a `ShellMessage` variant (about 15,
  with 3 to 17 callers each across the app tests), keeping those that decode
  something. `take_copies`, `take_copied`, `take_copied_text` and
  `take_copied_images` become `take_copied(format)`, with the serial of a copy
  in a separate accessor if a test needs it (`take_copies` has two callers in
  be-sdk-beui-platform).
- `HostTest` (`be-host-tests/src/lib.rs`): `start`, `start_desktop`,
  `start_in_space`, `start_drawing_previews` and `start_with` become
  `start(Options)`.
- `be` CLI (`be-tool-cli/src/cli.rs`): a row's page is a text block target.
  `be ROW page` resolves to it, made on first write, and the text verbs
  (`cat`, `write`, `append`, `edit`) work on it; remove the copied `page`
  verbs and `set --body/--append`. Keep `new … --body`. `add-option` merges
  into `option`. `workspaces` becomes `ls`, whose `--json` carries
  `writable`. Update guides/tracker.md.
- `desktop.wit` `program-action`: `run(string)` and `exec(program-exec)`
  become one `exec`, and `DesktopHost::launch` (a shell line, clashing with
  WIT `launch`) goes; a typed line is wrapped as the platform's shell command
  by the launcher. `DesktopHost`'s `set_owned`, `set_blank_after`,
  `set_lock_after` and `set_primary_selection` become
  `configure(DesktopSettings)`; `set_locked` stays, being state.
- `drive` (`beui-layer-drive`): `find`, `reveal` and `shown` become
  `find TARGET [reveal|shown]`, and `gone` becomes `wait not TEXT`. Update
  guides/running/drive.md.

## Smaller cleanups found alongside

- `shell-dialog` is defined in `host-services.wit` but used only by `app.wit`;
  move it.
- The app protocol's `size {width, height}` is converted to and from `vec2`
  in many places; use `be3:ui`'s `vec2`.
- `preview-revision` and `release-revision` (`host-services.wit`) become a
  `revision-preview` resource whose drop releases it, like `replica`.
- be-sdk-beui re-exports `ChildHandle as ChildBlockHandle`; name the type
  `ChildBlockHandle`.
