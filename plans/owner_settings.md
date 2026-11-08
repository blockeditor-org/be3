# Settings only their owner reads

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
  escrow key. `mint_scope` and `Peer::tend` skip escrow for it, and tending
  never treats the missing escrow seal as work.
- **The server hides it.** Only its author can list, read or watch an Owner
  kind's block; administrators cannot see that it exists. Only the author's own
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

1. `KindVisibility::Owner`, the escrow exceptions, and the server's rules.
2. Tests: an administrator neither lists nor opens it; a new device of the
   owner opens it; a new recovery phrase reseals it.
3. Update guides/settings.md and guides/blocks/keys.md (Escrow), and add a
   decision: owner-only settings are never escrowed.

## Not planned here

- An organization policy that escrows owner-only settings anyway.
