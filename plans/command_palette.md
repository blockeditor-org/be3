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
  tree. The list is the frame's `active_actions()`, so it follows the app's own
  focus (`watch_focused()`), pushed, never polled.
- **`described-action` gains** `glyph: option<string>` (an icon name, as
  decisions/beui_icons_are_saved_by_name.md saves icons) and `shortcut:
  option<key-chord>` in place of the string, reusing `types.wit`'s `key-chord`
  (it has Super). Screen readers read the chord's label as before.
- **Running** stays `app.run-action(screen, id)`, with the screen of the
  instance's frame view.
- **The host draws the palette** when `open-block` carries a new
  `host-palette: bool`. Then `BeuiFrame` registers no Ctrl+Shift+P of its own,
  and its frame menu's "Command palette" entry calls a new host call
  `shell.open-palette(instance)`. Without the flag (app tests, hosts that draw
  none) the frame keeps its own palette.

### Host (be-host)

- Keep the last action list per instance (already in description.rs; keep
  it while describe is off).
- On Ctrl+Shift+P (or `open-palette`), walk from the focused instance up
  through its parents to the root and gather each instance's list innermost
  first, grouped under the instance's block name. Two instances' "Undo" are
  both shown, the innermost first, so searching "undo" puts the focused
  editor's at the top. Deduplicate by id within one instance only.
- Add the host's own actions: switch account (`UiCommand::SwitchAccount`,
  `be-host/src/ui/keys.rs`), settings, the theme toggle, and the lock screen
  where the host has one.
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

- **Quick open** on Ctrl+P (Cmd+P on macOS): the palette in a mode that lists
  blocks by name from the graph the host already has loaded, opening the
  chosen one. plans/search.md later feeds its results into this mode.
- Next and previous tab: today Ctrl+Tab is a hard-coded key handler in
  `beui-dock-host/src/unstyled.rs`; move it to an action.
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
