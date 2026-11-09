# One command palette for the whole host: host actions and quick open

Depends on: none.

be-host draws one command palette (`be-host/src/compositor/palette.rs`) from
the actions every instance describes over the app protocol
(guides/apps/editor.md, Actions and the palette). What is left is the host's
own actions, the workspace's, and quick open. Ancestor shortcuts (a containing
app's chord while a child has the focus) are deferred:
plans/deferred/command_palette_shortcuts.md.

## Host actions

Add the host's own actions to the palette, after every instance's: switch
account (`UiCommand::SwitchAccount`, `be-host/src/ui/keys.rs`), lock screen
(`UiCommand::LockScreen`) where the host has one, and a theme action that
cycles system, light and dark in the host's preference layer (no theme toggle
exists today). Settings is a be-app-workspace action opening its Settings tab,
not a host action.

## Workspace actions and quick open

be-app-workspace registers ordinary actions in its own document, which reach
the palette through the same protocol:

- **Quick open** on Ctrl+P (Cmd+P on macOS) calls `open-palette(instance,
  blocks)`; the host draws block rows from its graph (what it has loaded, and
  recents) and opens the chosen one, until plans/search.md fills the mode with
  results. Ctrl+P works while the workspace has the focus; tests and
  paintings don't assume it works from inside a child.
- Settings: an action opening the workspace's Settings tab.
- Next and previous tab: today Ctrl+Tab is a hard-coded key handler in
  `beui-dock-host/src/unstyled.rs` (an `on_shortcut` in tree capture); move it
  to an action in beui-dock-host that keeps `.capture()`, so it still works
  from a text input in a pane.
- Close the tab, split right and down, show or hide the files pane, new file.

Desktop mode: be-app-desktop's window keys are ordinary actions; its frame
palette test (`the_window_keys_are_listed_in_the_command_palette`) becomes a
host test that the host palette lists them.

## Tests

- A host test that the palette lists the workspace's actions and the host's,
  and that the theme action cycles.
- Paintings: quick open.
