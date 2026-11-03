# One command palette for the whole host

Today every editor frame has a command palette of its own (Ctrl+Shift+P in
`be-sdk-beui`'s `BeuiFrame`). It lists the actions that are live in that
app's document: the frame's own (undo, redo, rename, share, file details,
close) and whatever the editor registered. It cannot see past the app it is
in. A text block shown as a direct editor inside a canvas has its palette, and
the canvas has another, and neither one knows about the workspace. This plan
replaces them with one palette. It lists the actions of the focused editor, of
every app that contains it, and of the host. Not implemented yet. Items
marked **(ask)** need a decision from the user during the work.

## What exists

- **Actions in beui.** `beui::reactive::Action` (`beui-view/src/actions.rs`)
  has an id, label, glyph, shortcuts, an enabled prop, an optional checked
  prop and an `in_menu` flag. `action_scope` ties a set of actions to a node.
  `active_actions_from(focus_path)` lists the ones live where the focus is,
  innermost first, deduplicated by id. `styled::CommandPalette` takes its rows
  from that list when it opens, unless it is handed `PaletteItems::list(..)`.
- **The frame menu already reaches the host.** An editor sends its frame
  menu's actions to the host as a tree of `MenuEntry` values (label, glyph,
  enabled, first shortcut, check) whenever an entry changes, and runs the
  command whose id the host sends back (`MenuPick`); guides/adding_an_app.md
  describes it. Actions not in the menu do not cross.
- **Keyboard focus in the host.** be-host-runtime tracks which instance has
  the keyboard (`crates/be-host-runtime/src/input.rs`, `instances.rs`); key,
  text and IME events go only to it. The host knows every child instance's
  parent, which is how `report_child_bars` (`be-host-runtime/src/runtime.rs`)
  sends a child's `BarAction` to the app that placed it.
- **An app-to-host message with an echo to the parent.** `BarAction` goes
  guest `EditorHost::bar_action` (`be-sdk/src/host.rs`) →
  `be-sdk/src/editor_session.rs` → `be-host-runtime` (`instances.rs`,
  `runtime.rs`, as `ChildBar` to the parent app) → `be-host/src/compositor/editors.rs`.
  A new message follows the same path, plus `be-sdk-api` and the
  `be-sdk-test` harness.
- **Intercepted keys.** An instance's intercepting actions name chords the
  host offers it ahead of whatever is focused (`Input::InterceptedKey`,
  guides/beui_keyboard.md), so the host already routes chosen chords to an
  instance that does not have the focus.
- **A reactive focus signal.** `Document::watch_focused()` exists, so the
  guest mirror can re-run when the focus moves rather than polling.

## The design

The host owns the palette, because it is the only one that knows the focused
instance and its ancestors. Apps only describe their actions and run one
when asked. The protocol carries plain data, so it stays independent of the
framework: an app written without beui can publish actions the same way.

### Protocol

- `EditorMessage::Actions { instance, actions: Vec<ActionInfo> }`, app to
  host. `ActionInfo` holds `id`, `label`, `glyph`, `shortcuts: Vec<Chord>` (a
  protocol chord: key, ctrl, shift, alt), `enabled`, `checked: Option<bool>`
  and `in_menu`.
  - The app sends it whenever the list or any field in it changes. It is a
    snapshot, not a diff; an app rarely has more than about 50 actions.
  - The list is what `active_actions()` returns, so it follows the app's
    own focus (`watch_focused()`).
- `EditorMessage::RunAction { instance, id }`, host to app. The guest looks
  the id up among its live actions and runs it if it is still enabled. It does
  nothing when the action is gone or disabled, because the list the palette
  showed may be a frame old.
- A capability flag in the frame spec says the host
  draws the palette. When it is set, `BeuiFrame` does not register its own
  Ctrl+Shift+P, so a frame still has a palette in app tests and in hosts
  that do not draw one.

### Guest (`be-sdk-beui`)

- An effect over a memo of the live actions sends `Actions`. The memo builds
  one `ActionInfo` per action by reading its label, enabled and checked props,
  so it re-runs when any of them changes. Only the values cross the protocol,
  never the closures.
- `RunAction` resolves by id through `active_actions()` and calls `run()`.
- The frame registers nothing new.

### Host (be-host)

- Keep the last `Actions` list per instance.
- On Ctrl+Shift+P, walk from the focused instance up through its parents to
  the root and gather each instance's list:
  - innermost first;
  - deduplicated by id within one instance only, since two instances' "Undo"
    are two different actions;
  - grouped under the instance's block name, so "Undo" for the text and
    "Undo" for the canvas can both be shown (**ask**: show both, or only the
    innermost one of each id).
- Add the host's own actions: switch account, settings, theme, and the dialogs
  the host owns.
- Draw the palette with `styled::CommandPalette`. Its rows come from an
  explicit list instead of the document's registry: pass
  `items={PaletteItems::list(..)}`, one `PaletteItem` per `ActionInfo`
  (key, title, glyph, shortcut, checked, disabled), with the instance's
  block name as the `detail` line. `on_choose` gets the chosen item's key;
  make each key name its instance and action, and send `RunAction` from
  there.
- After a choice, give the focus back to the instance that had it, then send
  `RunAction`, the same order the frame palette uses today.

### Workspace actions

workspace-ui has no actions today. With the palette it should register them
as ordinary actions in its own document, and they would show up through the
same protocol:

- quick open (find a file by name), with its own shortcut (**ask**: Ctrl+P);
- switch to the next and previous tab (Ctrl+Tab already exists in the dock);
- close the tab, split right and down, show or hide the files pane;
- new file.

### Shortcuts across apps

Keys still go only to the focused instance, which runs its own actions. An
ancestor's shortcut (say the workspace's Ctrl+P while a canvas has the focus)
needs the host to try the ancestors' chords when the focused instance leaves a
key unhandled. Two ways to do that:

1. Apps report whether they handled each key, and the host retries the
   chord on each ancestor in turn. This needs a round trip per key.
2. The host matches the chord against the ancestors' published `ActionInfo`
   shortcuts and sends `RunAction` directly. This needs no round trip, but it
   only works for keys the focused app does not want, which the host cannot
   know without option 1.

Recommend 1, limited to keys with Ctrl or Alt (**ask**).

## Tests

- `be-sdk-api`: `Actions` and `RunAction` round-trip, as
  the other messages do in `be-sdk-api`'s tests.
- `be-sdk-test`: a `BeuiTest` reports an app's published actions, follows
  a change in an action's enabled state, and runs an action from a
  `RunAction`.
- be-host: with a text child focused inside a canvas inside the workspace,
  the palette lists all three instances' actions, innermost first, and running
  a canvas action from it reaches the canvas.

## Suggested order

1. Protocol messages and the guest mirror, behind the capability flag.
2. The host palette fed from the focused instance only.
3. Walk up the ancestors.
4. Host and workspace actions.
5. Ancestor shortcuts.
