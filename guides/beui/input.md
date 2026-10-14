# Keyboard and input

How keys, pointers, touch and text reach beui code: actions and shortcuts, the path a key takes
through the document, how embedded apps (plugins, Wayland programs) get their input, control props
and the keyboard behaviour of every styled control. Read it before adding a command, a shortcut,
a key handler or a keyboard-driven control. Reactive props and controlled state (`focused`,
`open`) are in `guides/beui/reactive.md`.

## Add a command: actions

A command a person can run (a button, a menu row, a shortcut, a command palette entry) is written
once, as an `Action`. Everything that offers it is handed the action rather than its own label,
disabled flag and callback:

```rust
let group = Action::new("canvas.group", "Group", move || state.run(Group))
    .glyph(ICON_GROUP_WORK)
    .shortcut(Chord::ctrl(Key::G))
    .enabled(can_group)
    .register();
view! {
    <IconButton action={group.clone()} />
    <MenuItem action={group} />
}
```

- `register` adds it to the nearest `ActionScope` (or the document's own, outermost one) and
  removes it when the registering scope goes.
- `Button`, `IconButton`, `ToggleButton`, `ActionRow` and `MenuItem` take an `action` prop. They
  show its label and glyph unless given their own, are disabled while it is, show its first
  shortcut in their tooltip, and run it when pressed. A `ToggleButton` is pressed while the action
  is `checked`.
- An action's `run` does nothing while it is disabled.
- `in_menu()` puts it in `menu_actions()`, which fills an editor frame's menu (the dock shows it
  behind its More button).

### Where an action listens

An action is a key listener (see "How a key travels").

| Placement | Answers |
|---|---|
| Registered under no scope | Bubble phase at the root. |
| Under `action_scope(&node_ref)` | Bubble phase at that node, so only while the focus is inside it (an editor's, a text area's). A key the focused control does not handle reaches the innermost scope first. |
| `.capture()` | Moves it (at the root or in a scope) to the capture phase, so it answers before the focused control. |
| `.at(KeyAt::Window)` | Above the tree: hears what nothing on the way up took, wherever the focus is, dialogs included; the command palette lists it wherever it opens. It leaves an app's keys alone. |
| `.at(KeyAt::Window).capture()` | Takes a key away from a focused app: offered the press before anything on the tree and before the app, and a press it takes never reaches the app, nor its release. Reserve it for shortcuts that must work over anything, such as linux-desktop's Super+F and media keys. |
| `.at(KeyAt::Host).capture()` | The only kind still offered keys while an app with `inhibits_shortcuts` has the focus (see "Apps"). |

Which scopes are live:

- With the focus on a node no scope holds, every scope is live; with nothing focused, every scope
  in the topmost trapping layer is.
- A chord without Ctrl, Alt or Super (and not a media key) does not reach actions while the target
  types text or is an app, so typing a letter into a field never switches a canvas tool. Inside
  an app only chords with Ctrl, Alt or Super, and media keys, are offered, since the app counts as
  a text field.

### The command palette

`styled::CommandPalette` lists the actions live where the focus was when it opened.

- Typing filters by every word; Up/Down/Page Up/Page Down move the highlight over enabled actions;
  Enter or a click runs the highlighted one; Escape closes it.
- Disabled actions are listed last, greyed.
- Closing it puts the focus back where it was before the action runs.
- An editor frame opens it with Ctrl+Shift+P and from a row of its menu.

## Listen for keys

Prefer an action. When a listener is needed:

| API | What it is |
|---|---|
| `on_key(at, phase, handler)` | A listener at a position `KeyAt` in the capture or bubble phase. |
| `on_shortcut(handler)` | A tree capture listener at the component that registers it (the root outside a component), offered every press before the focused control. That is how the dock's Ctrl+Tab reaches it from inside a text input in a panel, and why it does not while a dialog is open. |
| `on_global_key(handler)` | A window capture listener, handed a `KeyEvent`: the `input` (a `KeyPress` or a modifier `Tap`), the `phase`, and the `target` (`node`, and whether it `typing`s text or is an `app`). |
| `on_scroll_key` | A listener whose keys move the view themselves, so answering one does not scroll the focus into view. |
| A focusable's `on_key` | The target's own handler. Return `true` only for keys it handled. |
| `held_modifiers()` | A signal of the modifiers held now; changes on modifier presses only. |

Shortcuts that are not chords, such as a hold-and-release switcher, are built from
`on_global_key` and `held_modifiers()`.

A listener at the focused node itself is on the path in both phases, which is how a compound
control steers keys before the control inside it handles them: the command palette and select put
a capture listener on their search box (`@node_ref` on the `TextInput`) for the arrows, and a
scroll area puts a bubble listener on itself for the arrows its children leave.

`KeyAt` positions:

- `KeyAt::Host` and `KeyAt::Window` are above the tree, on every path.
- `KeyAt::Root` and `KeyAt::Node(node_ref)` are tree nodes, on the path only while the focus is
  inside them. With nothing focused, every listener in the topmost trapping layer hears the key,
  as if the focus could be anywhere in it.

## How a key travels

Every press, repeat and release takes one trip along the focus path, like a DOM event: from the
host, through the window, down the tree from the root to the focused node (the target), and back
up. With nothing focused, the target is the topmost layer that traps focus, or the root.

The trip, in order; the first listener to answer `true` ends it:

1. Host capture listeners, then window capture listeners.
2. Tree capture listeners from the root down to the target.
3. The target: its `on_key`, then its own activation and arrow stepping.
4. Tree bubble listeners from the target up.
5. Window bubble listeners, then host bubble listeners.

Answering a key scrolls the focus into view (except for `on_scroll_key` listeners).

### Built-in listeners

The document's own keys are listeners too (`builtin` in `interact.rs`). The lock never cuts them
unless noted.

- The first host capture listener ends autoscroll on any press, and takes Escape doing it.
- The last window capture listener is F6 and Shift+F6, which move the focus between forwarding
  catchers (cut with the window).
- The first window bubble listener asks the innermost dismissable layer around the focus to close
  on Escape (overlays are in `guides/beui/components.md`). So a window listener for Escape hears it
  only when no layer took it. A key bound for an app never gets that far, so a window capture
  listener that wants Escape over an app asks `Document::escape_layer` and `dismiss_overlay`
  itself, as block-app's presentation mode does.
- The last host bubble listener is Tab, Escape (which cancels a held activation) and Back.
- Ctrl+V asks the host for a paste once the trip is over, only if nothing took the key (an app
  counts as nothing), or if what took it asked for a paste itself.

### Held keys

A listener that takes a press in the capture phase holds its key: the physical key, the text it
typed, its repeats and its release go to that listener alone, with `phase` `Held`, wherever the
focus has gone since, and nowhere else. A held key skips the trip.

Whatever takes a press later in the trip (the target or a bubble listener) holds its release the
same way, while its repeats take the trip again. So the release of an Escape that closed a menu
never reaches the app the focus returned to. This is the one record of taken keys; nothing else
keeps its own.

### Boundaries

Boundaries cut the path:

- A layer that traps focus (a menu, a dialog, a popover) cuts the tree: with the focus inside it,
  nodes outside it are not on the path, so shortcuts and actions outside a dialog do not hear keys
  typed in it. The host and window still do.
- An app (a forwarding catcher with the focus, such as a plugin's region or a Wayland program) is
  the target and takes every key that reaches it, so nothing bubbles. Only host and window capture
  listeners hear a key bound for an app; tree listeners do not. An app with `inhibits_shortcuts`
  also cuts the window, so only the host hears.
- The lock screen cuts the host and window for every key but media keys.

### Modifier keys and taps

The modifier keys arrive as `Key::Shift`, `Key::Ctrl`, `Key::Alt` and `Key::Logo` presses and
releases (`Key::is_modifier`), alongside the `Event::Modifiers` they cause.

- They reach host and window capture listeners and forwarded regions (plugins, Wayland programs),
  but not the tree, the focused control's `on_key` or actions, which read the state from
  `KeyPress::modifiers` or `held_modifiers()`.
- They do not show the focus ring or end autoscroll, and the runners drop their repeats.

To react to a modifier key itself, such as tapping Super alone, register a global action with
`Chord::tap(Key::Logo)`; do not read events. linux-desktop's program launcher opens this way.

- The document counts a modifier pressed and released with no other key, press, scroll or focus
  loss between as a tap, judged from key events alone: the runners disagree on whether
  `Event::Modifiers` comes before or after the key's own event, and winit on X11 can report a
  modifier state that reverts before the key arrives.
- The tap is offered on the release to host and window capture listeners as a `KeyInput::Tap`. The
  release still goes on to whatever it would have reached.
- An intercepting tap action is named among the document's intercepted keys as a `KeyChord` with
  `tap` set (see "Apps").

`Modifiers` and `Chord` have `logo` for the Super (Windows) key. The runners report Super as
`logo`, except on macOS, where Command acts as Ctrl and Super is never reported. `Key` also has the
volume, mic mute, brightness and media transport keys, mapped wherever the platform reports them.

## Apps: plugins and Wayland programs

### Intercepting keys from a host

A document that runs inside a host (a plugin's) intercepts keys the same way:

1. The chords of its enabled capturing host and window actions (never one that types) are
   `Document::intercepted_keys`. They reach the runner as `WindowState::intercepted_keys`, handed
   to `Platform::show_window`.
2. The host offers those chords to its own window listeners ahead of the focused app.
3. It hands each press it takes, its repeats and its release back as an `Event::InterceptedKey`,
   which the document offers to its host and window capture listeners as a key bound for an app;
   it reaches nothing else.
4. The host keeps sending the held modifiers until the chord's are released, so `held_modifiers()`
   sees Alt released while a Wayland program has the keyboard. That is how linux-desktop's Alt+Tab
   works over any window.

block-app takes intercepted keys only from the shell's frame, and only chords with Ctrl, Alt or
Super, or media keys. A tap chord goes the same way, as the protocol's `InputEvent::InterceptedTap`,
which the document receives as `Event::InterceptedTap`, but only while the region does not have the keyboard itself, since then
it hears the key and counts the tap on its own; the release still reaches whatever had the focus.

### Apps that want every chord

A forwarding catcher with `inhibits_shortcuts` set (an `Interactive` prop) is an app that wants
every chord itself, such as a remote desktop or a virtual machine. While it has the focus:

- the window is cut from the path, so window listeners and taps leave its keys alone, F6 does not
  move the focus, and all of them reach the app;
- only host listeners (`.at(KeyAt::Host).capture()` actions) still answer, and only the host's own
  are ever offered an inhibited key. block-app's intercept handler is a window listener, so it
  hands a plugin nothing while it is set.

The host set is kept small and fixed: block-app's Super+L lock and be-wayland's Super+Escape,
which takes the shortcuts back from the window (see `guides/linux_desktop.md`).

### Wanting the keyboard

A document with a layer open that traps focus (`Layers::takes_keyboard`: a dialog, a popover, the
launcher) reports `WindowState::wants_keyboard`, which the runner hands to `Platform::show_window`.
A plugin passes it on as `WindowRequests::wants_keyboard` (`guides/plugins/editor_api.md`), and
block-app's region then takes the focus from whatever had it (a program's window, another editor)
and gives it back when the overlay closes. So a dialog a plugin opens from an intercepted key can
be typed into.

## One input path

Every embedded editor is a forwarding catcher: a plugin's region, and in `block-app --session` and
`--desktop` each Wayland program's window. A forwarding catcher is an `Interactive` with
`on_forward` (and `hit_at` for the points it takes), handed what beui routes to it as a
`ForwardedInput`:

| Variant | Carries |
|---|---|
| `Forwarded::Pointer` | The pointer events beui resolved for it, with `over`: the catcher under the event whatever holds the press, which a drag between Wayland windows needs. |
| `Forwarded::Left` | The pointer is neither over it nor held by a press in it. |
| `Forwarded::Focus` | It gained or lost the focus. |
| `Forwarded::Input` | Keys, text, IME and file drops meant for it. |

The host turns these into its own protocol (`plugin_host::forward_region` for a plugin,
`be_wayland::Compositor` for a window) and keeps no capture or hover of its own, so press capture,
hover, claims, overlays, focus, intercepting actions and the lock apply to both by the same rules.
The one exception is the browser tab's web view (`crates/editors/browser-tab`), a native view over
the window that takes input from the platform, not from beui.

What one event goes through, in order:

1. The runner. `beui-adapter-drm` drops the input that wakes blanked screens, the rest of it for a
   grace period, and every release of a press it dropped (`wake.rs`, see
   `guides/linux_desktop.md`). This must happen there: it is decided on libinput's clock before
   the keymap sees the key. The keymap's own actions (switching terminals, Ctrl+Alt+Backspace)
   are the adapter's too. A key is one `Event::Key`: the `KeyPress`, the scan code (`code`, for
   Wayland programs, and on its own as `Key::Unidentified` for a key beui has no name for) and the
   text it typed. `Event::Text` is text without a key; `Event::Paste` is a paste.
2. `InputState::begin_frame` turns the frame's events into `PointerEvent`s (moves, presses,
   releases, cancels, wheel turns, zooms), each where the pointer was at that moment. A mouse event
   that lands on a finger is the platform's copy of the touch and is dropped.
3. `interact`. The document counts the frame as input (`Document::input_frames`, which the
   compositor's idle time reads). For each pointer event it hit tests the layers once, asks the
   layers it is outside of to close, finds a modifier claim (`claim_modifiers`, `claim_at`) or a
   capture for a press, hands it to the pointer's session (one per pointer and button, which also
   counts fingers for pinches and two-finger gestures), and moves the focus.
4. Keys, one at a time along the focus path ("How a key travels"): host and window listeners first
   (capturing actions, and in block-app the shell plugin's intercepted chords,
   `compositor/intercept.rs`), then F6 and Shift+F6. A key one of them takes is gone, code and text
   with it, and so are its later repeats and release. A key bound for an app goes no further; any
   other key goes down the tree and back up, and types its text unless something took it.
5. `forward::route`. The pointer goes to the catcher under it (`hit_at` says no where something
   else is drawn over it); a press stays with the catcher its down reached until every button is
   released; a claimed press goes to its claimant; wheel and drop events go to the catcher under
   the pointer; keys, text and IME go to the focused catcher. A catcher beneath a layer that
   blocks everything loses its capture.

So no editor reads the frame's events itself, and nothing gives a key a meaning outside an action,
a shortcut or a control.

A host that shows several documents in one frame (as the inspector does beside the app) gives each
the input it may see through `Context::with_input`:

- `InputState::only` keeps some events.
- `without_pointer` takes the pointer away, telling the document the pointer left in the frame it
  starts.
- A `Focus(false)` before and `Focus(true)` after the time the document's keys go elsewhere, like a
  window losing the keyboard; this also stops it claiming the IME.

The inspector panel is a document of its own: Ctrl+Shift+F moves focus into it and back, and
Escape inside it returns focus to the inspected document.

## Focus and Tab

- Tab and Shift+Tab traverse visible controls in tree order and wrap within the document. Hidden
  panels and collapsed content are excluded.
- `tab_stop` is a prop on `<unstyled::Button>` and a focusable `<Interactive>`. Set it `false` to
  keep a control reachable by pointer and programmatic focus but out of Tab order; that is how
  single-Tab-stop groups work.
- Changing a selection programmatically updates the group's Tab stop and moves focus with the
  selection when the group already contains focus. Programmatic changes do not pull focus from
  other controls.
- Empty groups have no Tab stop, and invalid selection updates are ignored.
- `Document::focused_node` reports the current focus.

## Control props

Every control is a `#[component]` driven by reactive props. Pointer and keyboard selection share
the same state and change callbacks, and the styled controls paint focus outlines.

- `<RadioGroup>` and `<Listbox>` take `labels` and an `Option<usize>` `selected`, and report
  `on_change`. `None` clears the selection, and the first option becomes the entry point.
- `<ToggleButton>` takes `label` and `pressed`; its label stays stable as `pressed` changes.
- `<Tree>` takes the `keys` of the visible rows in tree order, an `item` callback answering the
  `TreeItem` (label, depth, whether it can expand, whether it is expanded) for one key, and the
  `selected` key, which may be a row hidden inside a collapsed one. It reports `on_select`,
  `on_expand`, `on_reveal` and `on_hover_change`, and its children build the cells of one row. The
  tree owns the roving Tab stop, the marker, the indent and the keyboard model; the caller owns the
  rows, so a tree of anything keyed by anything hashable works. The beui inspector uses it.
- `<Link>` takes `label`, an optional `glyph` and `on_click`. It reads as a link, underlines while
  hovered or focused, and goes muted and unclickable while `disabled`. It is the control for "take
  me to that thing": `block_editor_beui::BlockLink` is one, which given the `Editor` and an
  `Option<ChildTarget>` follows the block's name and icon, opens it in the host on click, and shows
  its `fallback` while the reference has not resolved.
- `<Select>` takes `options` and `selected` and opens a popup with a search box over the options.
- The options of `<Select>`, `<Tabs>`, `<Listbox>`, `<RadioGroup>` and `<ResponsiveTabs>` are a
  `Children<unstyled::ChoiceOption>` written as `<ChoiceOption>` tags, each label a prop that
  follows its signal. Options from runtime data are built as a `Vec` of those tags, not of labels.
- `<ContextMenu>` wraps a `region` so a secondary click opens a menu built from `items`, a
  `Children<unstyled::MenuItem>` written as `<MenuItem>` tags, with a submenu's items between its
  own tags. `on_select` receives the selected item's index path through any submenus. Each item's
  `label` and `disabled` are ordinary props, so rows follow their signals without a rebuild.
- `Select` and `ContextMenu` sit on the base `overlay` element: an anchored, viewport-relative
  popup painted above the tree that traps Tab while open.

beui-demo shows these on its Choices (radio group, listbox, select), Buttons (toggle button), Menus
and Tree pages.

### Composing controls

The unstyled controls hand their interaction state to whoever renders their content.
`<unstyled::Button content={...}>` calls the content builder with a `ButtonHandle` carrying
`hovered`, `active`, `focused` and `disabled` signals; the text input and select do the same. Build
reactive props (and effects reacting to a child's hover or focus) from those signals instead of
querying the control's state afterwards.

### Reading control state back

Reading a control's state out, rather than owning the signal that drives it, is for tests and host
integration: `*_selected`, `*_open`, `*_pressed`, `slider_value`, `text_input_value` and friends
take `&Document` and a node. `focus_*` takes just the node and focuses the control ambiently.

## Text, IME and the clipboard

Host events:

- `Event::Focus(false)` when the window or editor region loses focus.
- `Event::Text` for text; `Event::Paste` for a paste.
- `Event::Ime` for an input method's edits.
- Copy and cut return text in `FrameOutput::copied_text`, which the host writes to its clipboard.
  The desktop runner and the block editor integration both handle it; other custom hosts handle it
  in their platform integration.

`Event::Ime` carries a few operations every platform's input method maps onto, with ranges in
UTF-8 bytes of the field's text: set the composing text, commit text, finish composing, mark
existing text as the composition, replace a range (an autocorrect suggestion), delete around the
selection, and set the selection.

- A focusable's `on_ime` receives them. One without it gets committed and finished compositions
  through `on_text`.
- A text area hands them to text-editor-core as `EditorCommand::Ime`, which writes the composition
  into the document and tracks its range (`Core::composition`, drawn underlined); any other command
  ends it. The text area leaves keys to the input method while a composition lasts.
- It reports its caret through `Interactive`'s `ime_cursor` and the surrounding text through
  `ime_text`, so `WindowState::ime` places the candidate window beside the caret and the runner can
  show the input method the field's real text. `beui_core::app::ime_mirror::ImeMirror` keeps a
  platform text field (the web's hidden textarea) in step with it and turns its edits back into
  these operations.

### The on-screen keyboard

`ImeArea::keyboard` says whether the runner should show an on-screen keyboard:

- `Keyboard::Held` for a focusable with `keyboard_on_focus=false` (a select's search box): it takes
  hardware keyboard text as soon as it is focused, but holds the on-screen keyboard back until it
  is tapped.
- `Keyboard::Shown { asks }` otherwise. `asks` is the document's count of taps on a focused field
  that wants the keyboard (any press on it, including from a mouse), so a tap changes the window
  state even when nothing else did, and a user who closed the keyboard gets it back by tapping.
- Android opens the keyboard whenever it turns `Shown` or `asks` changes. The web adapter refocuses
  its hidden textarea within the press itself, since a browser opens its keyboard only during the
  user's gesture.
- A plugin's region passes its editor's answer on whole: `ime_keyboard` for whether it is held, and
  `Interactive`'s `keyboard_asks` for the editor's count, which a focusable shows in place of the
  document's while not counting its own taps. So a tap on the plugin asks only when the editor's own
  document decided it should.

## Touch

beui accepts `Event::Touch` with a stable device and finger id, a lifecycle phase, a logical
position and optional normalised pressure. It tracks every active contact in `InputState::touch`.

- The first contact drives the primary pointer, so pressables, sliders, text selection, focus and
  overlays work with no touch-only API.
- A second contact or a cancelled contact cancels a pending tap.
- A second finger landing while the first is still held where it touched down is a secondary drag,
  reported through `Interactive`'s `on_secondary_drag` exactly as a right-button drag is
  (`SecondaryDrag`: where it began, where it is, whether it started, ended or was cancelled, and
  the modifiers). That is how a board marks up arrows by either means. Once the held finger moves,
  it is a pinch and the secondary drag is cancelled.
- A tap may drift up to eight logical points. Beyond that the gesture locks to its dominant axis.
- Vertical gestures drag the deepest scroll view under the initial contact, keep that scroll
  captured when the finger leaves its rect, and do not click the row they started on. A released
  vertical drag coasts with its sampled velocity; a drag beyond either end is resisted, then
  springs back.
- Horizontal gestures remain available to controls such as sliders.
- Touch contacts do not hover, focus is assigned only once a gesture resolves as a tap, and the
  touch pointer disappears on release, so no hover styling is left behind.
- Platforms may send their own mouse event per touch; beui drops a mouse event that lands on a
  finger, so a real mouse elsewhere keeps working while a finger is down.

Sources of touch:

- Standalone apps receive winit touch events automatically.
- The inspector (Ctrl+Shift+I) has "Emulate touch with mouse", which turns the primary button into
  a touch contact.
- Embedded beui plugins receive the same data over the block plugin input protocol.

## Testing input

Keyboard tests run without a window. `./scripts/buck test //crates/beui:test` runs the tests that
drive whole documents, and each `beui-*` crate has its own `:test`. `block_ui_test::BeuiTest` has
`touch_start`, `touch_move`, `touch_end` and `touch_cancel` for headless gesture tests, `finger`
for gestures of more than one finger, and `secondary_drag` for a right-button drag. Writing GUI
tests is in `guides/testing.md`.

## Keyboard behaviour of the styled controls

The styled controls follow the keyboard conventions of the
[W3C Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/).
Check this list before changing a control's keys, and keep it current.

- **Buttons, links, toggle buttons, checkboxes, switches, list rows, disclosure and accordion
  headers**: Space or Enter activates on release. Escape or focus loss cancels a held activation.
  Key repeats do not activate again.
- **Tabs**: one Tab stop, at the selected tab. Left/Right wrap and select immediately. Home/End
  select the first/last tab. Up/Down leave a horizontal tab selection alone.
- **Radio groups**: one Tab stop, at the selected option or the first when unselected. Arrows wrap
  and select; Space selects without clearing an existing selection. Home/End select the first/last.
- **Single-select listboxes**: one Tab stop. Up/Down select previous/next and stop at the ends.
  Home/End select the first/last. Typing searches case-insensitive prefixes; repeating a letter
  cycles matches. The search resets after one second or when focus leaves.
- **Sliders**: Right/Up increase and Left/Down decrease by 5% of the track; Page Up/Down by 20%.
  Home/End select minimum/maximum. Values stay within `min` to `max` (default 0 to 1). A curved
  `scale` keeps steps even along the track, so they are small where the track is fine and large
  where it is coarse.
- **Text inputs**: Left/Right, Home/End, Shift-selection, Ctrl/Alt word navigation and deletion,
  Ctrl+A, Ctrl+C/X, Ctrl+Z, Ctrl+Shift+Z/Ctrl+Y, and Enter to submit. Up and Down move to the start
  and end of the text, as on the one line of a multiline area. Space inserts text. Paste replaces
  the selection. A secondary click opens the Copy, Cut, Paste and Select All menu that a touch tap
  on the selection or the caret handle opens. The desktop runner maps Command to Ctrl on macOS;
  Super chords type nothing.
- **Multiline text areas** (in addition): Tab and Shift+Tab indent; Escape then Tab or Shift+Tab
  moves the focus out. They register Find (Ctrl+F), Find and replace (Ctrl+H), next and previous
  match (Ctrl+G, Ctrl+Shift+G), Duplicate the line (Ctrl+Shift+D), Select the next occurrence
  (Ctrl+D) and folding (Ctrl+Shift+[) as actions, which answer while the focus is in the area.
- **Selectable text**: `<SelectableText>` makes the plain text nodes under it selectable together.
  A mouse drag selects from one text to another in tree order, Shift+click extends, and a click
  clears. Ctrl+C copies, joining texts on one line with a space and lines with a line break; Ctrl+A
  selects everything under it. A secondary click opens a Copy and Select All menu. It is not a Tab
  stop, and its keys also reach it from a focused control inside it.
- **Scroll areas**: Tab focuses the area. Up/Down scroll by a line; Page Up/Down and
  Space/Shift+Space by a page; Home/End reach the ends. Tabbing to a child or navigating a choice
  scrolls it into view. Up/Down, Home/End and Page keys a child control leaves unused scroll the
  nearest containing area. Virtual lists can be paged before tabbing into their realized controls.
- **Select (dropdown)**: clicking or activating the trigger opens the popup and focuses its search
  box; typing filters options by case-insensitive substring. Up/Down/Home/End on the closed trigger
  also open it and move the highlight that way. Up/Down move the highlight without moving the
  caret; Home/End jump to the first/last visible option. Enter confirms and closes. Escape or an
  outside click closes without changing the selection and returns focus to the trigger.
- **Tree views**: one Tab stop, at the selected row or the first. Up/Down move to the
  previous/next visible row and stop at the ends; Home/End reach the first/last. Right expands a
  collapsed row, then moves to its first child; Left collapses an expanded row, then moves to its
  parent. Space or Enter selects a row like a click and leaves it open or closed; only Left, Right
  and the chevron expand or collapse. Typing searches case-insensitive prefixes over visible rows.
  Selection follows the focused row.
- **Pan and zoom areas**: Tab focuses the area. Arrows pan by a step; `+` and `-` zoom around the
  middle of the viewport and `0` returns the scale to one. The area owns these keys, so arrows pan
  it rather than scrolling what contains it.
- **Dock**: one Tab stop per tab bar, at the tab the pane shows, walked like any tab list; the bar
  scrolls the focused tab into view. The bar between two panes is a Tab stop with a `Splitter` role
  that the arrows move. Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of the pane the focus is in,
  wherever inside it. The switcher across every pane and window (`DockingLayout::begin_switch`) is
  bound to no key.
- **Calendar**: one Tab stop in the grid, at the focused day. Left/Right move a day and Up/Down a
  week, crossing months. Home/End reach the start/end of the week. Page Up/Down move a month and
  Shift+Page Up/Down a year, keeping the day within the month. Space or Enter picks the day. Days
  outside `min` and `max` are skipped. The title's month button switches to a grid of months
  (arrows move by a month and a row, Page Up/Down by a year, Enter shows that month's days); its
  year button to a grid of twenty years (arrows by a year and a row, Page Up/Down by twenty years,
  Enter shows that year's months).
- **Date and time fields**: each segment is a Tab stop and a spin button. Up/Down step it and wrap;
  Page Up/Down step by a larger amount; Home/End reach its first/last value. Left/Right move between
  segments and stop at the ends. Alt+Down opens the calendar over the field with focus kept in the
  segment. Digits type into the segment and move on once it is full or no further digit could fit;
  `-`, `/`, `:` and space move on too, and `a`/`p` set AM/PM. Backspace or Delete clears a segment,
  and Backspace on an empty one goes back a segment. The button beside the segments opens the
  calendar and time list; Escape closes them and returns focus to it.
- **Time lists**: one Tab stop, at the selected time. Up/Down move a time, Page Up/Down an hour,
  Home/End the first/last. Laid out as a grid, Left/Right move a time, Up/Down a row and Page
  Up/Down four rows. Space or Enter picks the time.
- **Color area**: Tab focuses it. Left/Right change saturation and Up/Down brightness by 1%, or 10%
  with Shift; Page Up/Down change brightness by 10%; Home/End reach no and full saturation. Hue and
  opacity are sliders.
- **Context menu**: a secondary click opens it at the pointer and focuses its first item, shown
  highlighted; Tab is trapped on the menu's single roving Tab stop while open. Up/Down move between
  items; Home/End jump to the first/last. Right Arrow (or hovering an item) opens its submenu and
  focuses its first item; Left Arrow closes a submenu and refocuses the item that opened it. One
  submenu per level stays open. Enter or a click on a leaf selects it and closes the whole stack;
  Escape closes one level at a time; an outside click closes the whole stack.
- **Keep changes prompt**: opens with focus on Keep, so Enter keeps; Escape reverts, as running out
  its countdown does.
