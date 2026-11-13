# API simplification

Depends on: none.

Places where many requests, WIT functions or public functions do one job.
Each step replaces them with fewer, conceptually simpler ones that behave
the same. The steps are independent unless a step says otherwise, and each
lands on its own. Step numbers stay as written when a step is done and
removed. No format is frozen, so wire and WIT changes need no migration. Line
numbers are from when this plan was written.

## 2. The shell's one-way reports are one batched call (left)

`shell.tell` and `shell-message` replaced the 28 one-way functions, and
`EditorHost` keeps what an editor tells the shell in one ordered queue
(`told`, `EditorHost::tell`), so messages go in the order the editor made
them in; `be-sdk-test` has a test of that order. What is left of this step:

- The queued messages come first in `outbound`; the ones that need session
  state stay computed there (`DragAccepted`, `HandlesDismiss`, `LeaveFrame`,
  `Paste`, `Picked`, `Focused`, `ChildMenuPick`, `PlayAudio`, the menus, the
  card, the artifact and the intrinsic size), so their order is still fixed.
  Make each push at the moment the editor asks where it does not need session
  state, and compute the child menu pick's anchor when it is queued.
- `bar-action` has one variant, `details`; drop the enum and make it a
  `details` message (and `child-bar` a `details` event).

## 3. The editor's one-way methods are one event call (left)

`editor.hear` and `editor-event` replaced the 14 one-way methods, and the host
makes `EditorEvent`s directly (`Outbound::Event`); `EditorCall` keeps only
what is not an event, and `child-bar` and `child-focused` carry the child
only. What is left: the host's `docks::take_bars` still carries a `ViewId`
nothing reads.

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

## 15. Host, CLI and test harness

- `BeuiTest` (`be-sdk-test/src/beui.rs`): remove the one-line `take_*`
  wrappers that only pick a `ShellMessage` variant (about 15, with 3 to 17
  callers each across the app tests) in favour of `take_shell_messages(pick)`,
  keeping those that decode something. Remove `take_copies`,
  `take_copied_text` and `take_copied_images` in favour of
  `take_copied(format)`, with the serial of a copy in a separate accessor if a
  test needs it (`take_copies` has two callers in be-sdk-beui-platform).
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
