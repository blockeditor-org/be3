# beui controls and keyboard behavior

The styled controls follow the keyboard conventions in the [W3C Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/). They paint focus outlines, and pointer selection and keyboard selection share the same state and change callbacks.

| Control | Keyboard behavior |
| --- | --- |
| Buttons, links, toggle buttons, checkboxes, switches, list rows, disclosure and accordion headers | Space or Enter activates on release. Escape or focus loss cancels a held activation. Repeated key-down events do not activate repeatedly. |
| Tabs | One Tab stop, at the selected tab. Left/Right wrap and select immediately. Home/End select the first/last tab. Up/Down leave the horizontal tab selection alone. |
| Radio groups | One Tab stop, at the selected option or the first option when unselected. Arrows wrap and select; Space selects without clearing an existing selection. Home/End select the first/last option. |
| Single-select listboxes | One Tab stop. Up/Down select the previous/next option and stop at the ends. Home/End select the first/last option. Typing searches case-insensitive prefixes; repeated letters cycle matches. The search resets after one second or when focus leaves. |
| Sliders | Right/Up increase and Left/Down decrease by 5% of the track. Home/End select minimum/maximum. Page Up/Down adjust by 20% of the track. Values remain within the range, which is `min` to `max` and defaults to 0 to 1. A curved `scale` keeps the steps even along the track, so they are small where the track is fine and large where it is coarse. |
| Text inputs | Left/Right, Home/End, Shift-selection, Ctrl/Alt word navigation and deletion, Ctrl+A, Ctrl+C/X, Ctrl+Z, Ctrl+Shift+Z/Ctrl+Y, and Enter to submit; a single-line input with no `on_submit` leaves Enter to what is around it, such as a dialog's confirm action. Up and Down move to the start and the end of the text, as they would on the one line of a multiline area. Space inserts text. Paste replaces the selection. A secondary click opens the Copy, Cut, Paste and Select All menu that a touch tap on the selection or the caret handle opens. The desktop runner maps Command to Ctrl on macOS; Super chords type nothing. A multiline text area takes Tab and Shift+Tab to indent; Escape then Tab or Shift+Tab moves the focus out of it instead. It registers Find (Ctrl+F), Find and replace (Ctrl+H), the next and previous match (Ctrl+G, Ctrl+Shift+G), Duplicate the line (Ctrl+Shift+D), Select the next occurrence (Ctrl+D) and folding (Ctrl+Shift+[) as actions, which answer while the focus is in it. |
| Selectable text | `<SelectableText>` makes the plain text nodes under it selectable together: a mouse drag selects from one text to another in tree order, Shift+click extends the selection, and a click clears it. Ctrl+C copies the selection, joining texts on one line with a space and lines with a line break; Ctrl+A selects everything under it. `copy_key` and `select_all_key` (each an `Option<Chord>`) change those keys or turn them off with `None`, as the terminal does to leave Ctrl+C and Ctrl+A to the shell (copying on Ctrl+Shift+C instead); a secondary click still opens the Copy and Select All menu, which shows whichever keys are set. A drag past the edge of the scroll around it scrolls. It is not a Tab stop, and the keys also reach it from a focused control inside it. |
| Scroll areas | A scroll area is a Tab stop only while its content overflows and nothing inside it is a Tab stop; with a control inside, the keyboard reaches the area through that control, so it drops its own stop. Focusing anything inside scrolls it into view. A scroll area takes the keys that scroll it only while it can scroll that way, so one already at its end leaves a key to the area around it, as nested scrolling boxes in a browser do: the arrows along its axis by a line and Home/End to the endpoints, and a vertical one also Page Up/Down and Space/Shift+Space by a page. Those keys that the focused control leaves unused bubble to the nearest area that takes them, as a browser scrolls an overflowing box around a focused button. Tab and Shift+Tab walk a virtual list's rows as if every row were built, building the ones they reach, and a focused row scrolled out of view keeps the focus. |
| Toolbars and docks | Every control is a Tab stop. Left/Right move the focus to the previous/next control and stop at the ends; Home/End reach the first/last. A control that uses those keys itself, such as a text input, keeps them. `arrow_focus(row, direction)` gives a row of controls this, and `block_editor_beui`'s `Toolbar` and `BottomDock` and the text editor's format bar use it. |
| Select (dropdown) | Clicking or activating the trigger opens the popup and focuses its search box; typing filters the options by case-insensitive substring. Up/Down/Home/End on the closed trigger also open the popup and move the highlight in that direction. Up/Down move the highlighted option without moving the text caret; Home/End jump to the first/last visible option. Enter confirms the highlighted option and closes the popup. Escape or an outside click closes the popup without changing the selection and returns focus to the trigger. |
| Tree views | One Tab stop, at the selected row or the first row. Up/Down move to the previous/next visible row and stop at the ends; Home/End reach the first/last row. Right expands a collapsed row and then moves to its first child; Left collapses an expanded row and then moves to its parent. Space or Enter selects a row, the same as clicking it, and leaves it open or closed; only Left, Right and the chevron expand or collapse. Typing searches case-insensitive prefixes over the visible rows. Selection follows the focused row. |
| Pan and zoom areas | Tab focuses the area. Arrows pan by a step; `+` and `-` zoom around the middle of the viewport and `0` returns the scale to one. The area owns the keys it uses, so arrows pan it rather than scrolling whatever contains it. |
| Dock | One Tab stop per tab bar, at the tab the pane is showing, walked like any other tab list; the bar scrolls the tab that takes focus into view. The bar between two panes is a Tab stop with a `Splitter` role that the arrows move. Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of the pane the focus is in, wherever the focus is inside it. The switcher across every pane and window (`DockingLayout::begin_switch`) is bound to no key. Pressing a pane's or window's bar, tab or grips gives the keyboard to what its tab shows (guides/beui.md). |
| Calendar | One Tab stop in the grid, at the focused day. Left/Right move a day and Up/Down a week, crossing into the next or previous month. Home/End reach the start/end of the week. Page Up/Down move a month and Shift+Page Up/Down a year, keeping the day within the month. Space or Enter picks the day. Days outside `min` and `max` are skipped. The title's month button switches to a grid of months, where the arrows move by a month and a row, Page Up/Down by a year, and Enter shows that month's days; its year button switches to a grid of twenty years, where the arrows move by a year and a row, Page Up/Down by twenty years, and Enter shows that year's months. |
| Date and time fields | Each segment is a Tab stop and a spin button. Up/Down step it and wrap; Page Up/Down step by a larger amount; Home/End reach its first/last value. Left/Right move between segments and stop at the first and last. Alt+Down opens the calendar over the field with the focus kept in the segment. Digits type into the segment and move on once it is full or no further digit could fit; `-`, `/`, `:` and space move on too, and `a`/`p` set AM/PM. Backspace or Delete clears a segment, and Backspace on an empty one goes back a segment. The button beside the segments opens the calendar and time list; Escape closes them and returns the focus to it. |
| Time lists | One Tab stop, at the selected time. Up/Down move a time, Page Up/Down an hour, Home/End the first/last. Laid out as a grid, Left/Right move a time, Up/Down a row and Page Up/Down four rows. Space or Enter picks the time. |
| Color area | Tab focuses the area. Left/Right change the saturation and Up/Down the brightness by 1%, or 10% with Shift; Page Up/Down change the brightness by 10%; Home/End reach no saturation and full saturation. Hue and opacity are sliders. |
| Context menu | Secondary click opens the menu at the pointer and focuses its first item, which is shown with a highlighted background; Tab is trapped on the menu's single roving Tab stop while it is open. Up/Down move between items and update the highlight; Home/End jump to the first/last item. Right Arrow (or hovering an item) opens its submenu and focuses its first item; Left Arrow closes a submenu and refocuses the item that opened it. Only one submenu per level stays open. Enter or clicking a leaf item selects it and closes the entire menu stack; Escape closes one level at a time; an outside click closes the whole stack. |
| Dialogs | Enter that the focused control does not take runs the dialog's confirm action, unless that is disabled or destructive. A focused button still answers Enter itself. |
| Keep changes prompt | Opens with the focus on Keep, so Enter keeps; Escape reverts, as running out its countdown does. |

The inspector panel is a document of its own, so Ctrl+Shift+F moves focus into
it and back, and Escape inside it returns focus to the inspected document.

## How a key travels

Every key press, repeat and release takes one trip along the focus path, the
way a DOM event does. The path runs from the host, through the window, down the
tree from the document root to the focused node (the target), and back up
again; with nothing focused, the target is the topmost layer that traps focus,
or the root. Listeners sit at positions on it (`KeyAt`):

- `KeyAt::Host` and `KeyAt::Window` are above the tree, on every path.
- `KeyAt::Root` and `KeyAt::Node(node_ref)` are tree nodes, on the path only
  when the focus is inside them. With nothing focused, every listener in the
  topmost trapping layer hears the key, as if the focus could be anywhere in it.

and each listens in the capture phase, on the way down before the target, or the
bubble phase, on the way back up after it (`on_key(at, phase, handler)`). A key
one of them holds (below) skips the trip and goes straight to its holder. The
trip is: host and window capture listeners, tree capture listeners from the
root down to the target, the target itself (its `on_key`, then its own
activation and arrow stepping), tree bubble listeners from the target up, and
the window and host bubble listeners. The first listener to answer `true` ends
the trip. The document's own keys are listeners on it too (`builtin` in
interact.rs), which the lock never cuts unless noted: the first host capture
listener ends autoscroll on any press and takes Escape doing it; the last
window capture listener is F6 and Shift+F6 (cut with the window); the first
window bubble listener asks the innermost dismissable layer around the focus to
close on Escape (see the beui guide; a key bound for an app never gets that
far, so a window capture listener that wants Escape over an app asks
`Document::escape_layer` and `dismiss_overlay` itself, as block-app's
presentation mode does); and the last host bubble listener is Tab,
Escape (which cancels a held activation) and Back. So a window listener for
Escape hears it only when no layer took it. Ctrl+V asks the host for a paste
once the trip is over, only if nothing took the key (an app counts as nothing),
or if what took it asked for a paste itself. Ctrl+Shift+V is no different, and
Ctrl+Shift+C reaches the focus as a key like any other chord (beui's inspector
takes it to pick a node unless the focus is in an `Interactive` with
`clipboard_keys`). That prop is for a control that wants Ctrl+C, Ctrl+V, Ctrl+X
and Ctrl+A as keys, such as a terminal: it does nothing to the trip, but tells
the web adapter, which must decide in `keydown` whether the browser runs its own
copy, paste, cut or select-all, to stop it for those keys and for Ctrl+Shift+C
while leaving Ctrl+Shift+V to the browser's paste (guides/beui.md, the runner).

A listener at the focused node itself is on the path in both phases, which is
how a compound control steers keys before the control inside it handles them:
the command palette and select put a capture listener on their search box
(`@node_ref` on the `TextInput`) for the arrows, and a scroll area puts a bubble
listener on itself for the arrows its children leave. Answering a key scrolls
the focus into view; `on_scroll_key` registers a listener whose keys move the
view themselves and so do not.

Boundaries cut the path, and they replace what used to be flags:

- A layer that traps focus (a menu, a dialog, a popover) is a boundary for the
  tree: with the focus inside it, nodes outside the layer are not on the path,
  so shortcuts and actions outside a dialog do not hear keys typed in it. The
  host and window still do.
- An app - a forwarding catcher with the focus, such as a plugin's region or a
  Wayland program - is the target, and it takes every key that reaches it, so
  nothing bubbles. Only host and window capture listeners hear a key bound for
  an app; tree listeners do not. An app with `inhibits_shortcuts` cuts the
  window too, so only the host hears.
- The lock screen cuts the host and window for every key but media keys.

`on_shortcut(handler)` is a tree capture listener at the component that
registers it (or the root outside a component), offered every press before the
focused control sees it: that is how the dock's Ctrl+Tab reaches it from inside
a text input in a panel, and why it does not while a dialog is open.

`on_global_key(handler)` is a window capture listener. It is handed a
`KeyEvent`: the `input` (a `KeyPress` or a modifier `Tap`), the `phase`, and the
`target` (`node`, and whether it `typing`s text or is an `app`). A listener
that takes a press in the capture phase holds its key: the physical key, the
text it typed, its repeats and its release go to that listener alone, with
`phase` `Held`, wherever the focus has gone since, and nowhere else. Whatever
takes a press later on the trip - the target or a bubble listener - holds its
release the same way, while its repeats take the trip again, so the release of
an Escape that closed a menu never reaches the app the focus went back to.
That is the one record of taken keys; nothing else keeps its own.
`held_modifiers()` is a signal of the modifiers held now, which changes on
modifier presses alone. Shortcuts that are not chords, such as a hold-and-release
switcher, are built from those two.

The modifier keys themselves arrive as `Key::Shift`, `Key::Ctrl`, `Key::Alt` and
`Key::Logo` presses and releases (`Key::is_modifier`), alongside the
`Event::Modifiers` they cause. They reach host and window capture listeners
and forwarded regions (plugins and Wayland programs), but not the tree,
the focused control's `on_key` or actions, which read the state from `KeyPress::modifiers` or `held_modifiers()`; nor
do they show the focus ring or end autoscroll, and the runners drop their repeats.
Something that reacts to the modifier key rather
than to the state, such as tapping Super alone, is a global action with a
`Chord::tap(Key::Logo)` rather than code reading events: the document counts a
modifier pressed and let go with no other key, press, scroll or loss of focus
between as a tap, judged from the key events alone (the runners do not agree on
whether `Event::Modifiers` comes before or after the key's own event, and winit
on X11 can report a modifier state that reverts before the key arrives), and
offers it on the release to the host and window capture listeners as a
`KeyInput::Tap`. The release still goes
on to whatever it would have reached. linux-desktop's program launcher opens
that way. An intercepting tap action (see below) is named among the document's
intercepted keys as a `KeyChord` with `tap` set, and a host that hears that tap
while the focus is elsewhere hands it over as an `Event::InterceptedTap`.

`Modifiers` and `Chord` have `logo` for the Super (Windows) key. The runners
report Super as `logo`, except on macOS, where Command acts as Ctrl and Super
is never reported. `Key` also has the volume, mic mute, brightness and media
transport keys, mapped wherever the platform reports them. `Key`, `Modifiers`,
`KeyChord` and the rest of the input vocabulary (pointer buttons, touch, pen and
back phases, IME events, `CursorIcon`, pastes and picked files, and the raw
`Input` a platform produces) live in the framework-free `ui-types` crate, which
the plugin protocol (`block-plugin-api`) uses too, so nothing converts between a
host's events and a plugin's. A host passes a key on to a plugin as the platform
reported it, physical key and text included, so the plugin's runner sees what the
host's did.

A key has two names. `Key` is what shortcuts match, and one rule names it on
every platform, `Key::from_typed`: if the key's character under the person's
layout is an ASCII letter or digit, `Key` is that letter or digit; otherwise it
is the key at that place on a US keyboard. Named keys (Space, Enter, arrows, F-keys,
modifiers, media keys) are named by what they are. So Ctrl+Z is the key that
types `z` wherever the layout puts it, a Cyrillic or accented letter (`с`, `ą`)
is the key it sits on, and punctuation is positional: AZERTY's comma key is
`Key::M` and US `;` is `Key::Semicolon`, as in VS Code's default keybindings.
Shift can't change the answer: on a letter it only changes case, and on the
digit row whichever side is a symbol falls to its place, which is that digit
(US Shift+1 `!`, AZERTY `&` and Czech `ě` are all `Key::One`). Menus label a
punctuation shortcut with its US character. Each runner gives the rule the
layout's character and the physical key: winit its key without modifiers and
physical key, the web `KeyboardEvent.key` and `code` (never the deprecated
`keyCode`), drm the keysym's character and its evdev code, Android the key's
unmodified character and key code. Web and winit share `Key::from_value`, which
reads a W3C key value. `evdev` on `Keystroke` and `Event::Key` is where the key
sits, as a Linux evdev code (`KEY_A` is 30), on every platform: winit and the
web map their W3C physical codes with `Key::evdev_from_code`, Android passes
its scan code and drm its evdev code, and drive sends `Key::evdev`;
`Key::from_evdev` names the key in that place. It is `None` for a key with no
physical place, such as one from an on-screen keyboard. It is for programs that
apply a layout of their own, such as be-wayland's Wayland clients; shortcuts
match `Key`.

Tab and Shift+Tab traverse visible controls in tree order and wrap within the document. Hidden panels and collapsed content are excluded. A focused node that is hidden (a closed overlay, a frame made invisible, a portal or parent that stops holding it) or removed loses the focus, and its `on_focus_change` hears `false`. Changing a selection programmatically updates the group's Tab stop and moves focus with the selection when the group already contains focus. Programmatic changes do not pull focus from other controls. Empty groups have no Tab stop, and invalid selection updates are ignored.

## Actions and the command palette

A command a person can run - a button, a menu row, a shortcut, an entry in the
command palette - is written once, as an `Action`, and everything that offers it
is handed that action rather than a label, a disabled flag and a callback of
its own:

```rust
let group = Action::new("canvas.group", "Group", move || state.run(Group))
    .glyph(ICON_COMPONENT)
    .shortcut(Chord::ctrl(Key::G))
    .enabled(can_group)
    .register();
view! {
    <Button icon_only=true action={group.clone()} />
    <MenuItem action={group} />
}
```

`register` adds it to the nearest `ActionScope` (or to the document's own,
outermost one) and removes it when the registering scope goes. `Button`,
`ToggleButton`, `ActionRow` and `MenuItem` take an `action` prop:
they show its label and glyph unless they are given their own, are disabled
while it is, show its first shortcut in their tooltip (a `MenuItem` beside its
label), and run it when pressed; a `ToggleButton` is pressed, and a `MenuItem`
ticked, while the action is `checked`. `.checked(signal)` makes a checkbox and
`.radio(signal)` one choice of a group (a `MenuItem` then has the radio role). An action's `run`
does nothing while it is disabled.

An action is a listener too. `action_scope(&node_ref)` makes a scope that
listens in the bubble phase at that node, so an action registered under it - an
editor's, or a text area's - only answers while the focus is inside the node,
and a key the focused control does not handle reaches the innermost scope
first. An action registered under no scope listens at the root. `.capture()`
moves an action, at the root or in a scope, to the capture phase, so it answers
before the focused control does. With the focus
on a node no scope holds, every scope is live; with nothing focused, every
scope in the topmost trapping layer is. A chord without Ctrl, Alt or Super (and
not on a media key) does not reach actions while the target types text or is
an app, so typing a letter into a field never switches a canvas tool.

`.at(KeyAt::Window)` moves an action above the tree: it hears what nothing on
the way up took, wherever the focus is, dialogs included, and the command
palette lists it wherever it opens. Since an app takes every key that reaches
it, such an action leaves an app's keys alone.

`.at(KeyAt::Window).capture()` is the opt-in to take a key away from a
focused app: the action is offered the press before anything on the tree and
before the app, and a press it takes never reaches the app, nor does its
release. Reserve it for shortcuts that must work over anything, such as
linux-desktop's Super+F and media keys. Inside an app only chords with Ctrl,
Alt or Super, and media keys, are offered, since the app counts as a text
field. A Wayland program's window is an app in the same way as a plugin's
region (see One input path below), so it gets this offer and no other;
modifier presses always reach it.

A forwarding catcher with `inhibits_shortcuts` set (an `Interactive` prop) is an
app that wants every chord itself, such as a remote desktop or a virtual machine:
while it has the focus, the window is cut from the path, so window listeners
and taps leave its keys alone and F6 does not move the focus, and all of them
reach the app. Only host listeners - `.at(KeyAt::Host).capture()` actions -
still answer, and only the host's own are ever offered an inhibited key:
block-app's intercept handler is a window listener, so it hands a plugin
nothing while it is set. The host is kept small and fixed: block-app's Super+L
lock and be-wayland's Super+Escape, which takes the shortcuts back from the
window (see guides/linux_desktop.md).

A document that runs inside a host - a plugin's - intercepts the same way. The
chords of its enabled capturing host and window actions (never one that types) are
`Document::intercepted_keys`, which reach the runner as
`WindowRequests::intercepted_keys`, handed to `Platform::show_window`; a host
offers the chords it was handed to its own window listeners ahead of the focused
app, and hands each press it takes, its repeats and its release back as an
`Event::InterceptedKey`, which the document offers to its host and window
capture listeners as a key bound for an app and which reaches nothing else. The host goes on sending the held
modifiers until those of the chord are let go, so `held_modifiers()` sees Alt
released while a Wayland program has the keyboard; that is how linux-desktop's
Alt+Tab works over any window. block-app takes intercepted keys only from the
shell's frame, and only chords with Ctrl, Alt or Super, or media keys. A tap
chord goes the same way as `Input::InterceptedTap`, but only while the
region does not have the keyboard itself, since it then hears the key and
counts the tap on its own; the release still reaches whatever had the focus.

A document with a layer open that traps focus
(`Layers::takes_keyboard`, such as a dialog, a popover or the launcher) reports `WindowRequests::wants_keyboard`, which the
runner hands to `Platform::show_window`. A plugin passes it on as
`WindowRequests::wants_keyboard` (guides/adding_a_plugin_editor.md), and block-app's region then takes the focus from
whatever had it (a program's window, another editor) and gives it back when
the overlay closes, so a dialog a plugin opens from an intercepted key can be
typed into.

`styled::CommandPalette` lists the actions that are live where the focus was
when it opened: typing filters them by every word, Up/Down/Page Up/Page Down
move the highlight over the enabled ones, Enter or a click runs the highlighted
one, and Escape closes it; disabled actions are listed last and greyed, and a
checked action, or the chosen one of a radio group, is ticked as in a menu. Closing
it puts the focus back where it was before the action runs. An editor frame
opens it with Ctrl+Shift+P and from a row of its menu. Handed `items={PaletteItems::list(..)}` instead, it
searches those `PaletteItem`s (each with a key, title, detail line, glyph or
picture, shortcut, checked and disabled) and calls `on_choose` with the chosen
key; `score` picks the ranking (`PaletteItem::substring_score` by default,
`PaletteItem::fuzzy_score` for names, keywords and comments, as linux-desktop's
program launcher uses). It and `Select` are stylings of `unstyled::SearchList`,
which owns the query, the highlight, the arrow and Page keys, scrolling to the
highlighted row, Enter and hover.

A menu is described once as `MenuNode`s: `MenuNode::action(&action)` (or
`labelled` to name it differently in this menu), `MenuNode::submenu(id, label,
glyph, items)` and `MenuNode::Separator`. `<MenuNodes nodes />` turns that into
`MenuItem` rows for a `ContextMenu`, a `MenuPopup` or a menu button, and
`add_to_menu(nodes)` adds a part to the document's own menu, `document_menu()`,
which an editor frame hands its host and the dock shows behind its More
button.

## One input path

Every embedded editor - a plugin's region, and in `block-app --session` and
`--desktop` each Wayland program's window - is a forwarding catcher: an
`Interactive` with `on_forward` (and `hit_at` for the points it takes),
handed what beui routes to it as a `ForwardedInput`: the pointer events beui
resolved for it (`Forwarded::Pointer`, with `over`, the catcher under the
event whatever holds the press, which a drag between Wayland windows needs),
`Forwarded::Left` once the pointer is neither over it nor held by a press in
it, `Forwarded::Focus` when it gains or loses the focus, and the keys, text,
IME and file drops meant for it (`Forwarded::Input`). The host knows nothing
of the editor behind it beyond turning those into its own protocol
(`plugin_host::forward_region` for a plugin, `be_wayland::Compositor` for a
window) and keeps no capture or hover of its own, so press capture, hover,
claims, overlays, focus, intercepting actions and the lock apply to both by
the same rules. For a surface that runs beui or takes its input,
`beui_core::interact::forward::Replay` turns a `ForwardedInput` back into the
raw `Input` a platform would have pushed: positions relative to the catcher
and divided by its zoom, modifier changes as their own `Input::Modifiers`, a
focus change only when it changes, the time each pointer sample happened, and
`Input::PointersCancelled` when a mouse press ends without its release (the
host's own pointers were cancelled), since the catcher's lost focus says
nothing about its pointers.
The plugin host fills its `InputBatch` with it, so a plugin's runner normalizes
the same input a window's runner does.

What one event goes through, in order:

1. The runner. `beui-adapter-drm` drops the input that wakes blanked screens,
   the rest of it for a grace period, and every release of a press it dropped
   (`wake.rs`, see guides/linux_desktop.md). That has to happen there: it is
   decided on libinput's own clock before the keymap sees the key, and nothing
   about it is the document's to judge. The keymap's own actions (switching
   terminals, Ctrl+Alt+Backspace) are the adapter's too. A key is one
   `Event::Key`: the `KeyPress`, the physical key as an evdev code (`evdev`,
   for Wayland programs, and on its own as `Key::Unidentified` for a key beui
   has no name for) and the text it typed. `Event::Text` is text that came without a key, and
   `Event::Paste` is a paste.
2. `InputState::begin_frame` turns the frame's events into `PointerEvent`s:
   moves, presses, releases, cancels, wheel turns and zooms, each where the
   pointer was at that moment. A mouse event that lands on a finger is the
   platform's copy of the touch and is dropped.
3. `interact`: the document counts the frame as input (`Document::input_frames`,
   which the compositor's idle time reads), and for each pointer event hit
   tests it over the layers once, asks the layers it is outside of to close,
   finds a modifier claim (`claim_modifiers`, `claim_at`) or a capture for a
   press, hands it to the pointer's session (one per pointer and button, which
   also counts the fingers for pinches and two-finger gestures), and moves the
   focus.
4. Keys, one at a time along the focus path (How a key travels, above): host
   and window listeners first (capturing actions, and in block-app the shell
   plugin's intercepted chords, `compositor/intercept.rs`), then F6 and
   Shift+F6, which move the focus between forwarding catchers. A key one of
   them takes is gone, physical key and text with it, and so are its repeats and
   release when they come. A key bound for an app goes no further; any other
   key goes on down the tree and back up, and types its text unless something
   on the way took it.
5. `forward::route`: the pointer goes to the catcher under it (`hit_at`
   says no where something else is drawn over it), a press stays with the
   catcher its pointer's down reached until every button is let go, a claimed
   press goes to its claimant, wheel and drop events to the catcher under the pointer, and
   keys, text and IME to the focused catcher. A catcher beneath a layer that
   blocks everything loses its capture.

So no editor reads the frame's events itself, and nothing gives a key a
meaning outside an action, a shortcut or a control. A host that shows several
documents in one frame, as the inspector does beside the app, gives each the
input it may see through `Context::with_input`: `InputState::only` keeps some
events, `without_pointer` takes the pointer away (telling the document the
pointer left, the frame it starts), and a `Focus(false)` before and
`Focus(true)` after the time the document's keys go elsewhere, the same as a
window losing the keyboard, which also stops it claiming the IME.

## Control props

Every control is a `#[component]`, so it is written as a tag inside `view!` and
driven by reactive props rather than by setter calls. `<RadioGroup>` and
`<Listbox>` take `labels` and an `Option<usize>` `selected` prop and report
changes through `on_change`; `None` clears the selection, and the first option
becomes the entry point. `<ToggleButton>` takes `label` and `pressed`, and its
label stays stable as the pressed state changes. The demo's Choices and Buttons
pages show all three.

`<Tree>` takes the `keys` of the rows that are visible in tree order, an `item`
callback that answers with the `TreeItem` (label, depth, whether it can expand,
whether it is expanded) for one key, and the `selected` key, which may be a
row hidden inside a collapsed one. It reports `on_select`, `on_expand`,
`on_reveal` and `on_hover_change`, and its children build the cells of one
row. The tree owns
the roving Tab stop, the marker, the indent and the keyboard model; the caller
owns the rows themselves, so a tree of anything keyed by anything hashable
works. The demo's Tree page and the beui inspector both use it.

`<Link>` takes a `label`, an optional `glyph` for an icon beside it, and
`on_click`. It reads as a link rather than a button, underlines itself while
hovered or focused, and goes muted and unclickable while `disabled`. It is the
control for "take me to that thing", which is why
`block_editor_beui::BlockLink` is one: given the `Editor` and an
`Option<ChildTarget>`, it follows the block's name and icon and opens it in the
host when it is clicked, showing its `fallback` while the reference has not
resolved.

`<Select>` takes `options` and `selected` and opens a popup with a search box
over the option list. Its options, and those of `<Tabs>`, `<Listbox>`,
`<RadioGroup>` and `<ResponsiveTabs>`, are a `Children<unstyled::ChoiceOption>`
written as `<ChoiceOption>` tags, with each label a prop that follows its
signal. A list that comes from runtime data builds a `Vec` of those tags rather
than a `Vec` of labels, so an option is one thing everywhere it is written.
`<ContextMenu>` wraps a `region` so a secondary click
opens a menu built from the `items` prop, a `Children<unstyled::MenuItem>`
written as `<MenuItem>` tags with a submenu's items between its own tags; its
`on_select` callback receives the selected item's index path through any
submenus. Each item's `label` and `disabled` are ordinary props, so a row
follows the signals it was given without the menu being rebuilt. Both controls
sit on the `base`
overlay element: an anchored, viewport-relative popup painted above the rest of
the tree that traps Tab while open. The demo's Choices and Menus pages show them.

Reading a control's state back out, rather than owning the signal that drives
it, is for tests and host integration: each control's typed handle
(`SelectApi`, `ToggleApi`, `SliderApi`, ...) comes from its `api` prop, or from
`document.api::<SelectApi>(node)` given its node.
`focus_*` takes just the node and focuses the control ambiently.

## Composing controls and embedding beui

The unstyled controls hand their interaction state to whoever renders their
content: `<unstyled::Button>|handle| <Face handle /></unstyled::Button>` builds its
face with a `ButtonHandle` carrying `hovered`, `active`, `focused` and `disabled` signals, and the
text input and select equivalents do the same. Build reactive props out of
those signals — including effects that react to a child's hover or focus —
instead of querying the control's state back afterwards.

`tab_stop` is a prop on `<unstyled::Button>` and a focusable `<Interactive>`: set it to
`false` to keep a control reachable by pointer and programmatic focus while
removing it from sequential Tab navigation, which is how single-Tab-stop groups
work. `yields_tab_stop` on a focusable `<Interactive>` makes
it a Tab stop only while nothing inside it is one, decided from the tree each
time Tab walks it; that is how a scroll area hands its stop to its content,
with its `tab_stop` following whether it overflows. Neither is set on a scroll
area from outside.

`press_focus=false` is the one way to make pressing something leave the focus,
and with it a text field's caret and selection, where it is. It is a prop on any
`<Interactive>`, focusable or not, and on the controls built on one (`Button`,
`Toggle`, `Checkbox`), and it covers everything inside it: a press
anywhere in it focuses nothing, whatever focusable is under the pointer. Put it
on a whole toolbar rather than on each of its buttons, as the text editor's
format bar and the dock's bars do, so its padding and scroll strip keep the
focus too. `on_key` returns `true` only for keys it handled; a control that needs
keys before a text input inside it sees them listens at it (How a key travels,
above). `Document::focused_node` reports the current focus and `Document::watch_focused` signals it; `holds_focus(node_ref)` is a memo of whether the focus is in that node, for something that shows while the focus is in it.

Use `@node_ref=&a_node_ref` on any tag when an enclosing component needs the
`NodeId` of something nested inside its tree; `NodeRef::get` reads it back once
the tree is built.

A host sends `Input::Focus(false)` when its window or editor region loses the keyboard. It means only the keyboard: the runner lets go of the modifiers and beui of the keys it holds, since their releases go elsewhere, but a press goes on, as a host still forwards it to the region it began on. A press, touch or pen whose release will not come is ended by `Input::PointersCancelled` instead (guides/beui.md, the runner). Text arrives through `Input::Text`, and a paste through `Input::Paste`. An input method's edits arrive as `Input::Ime`, a few operations every platform's input method maps onto: set the composing text, commit text, finish composing, mark existing text as the composition, replace a range (an autocorrect suggestion), delete around the selection, and set the selection, with ranges in UTF-8 bytes of the field's text. A focusable's `on_ime` receives them; one without it gets committed and finished compositions through `on_text`. A text area hands them to text-editor-core as `EditorCommand::Ime`, which writes the composition into the document and tracks its range (`Core::composition`, drawn underlined); any other command ends it. The text area leaves keys to the input method while a composition lasts, and reports its caret through `Interactive`'s `ime_cursor` and the text around it through `ime_text`, so `WindowRequests::ime` places the candidate window beside the caret and the runner can show the input method the field's real text (`beui_core::app::ime_mirror::ImeMirror` keeps a platform text field such as the web's hidden textarea in step with it and turns its edits back into these operations). `ImeArea::keyboard` says whether the runner should show an on-screen keyboard: `Keyboard::Held` for a focusable with `keyboard_on_focus=false` (a select's search box) focused while the last press was a finger's or a pen's (`Document::touched`), which takes text from a hardware keyboard as soon as it is focused but holds the on-screen keyboard back until it is tapped or a key other than a media key is pressed, and `Keyboard::Shown { asks }` otherwise: a mouse press or a key is taken as a sign of a hardware keyboard, so the field asks for an input method at once. `asks` is the document's count of taps on a focused field that wants the keyboard (any press on it, from a mouse), so a tap changes the window state even though nothing else about the field did, and a user who closed the on-screen keyboard gets it back by tapping the field: The winit adapter turns on the platform's input method (Wayland's text input, which a compositor's keyboard such as sway's opens for) only while the keyboard is `Shown`, so a held field composes nothing through an input method until it is tapped or typed into, and the key that ends the hold reaches it before the input method is on. Android opens its keyboard whenever the keyboard turns `Shown` or `asks` changes, as beui's own on-screen keyboard does while no keyboard is attached (guides/beui.md, The on-screen keyboard), and the web adapter refocuses its hidden textarea within the press itself, since a browser opens its keyboard only during the user's gesture. A plugin's region passes its editor's answer on whole: `ime_keyboard` for whether it is held, and `Interactive`'s `keyboard_asks` for the editor's count, which a focusable shows in place of the document's and keeps its own taps from counting, so a tap on the plugin asks only when the editor's own document decided it should. Copy and cut return text in `FrameOutput::copied_text`; the host writes this to its clipboard. Both the desktop runner and the block editor integration handle these outputs. Clipboard access for other custom hosts belongs to their platform integration.

Keyboard regression tests run without a window. `./scripts/test //crates/beui:test` runs the tests that drive whole documents, each `beui-*` crate has its own `:test`, and `./scripts/verify` runs the whole workspace's.

## Touch behavior

Beui accepts `Input::Touch` with a stable device and finger id, lifecycle
phase, logical position, and optional normalized pressure. It tracks every
active contact in `InputState::touch`; the first contact drives the primary
pointer so existing pressable controls, sliders, text selection, focus, and
overlays work without a separate touch-only control API. A second contact or
a cancelled contact cancels a pending tap rather than activating it.

A second finger that lands while the first is still held where it touched
down is a secondary drag rather than a pinch: it reports through
`Interactive`'s `on_secondary_drag` exactly as a right-button drag does
(`SecondaryDrag`: where it began, where it is, whether it started, ended or was
cancelled, and the modifiers), which is how a board marks up arrows by either
means. The moment the held finger moves, the gesture is a pinch after all and
the secondary drag is cancelled.

A tap may drift by up to eight logical points. Beyond that threshold beui
locks the gesture to its dominant axis. Vertical gestures drag the deepest
scroll view under the initial contact, keep that scroll captured when the
finger leaves its rectangle, and do not click the row where the gesture
started. A released vertical drag coasts with its sampled velocity, and a drag
beyond either end of a scroll view is resisted before springing back. Touch
contacts do not hover controls, and focus is assigned only after a gesture
resolves as a tap. Horizontal gestures remain available to controls such as
sliders. The touch pointer disappears after release, so touch does not leave
hover styling behind. Platforms may send a mouse event of their own for each
touch; beui drops a mouse event that lands on a finger, so a real mouse
elsewhere goes on working while a finger is down.

Standalone apps receive winit touch events automatically. Open the inspector
with Ctrl+Shift+I and enable “Emulate touch with mouse” to turn the primary
mouse button into a touch contact. Embedded beui plugins receive the same touch
data over the block plugin input protocol. `block_ui_test::BeuiTest` provides
`tap`, and `fingers` (as beui's `testing::Harness` does) for a gesture of one
finger or more, and `secondary_drag` for a right-button drag.
