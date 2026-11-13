# API simplification

Depends on: none.

Places where many requests, WIT functions or public functions do one job.
Each step replaces them with fewer, conceptually simpler ones that behave
the same. The steps are independent unless a step says otherwise, and each
lands on its own. Step numbers stay as written when a step is done and
removed. No format is frozen, so wire and WIT changes need no migration. Line
numbers are from when this plan was written.

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

## 12. beui: one form per concept

- `StoredCell::new(x)` is the one form for `Copy` values; migrate the
  `store(Cell::new(..))` uses. A first try showed most are not plain: they hold
  non-`Copy` values (a callback, a state struct) or read the cell with
  `with`/`try_with`, so each needs its own look (`StoredCell` has `replace`,
  `take` and `set`; add what a use needs).

Update beui-demo for every component that merges or goes.

## 15. Host, CLI and test harness

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
