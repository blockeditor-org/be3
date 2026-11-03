# Settings

A setting lives in a block of its kind's type, and the blocks of one kind are
read as layers. Who may change a setting is ordinary block access
(`be_block_graph::Access`: `None < KnowExists < View < Edit < Manage`).

## Where settings are kept

- **The user.** A root `be_block::Settings` block the user authored, found by
  filtering the roots by content type and author (`be_sdk::root_settings`,
  `be-host/src/root_settings.rs`). Its `kinds` map points to one block per
  kind (the every-device layer); a `SettingsProfile` per device does the same
  for this device.
- **A block**, usually a workspace. Its `be_block::SETTINGS` metadata (a
  `BlockRef`) points to a child `Settings` block whose `kinds` map points to one
  block per kind. Two kinds, the prefix registry and tags
  (`be_block_graph::EDIT_KINDS`), are not in it: each is a child of the owner
  block itself, held by a metadata key of its own (`be_block::edit_kind_key`,
  giving `PREFIXES` and `TAG_DEFINITIONS`).
- **The organization.** A `Settings` block at the top of a company
  organization whose id is `be_block::organization_settings(organization)`.
  Its `kinds` are the organization's defaults and its `policies` its policies.
  An administrator's client makes it when it first opens the organization and
  shares it with the server (`Peer::ensure_organization_settings`). The default
  organization has none.

`be_block::my_files(organization, account)` is the other derived id: each
account's My files, which the host makes on opening an organization
(`RootSettings::ensure_my_files`) and which the server keeps for that account
alone (`my_files_of` in `be-server/src/organization.rs`).

## Kinds

A kind is a type implementing `be_block::Layered`: a `Root` with `NAME`,
`over` (how an upper layer combines with a lower one; `be_block::union` merges
maps) and three constants:

- `FOLLOWS`: `Block` (the data's settings, resolved from where the block lives)
  or `Viewer` (the default: the block layer is skipped).
  [decisions/settings_follow_the_block_or_the_viewer.md]
- `RULE`: `Default` (the default: later layers win) or `Policy` (the block
  layer's set fields are fixed for the user's own layers).
- `VISIBILITY`: `Members` (the default) or `Inside`, below.

| Kind | Crate | FOLLOWS | RULE | VISIBILITY |
| --- | --- | --- | --- | --- |
| `DisplaySettings`, `InputSettings` | be-block | Viewer | Default | Members |
| `EditorSettings` (default editors) | be-type-settings | Viewer | Default | Members |
| `OfflineSettings` | be-block | Viewer | Default | Members |
| `OrganizationPolicies` | be-block | Viewer | Policy | Members |
| `PrefixRegistry`, `Tags` | be-block (`workspace_settings.rs`) | Block | Default | Inside |
| `Home` | be-block (`workspace_settings.rs`) | Block | Default | Members |
| `Hotbar` | be-type-logic-grid | Block | Default | Members |

Default editors have no block layer; a block keeps its own preferred editor in
`PREFERRED_EDITOR` metadata [decisions/default_editors_have_no_workspace_layer.md].

## Resolution

`be_block::Resolution::resolve` takes `ResolutionLayers` and stacks them, the
later winning: organization defaults, the nearest block's kind (a `Block` kind
only), the user's every-device layer, the user's this-device layer, and then
the locks: the block layer for a `Policy` kind, and the organization's policy
for every kind. `value()` folds the layers; `source_of` says which layer set a
field (`SettingsSource`), `locked_by` which lock fixes it, and `below_own` what
the user's layers sit on.

In an editor (`be-sdk-beui/src/editor/settings.rs`):

- `Editor::kinds_above(block)` walks up from the block and takes, per kind,
  the first edit kind or settings-block kind it meets (nearest wins; kinds are
  not merged across ancestors). It stops at the first ancestor the viewer can't
  view and fills the rest from `BlockQuery::OpenSettings`. It is a memo,
  cached per block and pushed when anything on the walk changes.
- `Editor::organization_layers(kind)` reads the organization settings block.
- `Editor::resolved_settings::<R>(block)` combines the three, cached per block
  and kind; `effective_settings` is the open block's value.
  `be_sdk_beui::LayeredSettings` is the per-setting read/write handle the
  settings pages use.
- `ensure_settings_block`, `ensure_block_kind`, `change_block_kind` and
  `clear_block_kind` make and change a block's settings.

## Access

A block's access is the highest of its own grant and every ancestor's
(`level_below` in `be-block-graph/src/lib.rs`), so a lower grant never takes
access away; only Manage shares, and authors and organization administrators
get Manage [decisions/block_access_is_the_highest_grant_on_the_path_up.md].

Settings blocks take their access from their parent
[decisions/settings_blocks_take_their_access_from_their_parent.md]:

- A `Settings` block under a block (`BlockNode::is_block_settings`) has Manage
  when its parent does, at most View when its parent is viewable, and at most
  KnowExists otherwise; its own grants never raise that. Making one, or moving
  one under a block, needs Manage on that block, and dropping a block's
  `SETTINGS` reference needs Manage (`require_keeping_settings` in
  `be-server/src/blocks.rs`).
- An edit kind (`BlockNode::is_edit_kind`) is capped at its parent's access,
  so anyone who edits the block edits its prefixes and tags
  [decisions/prefixes_and_tags_are_edit_kinds_held_by_the_block.md].
- Below a settings block or an edit kind, a block has exactly its parent's
  access: no author bonus, no grant raises it.
- No editor lists these blocks among their owner's children: `BlockInfo::references`
  and `BlockQuery::References` hold only what the content refers to.

## Visibility inside: ANYONE_INSIDE and OpenSettings

A kind granted View to the pseudo-account `be_block_graph::ANYONE_INSIDE`
(`BlockNode::is_open_inside`) is readable by anyone who views any block inside
its owner (`BlockGraph::views_inside`; also `visible`, `viewers` and its key's
`sealed_to`). Only a kind of a block's settings takes that grant, and only View.
The client grants it when it makes a kind whose `VISIBILITY` is `Inside`, and
the settings editor switches it; only a manager changes it, except that the
editor who made an edit kind opens it once as it is made (`opened_kinds` on the
server). So a block shared alone still shows its tags and resolves its row ids
[decisions/prefixes_and_tags_are_visible_to_anyone_inside.md].

`Listing::OpenSettings(block)` (`BlockQuery::OpenSettings` to apps) returns, per
kind, the nearest open kind above `block` in ancestors the viewer can't view
(`list_open_settings` in `be-server/src/blocks.rs`); the server pushes it again
when an open kind changes. Someone who stops viewing every block inside the
owner, or a kind that closes, queues a rotation of its key for whoever read it
inside.

## The prefix registry and tags

`PrefixRegistry` maps database to prefix, read as prefix to databases
(`databases_with`, `prefixes`), since a prefix is a label two databases may
share. It is the only way a row id resolves; guides/tracker.md says how `be`
and the database editor read and write it
[decisions/database_prefixes_resolve_only_through_the_prefix_registry.md].

`Tags` lists `Tag` blocks (name, `TagColor`); `TAGGED` metadata on a block
points to its tags, and a tag's blocks are its backrefs. Personal tags are the
same kind in the user's settings. The helpers are in
`be-sdk-beui/src/editor/tags.rs`.

## The organization and the server

The server has an account of its own (`ServerKey`, `be-server/src/organization.rs`);
`Peer::share_with_server` gives it View on a block and seals the key to it. A
setting the server enforces is shared with it, and it refuses what a policy
governs while it can't read the policy (`ErrorCode::PolicyUnreadable`)
[decisions/server_enforced_settings_are_shared_with_the_server.md]. It gives
every member View on the organization settings block and seals its key to them
itself (`ServerKey::seal_settings_to`); clients accept that key for this block
alone and only to read
[decisions/a_server_sealed_key_never_seals_anything.md].

`OrganizationPolicies` (outside sharing, who makes workspaces, history days,
storage quota) is enforced by the server (`ServerStore::policies`).
`OfflineSettings::choice` resolves whether a workspace is kept offline and what
an organization's devices do on losing access; a personal account never
deletes anything [decisions/personal_account_data_is_never_deleted_automatically.md].

## The UI

be-app-settings holds the settings pages: `root` (the user's settings, their
workspaces, the organization's policies), `root/block.rs` (a block's settings:
what is set here, who may read each kind, what comes from above), and
`prefixes.rs`, `tags.rs`, `home.rs`, `display`, `input`, `default_editors`.
"Settings" in the file tree's and the shell's block menus opens or makes a
block's settings.

## Tests

- Resolution: be-block `settings_resolve_from_the_organization_the_nearest_block_and_the_viewer`.
- Access in the graph: be-block-graph `a_lower_grant_never_reduces_inherited_access`,
  `a_settings_block_needs_manage_on_its_parent_to_change`,
  `an_author_keeps_no_more_of_a_settings_kind_than_its_settings`,
  `a_kind_open_inside_is_read_by_anyone_who_views_a_block_inside`. The graph's
  tests also check access against random graphs (`be-block-graph/src/tests.rs`).
- The server: be-server `a_settings_block_changes_only_with_manage_on_its_parent`,
  `an_editor_changes_the_prefixes_and_tags_but_no_other_kind`,
  `only_a_manager_lets_go_of_a_blocks_settings`,
  `a_settings_kind_opens_to_anyone_who_views_a_block_inside`,
  `losing_every_block_inside_rotates_a_kind_open_inside`.
- The organization: be-block-client
  `an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server`.
- Row ids: be-tool-cli `a_row_id_resolves_through_the_workspaces_prefixes`.
- The UI: paintings in `crates/be-app-settings/src/tests/`, such as
  `a_folders_settings_say_what_is_set_here_and_what_comes_from_above`.
