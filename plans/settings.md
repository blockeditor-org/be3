# Settings

A plan for three tiers of settings: per user (which exists), per block with a
workspace as the usual place, and per organization. It starts with the access
model, because who may change a setting is ordinary block access. Each step
says what "done" looks like. Items marked **(ask)** need a decision from the
user during the work.

## Where it stands

- User settings are a root `Settings` block the user authored, found by
  filtering `Query::Roots` by content type and author
  (`block-app/src/root_settings.rs`, `block-editor-plugin/src/root_settings.rs`).
  Its `kinds` map points to one child block per kind: display, input, default
  editors (`EditorSettings`) and the logic game hotbar (`HotbarContent`). A
  profile layers "this device" over "every device" (`Layered`).
- A `Workspace` block holds only `entries` (`be-block/src/workspace.rs`). There
  is no organization block; organizations, memberships and roles
  (`Administrator`, `Editor`) are server tables (`be-server/src/schema.rs`).
- Access is `None < KnowExists < View < Edit < Manage`
  (`be-graph/src/access.rs`). A block's access is the higher of its own grant
  and its parent's (`level_below`, `be-graph/src/lib.rs`), so a grant lower
  down can never take access away. Only Manage shares. Organization
  administrators get Manage on every block (`be-server/src/blocks.rs`).
- Since #603 no client lists every block, so nothing can enumerate every
  database prefix. `Tracker::find` searches breadth-first from the roots and
  what is shared (`be-cli/src/tracker.rs`). Since #600 a prefix is a label and
  need not be unique.
- Metadata can point to a block: a value holding a `BlockRef` is one of the
  block's references (`BlockMetadata::references`), and `ARTIFACT` points to
  its source block (`be_block::Artifact { source, source_type, settings }`).
- "Shared with me" is only a row in the file tree and the picker, filled from
  `ListShared` (`editors/file-tree/src/app/rows.rs`, `block-shell/src/picker.rs`).
  It is not a workspace and has no settings.

## Principles

- **Settings about the data follow the block; settings about the viewer follow
  the viewer.** Tag definitions, prefixes and the owning organization's data
  policies resolve from where the block lives. Theme, trust and the viewer's
  own offline choice resolve from the viewer, whatever they are looking at.
  When an owner's and a viewer's organization policies conflict, the stricter
  wins.
- **Access is the highest grant on the path up.** A lower grant never
  reduces what is inherited, so nobody can be locked out of what they manage,
  and nobody can keep personal files in someone else's workspace out of its
  owner's reach.
- **A setting the server enforces is shared with the server**, and the server
  refuses the action when it can't read the policy. Storing a setting
  unencrypted is only for data that doesn't matter.
- **Never delete a personal account's data automatically**, unsent edits
  included.

## 1. Breaking changes, landed together before one server wipe

Saved data may change incompatibly here, as a one-time exception to
`guides/frozen_formats.md`: there is one server with data, and its content is
kept as a `.block` file. Block files don't store permissions, so a re-import
takes the default access. Land every step of this section, wipe the server,
re-import once, and say so in `guides/frozen_formats.md`.

1. **Metadata that points to a block.** (Done.) A metadata value type holding a block
   id, recorded in the graph's `references` so backrefs and `KnowExists` see
   it. `ARTIFACT` points to a block instead of holding its source inline.
   Done: an artifact block is a backref of the block it describes, and
   `be-client/src/transfer.rs` exports and imports it.
2. **The access model.** Done (server and model). `Manage` is above `Edit`
   in `be_graph::Access` and `AccessLevel`, stored as `manage`. Manage shares
   (`set_access`, `list_access`, pending grants on contact links) and changes
   settings; Edit no longer shares. Authors and organization administrators get
   Manage. Decided instead of the nearest grant: a block's access is the
   highest of its own grant and every ancestor's, so a lower grant never
   reduces View, Edit or Manage (`level_below`). Removing someone's View
   queues a rotation, as before. Key work (sealing, rotation) still needs only
   Edit, since it carries out access already decided. Tests: be-graph
   `a_lower_grant_never_reduces_inherited_access`, be-server
   `only_a_manager_shares_and_a_lower_grant_reduces_nothing`,
   `an_editor_sees_only_the_blocks_it_made_or_was_given`.
3. **The settings block.** Done (server and model), with the built-in rule: a
   block of type `be_graph::SETTINGS_TYPE` (`Settings::CONTENT_TYPE`) whose
   parent is a block has Manage when its parent does, at most View when its
   parent is viewable, and at most KnowExists otherwise; its own grants never
   raise that (`BlockNode::is_block_settings`, `level_below`). Creating one, or
   moving one under a block, needs Manage on that block. The file tree skips a
   settings child of the block it lists. Tests: be-graph
   `a_settings_block_needs_manage_on_its_parent_to_change`, be-server
   `a_settings_block_changes_only_with_manage_on_its_parent`. Left for later
   steps: resolving and editing these settings in the client.

## 2. Access UI

4. **Share dialog.** (Done.) Each person's effective access and where it comes from
   ("Can edit, from *Team*", "Can view, set here"). In an organization, a line
   saying the organization's administrators have access, which can't be
   changed. When sharing a folder, a warning if items inside it have their own
   access ("3 items inside have different access"). The source is the block
   whose grant (or authorship) first reached the effective level going down
   (`BlockGraph::inherited_from`); `AccessEntry::source`, `AccessList { company,
   set_inside }` (`BlockGraph::set_inside` counts blocks below with grants of
   their own).
5. **File tree icons.** (Done.) A clickable share icon on a block whose access is set
   on it, opening the share dialog; a clickable settings icon on a block that
   has settings, opening them. Icons come from `beui_core::icons`. Done: a
   painting of a tree showing both. `BlockSummary` (and `BlockInfo`) carry
   `sets_access` (`BlockNode::sets_access`: a grant to an account, not on a
   settings block). The settings icon shows when the block's `SETTINGS`
   metadata points to its settings, and opens them as the menu's Settings does.

## 3. Resolving settings

6. **Resolution.** In order: organization policy, organization defaults, the
   nearest block with settings, the user's every-device layer, the user's
   this-device layer. Each kind declares whether it is a default (later layers
   win) or a policy (it limits what later layers may choose), and the access
   needed to change it: Edit for the prefix registry and tags, Manage for the
   rest. Resolution walks ancestors and stops at the first one the viewer
   can't view; cache the result per block, pushed when an ancestor's settings
   change, never polled. Done (client): a block's settings block is pointed to
   by its `be_block::SETTINGS` metadata (a `BlockRef`, so it holds the
   settings block) and lists one block per kind in `Settings::kinds`, like the
   user's root settings. Each `Layered` kind declares `NAME`, `FOLLOWS`
   (`Block` or `Viewer`: viewer kinds skip the block layer), `RULE` (`Default`
   or `Policy`: a policy's set fields are fixed for later layers) and
   `VISIBILITY`. `be_block::Resolution::resolve` combines the layers and says
   which layer set a field (`source_of`, `locked_by`); `Editor::kinds_above`
   walks and caches the nearest kind blocks per block, `Editor::resolved_settings`
   caches the resolution per block and kind, and `Editor::organization_layers`
   is the slot steps 14 and 15 fill. Every kind is Manage for now: the server
   checks a kind only through its settings block, whose `kinds` pointers it
   can't read, so an Edit kind (prefix registry, tags) needs its own rule in
   `level_below` and `require_parent_of` once its type exists. Dropping the
   reference to a block's settings block (its `SETTINGS` metadata) needs Manage
   (`require_keeping_settings`). Tests: be-block
   `settings_resolve_from_the_organization_the_nearest_block_and_the_viewer`,
   be-server `only_a_manager_lets_go_of_a_blocks_settings`.
7. **Visibility per kind.** "Anyone who can view a block inside" or "members
   only", chosen by the block's managers. Defaults: the prefix registry and tag
   definitions are visible to anyone inside, so a block shared alone still
   shows its tags and resolves `BE-ABC`; everything else is members only. This
   reveals tag names and prefixes to everyone a single block is shared with,
   which is why it is a setting. Done in the graph and server: a kind block
   granted View to the pseudo-account `be_graph::ANYONE_INSIDE` is viewable by
   anyone who views a block inside its settings' owner (`BlockGraph::views_inside`,
   `visible`, `viewers`, and `sealed_to` for its key), and only a kind under a
   block's settings takes that grant. The client grants it when it makes a kind
   whose `VISIBILITY` is `Inside`, and the settings editor switches it. Where
   the walk meets an ancestor the viewer can't view, it takes the nearest open
   kind of each type above from `Listing::OpenSettings` (`BlockQuery::OpenSettings`),
   which the server pushes again whenever an open kind changes.
   Tests: be-graph `a_kind_open_inside_is_read_by_anyone_who_views_a_block_inside`,
   be-server `a_settings_kind_opens_to_anyone_who_views_a_block_inside`.
8. **Settings editor.** A section for the current workspace and one for any
   block that has settings, opened from the block's menu so that putting
   settings on a folder is rare and deliberate. Where a value comes from
   another layer, say so ("set in *Folder X*", "set by your organization").
   Done: the user's settings list their workspaces, each opening (or, for a
   manager, making) its settings; a settings block under a block shows what is
   set there, who may read each kind, and what comes from above ("Set in
   *Team*"); "Settings" in the file tree's and the shell's block menus opens or
   makes it. A setting row fixed by another layer says where ("Set by your
   organization", "Set in *Folder X*"). Painting:
   `a_folders_settings_say_what_is_set_here_and_what_comes_from_above`.

## 4. Workspace settings

9. **Prefix registry.** A map from prefix to a list of databases, since a
   prefix stays a label. It is the only way a prefix resolves: there is no
   fallback search. Creating a database, changing its prefix, or moving it
   into another workspace updates the registries. An ambiguous prefix says so
   instead of picking one. `be` and the app both read it; naming a database by
   its name (`be Issues row ...`) is unchanged. Update `guides/tracker.md`.
10. **Tags.** Definitions (name, color) in the workspace's settings. Applying a
    tag is metadata on the block that points to the tag (step 1), so it needs
    Edit on the block. "Every block with tag X" is a `Query::Backrefs` on the
    tag. Personal tags are the same type in the user's settings.
11. **Hotbar.** A workspace or game world can set a default hotbar, and the
    user's own hotbar overrides it. Done: `Hotbar` follows the block, a
    manager of a game world gives it a hotbar from their own ("Give this world
    a hotbar"), and a grid starts from the nearest one until its player
    changes theirs; resetting the hotbar returns to it. Painting:
    `a_manager_gives_the_world_a_hotbar_that_players_start_from`.
12. **Default editors (ask).** Whether these have a workspace layer at all.
    Editors for one block type are views over the same data, so which one opens
    a block is a viewer preference, and a workspace layer adds a second place
    to look. Where an author's intent matters ("this database is a board"), it
    belongs to that block, the way `plans/canvas.md` keeps a canvas's last
    profile on the document, not to the workspace. Proposal: keep default
    editors user-only, and let a block store its own preferred editor.
13. **Home.** A landing page and pinned blocks for the workspace, and
    templates for new blocks.

## 5. Organization settings

14. **Sharing a block with the server.** The server becomes an account a block
    can be shared with, with keys like any reader. An action the server can't
    check against an unreadable policy is refused.
15. **The organization settings block.** One per custom organization, created
    with it, shared with the server, editable only by administrators. The
    default organization has none. It holds policies and defaults.
16. **Offline-first.** Add the setting now, and say in its description that
    offline-first doesn't exist yet. The organization's policy is required,
    allowed or blocked. Each user chooses per workspace in their this-device
    layer; blocks shared with them resolve from their own settings, since
    "Shared with me" is not a workspace. When offline-first is built: on
    losing access, unsent edits move to a local "unsent" area, stashed the way
    `Peer::stash` keeps the losing side of a merge, and are offered back if
    access returns (the unsent area exists in be-client: `Peer::unsent`,
    `restore_unsent`). Personal accounts never delete anything automatically.
    For organization accounts, policy decides what happens to cached content,
    with a grace period for unsent edits. Enforcement on lost machines is for
    MDM, out of scope.
17. **Policies the server enforces.** Sharing outside the organization, who can
    create workspaces, history retention (`be-commit/src/retention.rs` is
    hardcoded today) and storage quotas.
18. **Showing members what the organization can see.** A note on the account
    screen that the organization's administrators can read their settings and
    blocks.

## Later

- **Plugins.** Once plugins can be installed: a workspace recommends plugins
  without trusting them, so joining a workspace never runs a stranger's code.
  What runs is (plugins the user trusts and plugins the organization trusts)
  limited to the organization's allowlist.
- **Custom icons** as metadata pointing to an image block (step 1). An icon
  from a workspace's icon set is a reference, not a child, so it needs the
  "anyone inside" visibility of step 7.
- Database conventions shared across databases (status workflows, fields,
  saved views), instructions for agents working in a workspace, calendar and
  locale, default access for new blocks, a spell-check dictionary, and links to
  outside services.
