# beui focus, keys and input

How input reaches beui controls: the keyboard conventions they follow, the
path a key takes, actions and shortcuts, how embedded editors and Wayland
programs get their input, focus, the input method and touch. The dispatch is
`crates/beui-core/src/interact.rs` and its folder; the input vocabulary (`Key`, `Modifiers`, `KeyChord`, `Input`, IME and
touch events) is in `be-lib-ui-types` and `be-lib-ui-keys`, which the app
protocol shares, so nothing converts between a host's events and an app's.
Keyboard tests run headless: `./scripts/test //crates/beui-host:test` drives
whole documents, and each `beui-*` crate has its own `:test`.

## Control conventions

The styled controls follow the [W3C Authoring Practices
Guide](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/); each
control's exact keys are in its code and tests. The patterns that span them:

- Buttons and everything pressable activate on the release of Space or Enter;
  Escape or losing focus cancels, and key repeats do not activate again.
- A group (tabs, radio group, segmented control, listbox, tree, calendar
  grid, time list, dock tab bar) is one Tab stop, at the selected item, with
  arrows inside it. Selecting programmatically moves the stop, and moves the
  focus only if the group already has it. Tabs and segmented controls without
  room become a `Select`.
- A toolbar or dock bar keeps every control a Tab stop and adds arrow focus
  (`arrow_focus(row, direction)`); a control that uses the arrows, such as a
  text input, keeps them.
- A scroll area is a Tab stop only while it overflows and nothing inside it
  is one (`yields_tab_stop`). It takes its scrolling keys only while it can
  scroll that way, so they bubble to the next area out, as nested boxes do in
  a browser.
- A single-line text input with no `on_submit` leaves Enter to what is around
  it: a dialog runs its confirm action on an Enter nothing took, unless that
  action is disabled or destructive. A multiline area takes Tab to indent;
  Escape then Tab leaves it.
- The context menu opens on Shift+F10 or the Menu key with the focus in its
  `ContextMenu` or on the control it is part of; Escape closes one level.
- The inspector is a document of its own; Ctrl+Shift+F moves the focus into
  it and back.

## How a key travels

Every press, repeat and release takes one trip along the focus path, like a
DOM event: from the host, through the window, down the tree from the root to
the focused node (the target), and back up. With nothing focused, the target is
the topmost focus-trapping layer, or the root, and every listener in that layer
hears the key. Listeners sit at positions (`KeyAt`):

- `Host` and `Window`, above the tree, on every path. Listeners and actions
  registered inside a hosted `Window` hear only that window's keys.
- `Root` and `Node(node_ref)`, on the path only while the focus is inside them.
- `Control(node_ref)`: a `Node` that also hears keys while the focus is on the
  control it is part of (the nearest focusable above it), so a wrapper inside
  a control's face, such as `ContextMenu` in a tree row, answers for it.

Each listens in the capture phase (down, before the target) or bubble phase
(up, after it): `on_key(at, phase, handler)`. The first to answer `true` ends
the trip. A listener at the focused node itself is in both phases, which is how
a compound control steers keys before its inner control handles them: the
select and command palette capture the arrows on their search box.

The document's own keys are listeners too (`builtin` in interact.rs), in a
fixed order: the first host
capture listener ends autoscroll (taking Escape); the last window capture
listener is F6/Shift+F6, moving focus between forwarding catchers; the first
window bubble listener closes the innermost dismissable layer on Escape; the
last host bubble listener is Tab, Escape (cancelling a held activation) and
Back. So a window listener for Escape hears it only when no layer took it, and
one that wants Escape over a program calls `Document::escape_layer` and
`dismiss_overlay` itself (be-host's presentation mode). Ctrl+V asks the host
for a paste after the trip, only if nothing took the key.

Answering a key scrolls the focus into view. A handler that reveals something
else (a `SearchList` row it highlights while the field keeps the focus)
registers with `on_scroll_key`, or the core's reveal of the focus overrides its
own.

**Holding.** Whatever takes a press holds that key: a capture listener gets its
repeats, text and release alone (`phase` `Held`), wherever the focus goes; a
target or bubble listener gets its release while repeats travel again. So the
release of an Escape that closed a menu never reaches the program the focus
returned to. This is the only record of taken keys.

**Boundaries.** A focus-trapping layer (menu, dialog, popover) cuts the tree
outside it from the path, so shortcuts outside a dialog don't hear keys typed
in it; host and window still do. A program (a forwarding catcher with the
focus: an app's view, a Wayland window) is the target and takes every key that
reaches it, so only host and window capture listeners hear its keys; with
`inhibits_shortcuts` it cuts the window too. The lock screen cuts host and
window for everything but media keys.

`on_shortcut(handler)` is a tree capture listener at the registering component,
seeing every press before the focused control (the dock's Ctrl+Tab, which
works from a text input in a pane but not past a dialog). `on_global_key(handler)` is a
window capture listener, handed a `KeyEvent` with the target's kind (typing,
program). `held_modifiers()` is a signal of the modifiers held; hold-and-release
shortcuts (a switcher) are built from these two.

**Modifiers.** The modifier keys arrive as `Key::Shift`/`Ctrl`/`Alt`/`Logo`
presses and releases, but reach only host and window capture listeners and
forwarded views; the tree reads `KeyPress::modifiers` or `held_modifiers()`. A
modifier pressed and released alone is a tap, judged from key events alone
(runners disagree on whether `Event::Modifiers` comes first), offered on the
release as a `KeyInput::Tap` to host and window capture listeners. React to one
with an action on `Chord::tap(Key::Logo)`. Super is `logo`, except on macOS,
where Command acts as Ctrl.

**Naming a key.** `Key` is what shortcuts match, and `Key::from_typed` names it
the same on every platform: the ASCII letter or digit the key types under the
person's layout, otherwise the key at that place on a US keyboard. So Ctrl+Z
follows the layout's `z`, a Cyrillic letter is the key it sits on, punctuation
is positional (AZERTY's comma key is `Key::M`), and Shift never changes the
answer (US `!`, AZERTY `&` are `Key::One`). Each runner feeds it the unmodified
character and the physical key (on the web `KeyboardEvent.key` and `code`,
never `keyCode`). `evdev` on a key is its physical place as a Linux evdev code
on every platform, for programs that apply their own layout (Wayland clients);
`None` for an on-screen key.

## Actions and the command palette

A command - button, menu row, shortcut, palette entry - is written once as an
`Action`, and everything offering it is handed the action:

```rust
let group = Action::new("canvas.group", "Group", move || state.run(Group))
    .glyph(ICON_COMPONENT)
    .shortcut(Chord::ctrl(Key::G))
    .enabled(can_group)
    .register();
view! { <Button icon_only=true action={group.clone()} /> <MenuItem action={group} /> }
```

Controls with an `action` prop take its label, glyph, disabled state,
shortcut and `checked`/`radio` state from it. `register` adds
it to the nearest `ActionScope` until the registering scope goes. An action is
a key listener: `action_scope(&node_ref)` listens in the bubble phase at that
node, so its actions answer only while the focus is inside it, innermost
first; with no scope, at the root. `.capture()` answers before the focused
control. A chord without Ctrl, Alt or Super (and not a media key) never reaches
actions while the target types text or is a program, so typing a letter never
switches a canvas tool.

`.at(KeyAt::Window)` hears what nothing in the tree took, dialogs included, and
the palette lists it everywhere. `.at(KeyAt::Window).capture()` is the opt-in to
take a key from a focused program (the desktop's Super+F and media keys); only
chords with Ctrl, Alt or Super, or media keys, are offered over a program. With
`inhibits_shortcuts` (a remote desktop, a VM) only `.at(KeyAt::Host).capture()`
actions answer: be-host's Super+L lock and be-host-wayland's Super+Escape
(guides/desktop/linux.md).

A document inside a host - an app's - intercepts the same way: the chords of
its enabled capturing host and window actions are
`Document::intercepted_keys`, sent as `WindowRequests::intercepted_keys`. The
host offers them ahead of the focused program and hands a press it takes, with
its repeats and release, back as `Event::InterceptedKey` (taps as
`InterceptedTap`), and keeps sending held modifiers until the chord's are
released, which is how Alt+Tab works over any window. be-host takes intercepted
keys only from the shell's frame, and not while the view has the keyboard
itself (`be_host_runtime::view_has_keyboard`), when it hears them in order
instead. A document with a focus-trapping layer open reports
`WindowRequests::wants_keyboard`, so be-host gives an app's view the focus
while its dialog is open.

`styled::CommandPalette` lists the actions live where the focus was when it
opened, puts the focus back before running one, and opens with Ctrl+Shift+P in
an editor frame. Given `items={PaletteItems::list(..)}` it searches arbitrary
`PaletteItem`s instead. It, `styled::SearchList` (inline, no overlay) and
`Select` are stylings of `unstyled::SearchList`. A menu is described once as
`MenuNode`s; `<MenuNodes nodes />` renders it, and `add_to_menu` adds to the
document's own `document_menu()`, which an editor frame hands its host.

## One input path

Every embedded editor - an app's view, and each Wayland program's window in
`be-host --desktop` - is a forwarding catcher: an `Interactive` with
`on_forward` (and `hit_area` for the points it takes), handed a
`ForwardedInput` of what beui routed to it: pointer events (with `over`, the
catcher under the event, which a drag between Wayland windows needs), `Left`,
`Focus`, and keys, text, IME and drops. The host keeps no capture, hover or
focus of its own (`be_host_runtime::forward_view` for an app,
`be_host_wayland::Compositor` for a window), so capture, claims, overlays,
focus, interception and the lock apply to both by one set of rules.
`beui_core::interact::forward::Replay` turns a `ForwardedInput` back into the
raw `Input` a platform would push (positions relative to the catcher over its
zoom, `PointersCancelled` for a press that ended unreleased); the app host
fills its `InputBatch` with it, so an app's runner sees what a window's does.

An event goes through, in order:

1. The runner. `beui-platform-drm` drops the input that wakes a blanked screen
   (`wake.rs`, guides/desktop/linux.md), on libinput's clock before the keymap
   sees it. A key is one `Event::Key` with its text and evdev code;
   `Event::Text` is text with no key.
2. `InputState::begin_frame` turns events into `PointerEvent`s, each where the
   pointer was then. A mouse event landing on a finger is the platform's copy
   of the touch and is dropped.
3. `interact`: per pointer event, one hit test, closing layers it is
   outside of, claims and capture, the pointer's session, and focus.
4. Keys, one at a time along the focus path (above). A key one listener takes
   is gone, text and all; a key bound for a program goes no further.
5. `forward::route`: the pointer to the catcher under it, a press to its
   catcher until all buttons lift, keys, text and IME to the focused one.

So no editor reads the frame's events itself. A host showing several documents
in one frame (the inspector beside the program) gives each its share with
`Context::with_input`: `InputState::only`, `without_pointer`, and `Focus(false)`
/ `Focus(true)` around the time its keys go elsewhere.

## Focus

Tab walks visible controls in tree order and wraps within the document. A
hidden or removed node loses the focus and hears `on_focus_change(false)`.
`tab_stop=false` keeps a control focusable by pointer and code but off Tab
(single-stop groups use it). `press_focus=false` on any `Interactive` or
control built on one makes a press anywhere inside it leave the focus, and a
text field's caret, where they are; put it on a whole toolbar, not on each
button, so its padding keeps the focus too. `Document::watch_focused` signals
the focus and `holds_focus(node_ref)` is a memo of whether it is inside a node.

`Input::Focus(false)` from a host means the window lost the keyboard only: the
runner lets go of modifiers and beui of held keys, but a press continues. A
press whose release will not come ends with `Input::PointersCancelled`
(guides/beui/platforms.md, Input).

## Text and the input method

Text arrives as `Input::Text`, pastes as `Input::Paste`, and input-method edits
as `Input::Ime`: a few operations every platform maps onto (compose, commit,
finish, mark, replace range, delete around, set selection), with ranges in
UTF-8 bytes. A focusable's `on_ime` gets them; without one, commits arrive
through `on_text`. A text area passes them to beui-text-editor-core as
`EditorCommand::Ime`, leaves keys to the input method while composing, and
reports `ime_cursor` and `ime_text` so the candidate window sits by the caret
(`ImeMirror` in `beui-web-input` keeps the web's hidden textarea in step). Copy and cut return text in `FrameOutput::copied_text` for the host to
write to its clipboard. `clipboard_keys` on an `Interactive` (a terminal) tells
the web platform not to let the browser run its own copy, cut and select-all
(guides/beui/platforms.md, Clipboard and paste).

`ImeArea::keyboard` says whether to show an on-screen keyboard.
`Keyboard::Held` is a focusable with `keyboard=SoftKeyboard::OnPress` focused
after a finger or pen press: hardware keys work at once, but the on-screen
keyboard waits for a tap or a non-media key. Otherwise it is
`Keyboard::Shown { asks }`, where `asks` counts taps on the focused field so a
tap reopens a keyboard the person closed. The web refocuses its hidden
textarea within the press, since browsers open a keyboard only during a
gesture. An app's view passes its editor's answer
on as `Interactive`'s `keyboard`, replacing the document's own. beui's own
on-screen keyboard is in guides/beui/space.md, The on-screen keyboard.

## Touch

`Input::Touch` carries a device and finger id, phase, position and optional
pressure; the first contact drives the primary pointer, so pressables, sliders,
selection, focus and overlays work unchanged. A second contact or a cancel
cancels a pending tap. A second finger landing while the first is still held is
a secondary drag (`unstyled::gestures::secondary_drag`, as a right-button drag)
until the held finger moves, which makes it a pinch.

A tap may drift 8 points; beyond that the gesture locks to its axis, and a
vertical one drags the deepest scroll under it and clicks nothing. Touch never
hovers, and focus moves only once a gesture resolves as a tap. The inspector's "Emulate touch with mouse" (Ctrl+Shift+I) turns the mouse
into a finger; tests use `be_sdk_test::BeuiTest`'s `tap`, `fingers` and
`secondary_drag`, as beui's `testing::Harness` does.
