# One command palette for the whole host

Depends on: none.

Today every editor frame has a command palette of its own (Ctrl+Shift+P in
`be-sdk-beui`'s `BeuiFrame`). It lists the actions live in that app's document
and cannot see past the app it is in: a text block shown inside a canvas has
its palette, the canvas has another, and neither knows about the workspace.
This plan replaces them with one palette the host draws, listing the actions of
the focused editor, of every app that contains it, and of the host, plus quick
open. Ancestor shortcuts (a containing app's chord while a child has the focus)
are deferred: plans/deferred/command_palette_shortcuts.md.

## What exists

- **Actions in beui.** `beui::reactive::Action` (`beui-view/src/actions.rs`)
  has an id, label, glyph, shortcuts, an enabled prop and an optional checked
  prop. `action_scope` ties actions to a node; `active_actions_from(focus_path)`
  lists the live ones, innermost first, deduplicated by id.
  `styled::CommandPalette` takes its rows from that list unless it is handed
  `PaletteItems::list(..)`. Menus are separate `MenuNode` trees
  (`add_to_menu`, `document_menu`).
- **Actions already cross the app protocol, but only for accessibility.** A
  frame report's `description` (`types.wit`) carries `actions:
  list<described-action>` (`accessibility.wit`: id, label, `shortcut:
  option<string>`, enabled, checked, live). The guest fills it in `describe`
  (`be-sdk-beui-platform/src/lib.rs`); be-host-runtime collects every screen's
  actions into `ActionGroup`s per app pane and calls `context.publish_actions`
  (`be-host-runtime/src/description.rs`); the host runs one with
  `app.run-action(screen, id)`, which the guest resolves through
  `request_action_in` and runs only if still enabled. All of it runs only
  while the host asks `app.describe(true)` (a screen reader or `drive`
  attached). guides/apps/editor.md (Accessibility) documents it.
- **Menus** already carry a richer record: `menu-command` (`types.wit`: id,
  label, glyph, enabled, check, `shortcut: option<key-chord>`, pin), over
  `shell.menu(instance, view, items)` and `editor.menu-pick(view, id, anchor)`.
- **Focus.** beui's forwarding catchers own which app has the keyboard; the
  runtime answers `view_has_keyboard` (`be-host-runtime/src/runtime.rs`). The
  runtime knows each child instance's parent (the placements
  `report_child_bars` walks).
- **Desktop.** In desktop mode `be-app-desktop` is the root, and its window
  keys are listed in its frame palette
  (`be-app-desktop/src/tests/the_window_keys_are_listed_in_the_command_palette.rs`).
  The frame menu has a "Command palette" entry
  (`be-sdk-test/src/tests/the_menu_opens_the_command_palette.rs`).

## The design

The host owns the palette, because only it knows the focused instance and its
ancestors. Apps describe their actions and run one when asked, over the
existing `described-action` and `run-action`: one channel for assistive
technology and the palette, not two.

### Protocol

- **Actions flow without accessibility.** Split `actions` out of
  `description` into its own field of the frame report, sent whenever the list
  or a field in it changes, whatever `describe` says; `description` keeps the
  tree. The list keeps today's shape (`Document::describe_actions`,
  `Registry::describe`): the live actions plus every other registered action
  marked `live: false`. The palette shows only live ones; `drive` and screen
  readers keep all, and `run-action` still resolves non-live ones. "Sent when
  it changes" is a reactive effect over the registry's version, the focus
  (`watch_focused()`) and each action's label, enabled and checked signals,
  read tracked (`info()` reads labels untracked today), not a per-frame diff.
- **`described-action` gains** `glyph: option<string>` (an icon name, as
  decisions/beui_icons_are_saved_by_name.md saves icons) and `shortcut:
  option<key-chord>` in place of the string, reusing `types.wit`'s `key-chord`
  (it has Super). Screen readers read the chord's label as before.
- **Running** stays `app.run-action(screen, id)`, with the screen of the
  instance's frame view.
- **The host draws the palette** when `open-block` carries a new
  `host-palette: bool`. Then `BeuiFrame` registers no Ctrl+Shift+P of its own,
  and its frame menu's "Command palette" entry calls a new host call
  `shell.open-palette(instance, mode)`, with mode `actions` or `blocks`. Without the flag (app tests, hosts that draw
  none) the frame keeps its own palette.

### Host (be-host)

- Keep the last action list per instance (already in description.rs; keep
  it while describe is off).
- The host registers Ctrl+Shift+P as `.at(KeyAt::Window).capture()`, since a
  focused app view takes every key that reaches it (guides/beui_keyboard.md,
  Boundaries).
- On Ctrl+Shift+P (or `open-palette`), walk from the focused instance up
  through its parents to the root and gather each instance's list innermost
  first, grouped under the instance's block name. Two instances' "Undo" are
  both shown, the innermost first, so searching "undo" puts the focused
  editor's at the top. Deduplicate by id within one instance only.
- Add the host's own actions: switch account (`UiCommand::SwitchAccount`,
  `be-host/src/ui/keys.rs`), lock screen (`UiCommand::LockScreen`) where the
  host has one, and a theme action that cycles system, light and dark in the
  host's preference layer (no theme toggle exists today). Settings is a
  be-app-workspace action opening its Settings tab, not a host action.
- Draw it with `styled::CommandPalette` and `PaletteItems::list(..)`: one item
  per action, the block name as `detail`, each key naming its instance and
  action. After a choice, give the focus back to the instance that had it, then
  send `run-action`.
- Considered and ruled out: building detached host `Action`s in an
  `action_scope` per app catcher (as decisions/beui_dock_tab_menus_are_action_lists.md
  does for tab menus) so `active_actions_from` lists them. It would rebuild
  closures for every action of every app on each change and tie the list's
  order to the host's node tree; an explicit list is simpler.

### Workspace actions and quick open

be-app-workspace registers ordinary actions in its own document, which reach
the palette through the same protocol:

- **Quick open** on Ctrl+P (Cmd+P on macOS) calls `open-palette(instance,
  blocks)`; the host draws block rows from its graph (what it has loaded, and
  recents) and opens the chosen one, until plans/search.md fills the mode with
  results. Ctrl+P works while the workspace has the focus; tests and
  paintings don't assume it works from inside a child (ancestor shortcuts are
  deferred, plans/deferred/command_palette_shortcuts.md).
- Settings: an action opening the workspace's Settings tab.
- Next and previous tab: today Ctrl+Tab is a hard-coded key handler in
  `beui-dock-host/src/unstyled.rs` (an `on_shortcut` in tree capture); move it
  to an action in beui-dock-host that keeps `.capture()`, so it still works
  from a text input in a pane.
- Close the tab, split right and down, show or hide the files pane, new file.

Desktop mode: be-app-desktop's window keys become ordinary actions too, listed
by the host palette; its frame palette test becomes a host test that the host
palette lists them.

## Tests

- `be-sdk-api`: the split action list and the extended `described-action`
  round-trip.
- `be-sdk-test`: a `BeuiTest` publishes its actions with describe off, follows
  an action's enabled state, runs one from `run-action`; with `host-palette`
  the menu entry calls `open-palette`.
- A host test (stage only the apps it needs, with `-c be3.apps=...`) with a
  text child focused inside a canvas inside the workspace: the palette lists
  all three instances' actions, innermost first, and running a canvas action
  reaches the canvas.
- Paintings: the host palette with grouped actions, and quick open.

## Order of work

1. The protocol: actions split out of `description` and sent always,
   `described-action` extended, `host-palette` and `open-palette`.
2. The host palette fed from the focused instance only.
3. Walk up the ancestors.
4. Host actions, workspace actions and quick open; desktop's window keys.
