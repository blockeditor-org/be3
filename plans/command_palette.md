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
- **One focused instance in the host.** block-app keeps a single
  `host.focus: Option<Target>` (`be-host/src/host.rs`). Key, text and IME
  events go only to that instance (`app_host/input.rs`). The host also knows
  every child instance's parent, which is how `report_child_bars`
  (`app_host/runtime.rs`) sends a child's `BarAction` to the app that
  placed it.
- **An app-to-host message with an echo to the parent.** `BarAction` goes
  guest `EditorHost::bar_action` → `editor_session.rs` → `instances.rs` →
  `editors/app.rs`. From there it reaches either the parent app as
  `EditorMessage::ChildBar`, or the host. A new message follows the same path
  and touches the same files: `be-sdk-api` (`lib.rs` variant,
  `instance()`, `direction()`, and `block_ids.rs`), `be-sdk`
  (`host.rs`, `editor_session.rs`, `screens.rs`), `block-app` (`instances.rs`,
  `runtime.rs`, `editors/app.rs`, `editors.rs`) and the `be-sdk-test`
  harness.
- **Host-level keys.** block-app takes F6/Shift+F6 and Escape before apps
  (`host::consume_key`), so it can take Ctrl+Shift+P the same way.

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
    own focus. That needs a reactive focus signal on `Document`
    (`watch_focused()`), so the mirror effect re-runs when the focus moves
    rather than polling for it.
- `EditorMessage::RunAction { instance, id }`, host to app. The guest looks
  the id up among its live actions and runs it if it is still enabled. It does
  nothing when the action is gone or disabled, because the list the palette
  showed may be a frame old.
- A capability flag in the frame spec, like `panes_offered`, says the host
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
  `bar_actions_round_trip` does.
- `be-sdk-test`: a `BeuiTest` reports an app's published actions, follows
  a change in an action's enabled state, and runs an action from a
  `RunAction`.
- block-app: with a text child focused inside a canvas inside the workspace,
  the palette lists all three instances' actions, innermost first, and running
  a canvas action from it reaches the canvas.

## Suggested order

1. Protocol messages and the guest mirror, behind the capability flag.
2. The host palette fed from the focused instance only.
3. Walk up the ancestors.
4. Host and workspace actions.
5. Ancestor shortcuts.
