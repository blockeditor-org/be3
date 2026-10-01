# One command palette for the whole app

Today every editor frame has a command palette of its own (Ctrl+Shift+P in
`block-editor-beui`'s `BeuiFrame`). It lists the actions that are live in that
plugin's document: the frame's own (undo, redo, rename, share, file details,
close) and whatever the editor registered. It cannot see past the plugin it is
in. A text block shown as a direct editor inside a canvas has its palette, and
the canvas has another, and neither one knows about the workspace. This plan
replaces them with one palette. It lists the actions of the focused editor, of
every plugin that contains it, and of the app. Not implemented yet. Items
marked **(ask)** need a decision from the user during the work.

## What exists

- **Actions in beui.** `beui::reactive::Action` (`beui-view/src/actions.rs`)
  has an id, label, glyph, shortcuts, an enabled prop, an optional checked
  prop and an `in_menu` flag. `action_scope` ties a set of actions to a node.
  `active_actions_from(focus_path)` lists the ones live where the focus is,
  innermost first, deduplicated by id. `styled::CommandPalette` takes its rows
  from that list when it opens.
- **One focused instance in the app.** block-app keeps a single
  `host.focus: Option<Target>` (`block-app/src/host.rs`). Key, text and IME
  events go only to that instance (`plugin_host/input.rs`). The app also knows
  every child instance's parent, which is how `report_child_bars`
  (`plugin_host/runtime.rs`) sends a child's `BarAction` to the plugin that
  placed it.
- **A plugin-to-host message with an echo to the parent.** `BarAction` goes
  guest `EditorHost::bar_action` → `editor_session.rs` → `instances.rs` →
  `editors/plugin.rs`. From there it reaches either the parent plugin as
  `EditorMessage::ChildBar`, or the app. A new message follows the same path
  and touches the same files: `block-plugin-api` (`lib.rs` variant,
  `instance()`, `direction()`, and `block_ids.rs`), `block-editor-plugin`
  (`host.rs`, `editor_session.rs`, `screens.rs`), `block-app` (`instances.rs`,
  `runtime.rs`, `editors/plugin.rs`, `editors.rs`) and the `block-ui-test`
  harness.
- **App-level keys.** block-app takes F6/Shift+F6 and Escape before plugins
  (`host::consume_key`), so it can take Ctrl+Shift+P the same way.

## The design

The app owns the palette, because it is the only one that knows the focused
instance and its ancestors. Plugins only describe their actions and run one
when asked. The protocol carries plain data, so it stays independent of the
framework: a plugin written without beui can publish actions the same way.

### Protocol

- `EditorMessage::Actions { instance, actions: Vec<ActionInfo> }`, plugin to
  host. `ActionInfo` holds `id`, `label`, `glyph`, `shortcuts: Vec<Chord>` (a
  protocol chord: key, ctrl, shift, alt), `enabled`, `checked: Option<bool>`
  and `in_menu`.
  - The plugin sends it whenever the list or any field in it changes. It is a
    snapshot, not a diff; a plugin rarely has more than about 50 actions.
  - The list is what `active_actions()` returns, so it follows the plugin's
    own focus. That needs a reactive focus signal on `Document`
    (`watch_focused()`), so the mirror effect re-runs when the focus moves
    rather than polling for it.
- `EditorMessage::RunAction { instance, id }`, host to plugin. The guest looks
  the id up among its live actions and runs it if it is still enabled. It does
  nothing when the action is gone or disabled, because the list the palette
  showed may be a frame old.
- A capability flag in the frame spec, like `panes_offered`, says the host
  draws the palette. When it is set, `BeuiFrame` does not register its own
  Ctrl+Shift+P, so a frame still has a palette in plugin tests and in hosts
  that do not draw one.

### Guest (`block-editor-beui`)

- An effect over a memo of the live actions sends `Actions`. The memo builds
  one `ActionInfo` per action by reading its label, enabled and checked props,
  so it re-runs when any of them changes. Only the values cross the protocol,
  never the closures.
- `RunAction` resolves by id through `active_actions()` and calls `run()`.
- The frame registers nothing new.

### Host (block-app)

- Keep the last `Actions` list per instance.
- On Ctrl+Shift+P, walk from the focused instance up through its parents to
  the root and gather each instance's list:
  - innermost first;
  - deduplicated by id within one instance only, since two instances' "Undo"
    are two different actions;
  - grouped under the instance's block name, so "Undo" for the text and
    "Undo" for the canvas can both be shown (**ask**: show both, or only the
    innermost one of each id).
- Add the app's own actions: switch account, settings, theme, and the dialogs
  the app owns.
- Draw the palette with `styled::CommandPalette`. Its rows come from an
  explicit list instead of the document's registry: add an
  `actions: Option<Prop<Vec<Action>>>` prop. Build the remote rows as proxy
  `Action`s (an `ActionBuilder::build()` that does not register), each of
  whose `run` sends `RunAction` to its instance.
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

### Shortcuts across plugins

Keys still go only to the focused instance, which runs its own actions. An
ancestor's shortcut (say the workspace's Ctrl+P while a canvas has the focus)
needs the app to try the ancestors' chords when the focused instance leaves a
key unhandled. Two ways to do that:

1. Plugins report whether they handled each key, and the app retries the
   chord on each ancestor in turn. This needs a round trip per key.
2. The app matches the chord against the ancestors' published `ActionInfo`
   shortcuts and sends `RunAction` directly. This needs no round trip, but it
   only works for keys the focused plugin does not want, which the app cannot
   know without option 1.

Recommend 1, limited to keys with Ctrl or Alt (**ask**).

## Tests

- `block-plugin-api`: `Actions` and `RunAction` round-trip, as
  `bar_actions_round_trip` does.
- `block-ui-test`: a `BeuiTest` reports a plugin's published actions, follows
  a change in an action's enabled state, and runs an action from a
  `RunAction`.
- block-app: with a text child focused inside a canvas inside the workspace,
  the palette lists all three instances' actions, innermost first, and running
  a canvas action from it reaches the canvas.

## Suggested order

1. Protocol messages and the guest mirror, behind the capability flag.
2. The host palette fed from the focused instance only.
3. Walk up the ancestors.
4. App and workspace actions.
5. Ancestor shortcuts.
