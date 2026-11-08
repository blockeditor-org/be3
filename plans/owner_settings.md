# Settings only their owner reads

Depends on: none.

A settings kind an app can mark private to its owner: synced across the
owner's devices, readable by no one else, organization administrators and the
server included. The first use is an app's key for an outside service
(plans/boat.md). Not implemented yet.

## What exists

- A kind is a `be_block::Layered` content type; `Settings::kinds` maps content
  types to blocks, so an app's types crate can define one (`EditorSettings` in
  `be-type-settings`). Each account keeps its own under its top-level
  `Settings` block in each organization (guides/blocks/workspaces.md).
- `KindVisibility` (`crates/be-block/src/settings.rs`) is `Inside` or
  `Members`.
- In a company organization the server lets administrators know of every
  block (the `administrator` branches in `be-server/src/blocks.rs`), and
  `mint_scope` (`be-block-client/src/keys.rs`) seals every new scope key to the
  escrow key too, so administrators open everything (guides/blocks/keys.md,
  Escrow). `Peer::tend` seals any scope key not yet escrowed.

## The design

- **`KindVisibility::Owner`.** A kind with it is a block that is its own scope
  root, whose key is sealed only to the account's identity and never to the
  escrow key.
- **A scope root by rule.** `BlockGraph::is_scope_root` counts a block under
  another only when it has a viewable grant of its own, and a kind block under
  `Settings` has none. Add a graph rule: a block marked owner-only is a scope
  root.
- **The server holds the mark.** `KindVisibility` is a const on a Rust type
  that only clients see, but the server computes escrow work itself
  (`unescrowed` in `be-server/src/keys.rs`). So the mark is stored on the
  block: an `owner-only` flag on the protocol's `create-block`
  (`be-block-protocol/wit/blocks.wit`), set from the app protocol's
  `create-block` when the kind's type says `Owner`, and kept for the block's
  life. The server never lists such a block as `unescrowed`, and
  `mint_scope`, `Peer::tend` and `Peer::rotate` (which mints new scope
  epochs) skip escrow for it.
- **The server keeps it closed.** Only its author can list, read or watch an
  Owner kind's block; administrators cannot open it. They can see that it
  exists, since the account's `Settings` references it. Only the author's own
  account may make, move or grant it, beside `require_keeping_settings`. If
  that were bypassed, nothing could be opened anyway, since the key is not
  escrowed.
- **Losing the recovery phrase loses it.** The settings page says so; an app
  holding a key just asks to connect again. An administrator can still delete
  the account, which deletes the block with it.
- **Per organization.** Settings blocks belong to one organization, so a key
  connected in a company organization never reaches the personal one, nor the
  other way round.
- **Syncing.** Like every kind, it syncs to every device unless a device's
  profile keeps it.
- Ordinary settings stay readable by administrators.

## Order of work

1. `KindVisibility::Owner`, the `owner-only` flag on `create-block`, the
   scope-root rule, the escrow exceptions (`mint_scope`, `tend`, `rotate`,
   `unescrowed`) and the server's rules.
2. Tests: an administrator neither lists nor opens it; no escrow key work is
   ever listed for it, after creation or a rotation; a new device of the
   owner opens it; a new recovery phrase reseals it.
3. The settings page's notice that losing the recovery phrase loses these
   settings, with a painting.
4. Update guides/settings.md and guides/blocks/keys.md (Escrow), and add a
   decision: owner-only settings are never escrowed.

## Not planned here

- An organization policy that escrows owner-only settings anyway.
