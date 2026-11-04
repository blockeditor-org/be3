# Settings

A setting lives in a block of its kind's type, and the blocks of one kind are read as layers.
Who may change a setting is ordinary block access (`be_block_graph::Access`).
guides/blocks/workspaces.md covers the account's Settings block and device profiles, writing a
layered setting from an editor, and the settings the host applies itself.

## Where settings are kept

- **The user**: a root `be_block::Settings` block the user authored (`be_sdk::root_settings`,
  `be-host/src/root_settings.rs`). Its `kinds` map points to one block per kind (every device),
  and a `SettingsProfile` per device does the same for that device.
- **A block**, usually a workspace: its `SETTINGS` metadata points to a child `Settings` block
  with a `kinds` map. The two edit kinds, the prefix registry and tags
  (`be_block_graph::EDIT_KINDS`), are instead children of the block itself under their own
  metadata keys (`be_block::edit_kind_key`).
- **The organization**: a `Settings` block with the derived id
  `be_block::organization_settings(organization)`, holding defaults (`kinds`) and `policies`. An
  administrator's client makes it on first opening a company organization
  (`Peer::ensure_organization_settings`); personal organizations have none.

`be_block::my_files(organization, account)` is the other derived id: each account's My files,
kept by the server for that account alone.

## Kinds and resolution

A kind implements `be_block::Layered`; its `FOLLOWS`, `RULE` and `VISIBILITY` constants decide
whether the block layer applies, whether it locks, and who reads it. Find the kinds by grepping
for `impl Layered`; most are in be-block (`workspace_settings.rs` for prefixes, tags and Home).
Default editors have no block layer; a block keeps its own preferred editor in
`PREFERRED_EDITOR` metadata (decisions/default_editors_have_no_workspace_layer.md).

`be_block::Resolution::resolve` stacks the layers; `source_of` and `locked_by` say which layer set
or locks a field, for the settings pages.

In an editor (`be-sdk-beui/src/editor/settings.rs`), `Editor::kinds_above(block)` walks up and
takes, per kind, the nearest kind it meets (kinds aren't merged across ancestors), stopping at the
first ancestor the viewer can't view and filling the rest from `BlockQuery::OpenSettings`. It is
a memo pushed when anything on the walk changes. `Editor::resolved_settings` combines it with the
organization's layers and the user's; `be_sdk_beui::LayeredSettings` is the per-setting handle
the settings pages use.

## Access

A block's access is the highest grant on its path up
(decisions/block_access_is_the_highest_grant_on_the_path_up.md); only Manage shares. Settings
blocks and edit kinds take their access from their parent
(decisions/settings_blocks_take_their_access_from_their_parent.md,
decisions/prefixes_and_tags_are_edit_kinds_held_by_the_block.md): `level_below` in
`be-block-graph/src/lib.rs` has the rules. The server enforces the matching changes: making,
moving or dropping a block's settings needs Manage on it (`require_keeping_settings` in
`be-server/src/blocks.rs`). These blocks are never listed among their owner's children.

## Visible inside

A kind with `KindVisibility::Inside` (prefixes, tags) is granted View to the pseudo-account
`ANYONE_INSIDE`, so anyone who views a block inside its owner reads it, and a block shared alone
still resolves row ids and shows tags (decisions/prefixes_and_tags_are_visible_to_anyone_inside.md).
`Listing::OpenSettings(block)` (`BlockQuery::OpenSettings` to apps, `list_open_settings` on the
server) returns the nearest open kind above `block` among ancestors the viewer can't view, and is
pushed again when one changes. Losing view of every block inside, or a kind closing, queues a
rotation of its key.

## Prefixes and tags

`PrefixRegistry` is the only way a row id resolves
(decisions/database_prefixes_resolve_only_through_the_prefix_registry.md); guides/tracker.md
says how `be` uses it. `Tags` lists `Tag` blocks; `TAGGED` metadata on a block points to its
tags, and a tag's blocks are its backrefs. Personal tags are the same kind in the user's
settings. The editor helpers are in `be-sdk-beui/src/editor/tags.rs`.

## The organization and the server

The server has an account of its own (`ServerKey`, `be-server/src/organization.rs`). A setting
the server enforces is shared with it (`Peer::share_with_server`), and it refuses what a policy
governs while it can't read the policy
(decisions/server_enforced_settings_are_shared_with_the_server.md). The server seals the
organization settings block's key to every member itself; clients accept that key for this
block alone and only to read (decisions/a_server_sealed_key_never_seals_anything.md).
`OrganizationPolicies` are enforced by the server (`ServerStore::policies`). A personal account
never deletes anything on losing access
(decisions/personal_account_data_is_never_deleted_automatically.md).

## The UI and tests

be-app-settings holds the pages. The user's `Settings` block opens as a list
(`root/ui.rs`, docked as the workspace's Settings panel), and every row in it opens a page in the
main area, whose row stays selected while it is focused; the list itself changes nothing.
The pages are the kind blocks (Display, Input, Default editors, and Offline, `root/organization.rs`),
each profile (`SettingsProfileView`), and three more editors of the `Settings` type opened with
`open_block_with`: `devices` and `storage` on the user's block (`root/devices.rs`,
`root/storage.rs`: the account's usage against its quota, and a treemap of its blocks by
`tree_size`), and `organization` on the organization's (`root/organization.rs`). `root/block.rs` is a block's settings: what is set here,
who may read each kind, what comes from above. "Settings" in the file tree's and shell's block
menus opens or makes a block's settings.

Tests: be-block's `settings_resolve_from_the_organization_the_nearest_block_and_the_viewer`;
be-block-graph's access tests, including random graphs (`src/tests.rs`); be-server's settings
and open-inside tests; be-block-client's
`an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server`; be-tool-cli's
`a_row_id_resolves_through_the_workspaces_prefixes`; and paintings in
`crates/be-app-settings/src/tests/`.
