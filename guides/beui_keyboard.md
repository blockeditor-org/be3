# beui controls and keyboard behavior

The styled controls follow the keyboard conventions in the [W3C Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/). They paint focus outlines, and pointer selection and keyboard selection share the same state and change callbacks.

| Control | Keyboard behavior |
| --- | --- |
| Buttons, links, toggle buttons, checkboxes, switches, list rows, disclosure and accordion headers | Space or Enter activates on release. Escape or focus loss cancels a held activation. Repeated key-down events do not activate repeatedly. |
| Tabs | One Tab stop, at the selected tab. Left/Right wrap and select immediately. Home/End select the first/last tab. Up/Down leave the horizontal tab selection alone. |
| Radio groups | One Tab stop, at the selected option or the first option when unselected. Arrows wrap and select; Space selects without clearing an existing selection. Home/End select the first/last option. |
| Single-select listboxes | One Tab stop. Up/Down select the previous/next option and stop at the ends. Home/End select the first/last option. Typing searches case-insensitive prefixes; repeated letters cycle matches. The search resets after one second or when focus leaves. |
| Sliders | Right/Up increase and Left/Down decrease by 5% of the track. Home/End select minimum/maximum. Page Up/Down adjust by 20% of the track. Values remain within the range, which is `min` to `max` and defaults to 0 to 1. A curved `scale` keeps the steps even along the track, so they are small where the track is fine and large where it is coarse. |
| Text inputs | Left/Right, Home/End, Shift-selection, Ctrl/Alt word navigation and deletion, Ctrl+A, Ctrl+C/X, Ctrl+Z, Ctrl+Shift+Z/Ctrl+Y, and Enter to submit. Up and Down move to the start and the end of the text, as they would on the one line of a multiline area. Space inserts text. Paste replaces the selection. A secondary click opens the Copy, Cut, Paste and Select All menu that a touch tap on the selection or the caret handle opens. The desktop runner maps Command to Ctrl on macOS; Super chords type nothing. A multiline text area takes Tab and Shift+Tab to indent; Escape then Tab or Shift+Tab moves the focus out of it instead. It registers Find (Ctrl+F), Find and replace (Ctrl+H), the next and previous match (Ctrl+G, Ctrl+Shift+G), Duplicate the line (Ctrl+Shift+D), Select the next occurrence (Ctrl+D) and folding (Ctrl+Shift+[) as actions, which answer while the focus is in it. |
| Selectable text | `<SelectableText>` makes the plain text nodes under it selectable together: a mouse drag selects from one text to another in tree order, Shift+click extends the selection, and a click clears it. Ctrl+C copies the selection, joining texts on one line with a space and lines with a line break; Ctrl+A selects everything under it. A secondary click opens a Copy and Select All menu. It is not a Tab stop, and the keys also reach it from a focused control inside it. |
| Scroll areas | Tab focuses the area. Up/Down scroll by a line; Page Up/Down and Space/Shift+Space scroll by a page; Home/End reach the endpoints. Tabbing to a child or navigating a choice scrolls it into view. Unused Up/Down, Home/End, and Page keys on child controls scroll the nearest containing area. Virtual lists can be paged before tabbing into their realized controls. |
| Select (dropdown) | Clicking or activating the trigger opens the popup and focuses its search box; typing filters the options by case-insensitive substring. Up/Down/Home/End on the closed trigger also open the popup and move the highlight in that direction. Up/Down move the highlighted option without moving the text caret; Home/End jump to the first/last visible option. Enter confirms the highlighted option and closes the popup. Escape or an outside click closes the popup without changing the selection and returns focus to the trigger. |
| Tree views | One Tab stop, at the selected row or the first row. Up/Down move to the previous/next visible row and stop at the ends; Home/End reach the first/last row. Right expands a collapsed row and then moves to its first child; Left collapses an expanded row and then moves to its parent. Space or Enter selects a row, the same as clicking it, and leaves it open or closed; only Left, Right and the chevron expand or collapse. Typing searches case-insensitive prefixes over the visible rows. Selection follows the focused row. |
| Pan and zoom areas | Tab focuses the area. Arrows pan by a step; `+` and `-` zoom around the middle of the viewport and `0` returns the scale to one. The area owns the keys it uses, so arrows pan it rather than scrolling whatever contains it. |
| Dock | One Tab stop per tab bar, at the tab the pane is showing, walked like any other tab list; the bar scrolls the tab that takes focus into view. The bar between two panes is a Tab stop with a `Splitter` role that the arrows move. Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of the pane the focus is in, wherever the focus is inside it. |
| Calendar | One Tab stop in the grid, at the focused day. Left/Right move a day and Up/Down a week, crossing into the next or previous month. Home/End reach the start/end of the week. Page Up/Down move a month and Shift+Page Up/Down a year, keeping the day within the month. Space or Enter picks the day. Days outside `min` and `max` are skipped. The title's month button switches to a grid of months, where the arrows move by a month and a row, Page Up/Down by a year, and Enter shows that month's days; its year button switches to a grid of twenty years, where the arrows move by a year and a row, Page Up/Down by twenty years, and Enter shows that year's months. |
| Date and time fields | Each segment is a Tab stop and a spin button. Up/Down step it and wrap; Page Up/Down step by a larger amount; Home/End reach its first/last value. Left/Right move between segments and stop at the first and last. Alt+Down opens the calendar over the field with the focus kept in the segment. Digits type into the segment and move on once it is full or no further digit could fit; `-`, `/`, `:` and space move on too, and `a`/`p` set AM/PM. Backspace or Delete clears a segment, and Backspace on an empty one goes back a segment. The button beside the segments opens the calendar and time list; Escape closes them and returns the focus to it. |
| Time lists | One Tab stop, at the selected time. Up/Down move a time, Page Up/Down an hour, Home/End the first/last. Laid out as a grid, Left/Right move a time, Up/Down a row and Page Up/Down four rows. Space or Enter picks the time. |
| Color area | Tab focuses the area. Left/Right change the saturation and Up/Down the brightness by 1%, or 10% with Shift; Page Up/Down change the brightness by 10%; Home/End reach no saturation and full saturation. Hue and opacity are sliders. |
| Context menu | Secondary click opens the menu at the pointer and focuses its first item, which is shown with a highlighted background; Tab is trapped on the menu's single roving Tab stop while it is open. Up/Down move between items and update the highlight; Home/End jump to the first/last item. Right Arrow (or hovering an item) opens its submenu and focuses its first item; Left Arrow closes a submenu and refocuses the item that opened it. Only one submenu per level stays open. Enter or clicking a leaf item selects it and closes the entire menu stack; Escape closes one level at a time; an outside click closes the whole stack. |
| Keep changes prompt | Opens with the focus on Keep, so Enter keeps; Escape reverts, as running out its countdown does. |

The inspector panel is a document of its own, so Ctrl+Shift+F moves focus into
it and back, and Escape inside it returns focus to the inspected document.

A chord that belongs to a whole region rather than to whatever has the focus is
registered with `on_shortcut`, which is offered every key press before the
focused control sees it, and answers `true` for the ones it takes. Shortcuts
are consulted only while no menu or dialog is open, since those take the
document over. That is how the dock's Ctrl+Tab reaches it from inside a text
input.

`on_global_key` goes further: it is offered every key press and release
before anything else, including while a menu or dialog is open. It is handed a
`GlobalKeyPress`, whose `in_app` says the focus is in an app (a plugin editor's
region, or a Wayland program in a desktop session) that would otherwise get
the key. A handler takes such a key only when it means to intercept it from
the app. A press it answers `true` for, and that key's release, go nowhere
else.
`held_modifiers()` is a signal of the modifiers held now, which changes on
modifier presses alone. Shortcuts that are not chords, such as a hold-and-release
switcher, are built from those two.

The modifier keys themselves arrive as `Key::Shift`, `Key::Ctrl`, `Key::Alt` and
`Key::Logo` presses and releases (`Key::is_modifier`), alongside the
`Event::Modifiers` they cause. They reach `on_global_key` and forwarded regions
(plugins and Wayland programs), but not the focused control's `on_key`, shortcuts or
actions, which read the state from `KeyPress::modifiers` or `held_modifiers()`; nor
do they show the focus ring or end autoscroll, and the runners drop their repeats.
Something that reacts to the modifier key rather
than to the state, such as tapping Super alone (block-app's `SuperTap`), reads
the key events: the runners do not agree on whether `Event::Modifiers` comes
before or after the key's own event, and winit on X11 can report a modifier
state that reverts before the key arrives.

`Modifiers` and `Chord` have `logo` for the Super (Windows) key. The runners
report Super as `logo`, except on macOS, where Command acts as Ctrl and Super
is never reported. `Key` also has the volume, mic mute, brightness and media
transport keys, mapped wherever the platform reports them.

Tab and Shift+Tab traverse visible controls in tree order and wrap within the document. Hidden panels and collapsed content are excluded. Changing a selection programmatically updates the group's Tab stop and moves focus with the selection when the group already contains focus. Programmatic changes do not pull focus from other controls. Empty groups have no Tab stop, and invalid selection updates are ignored.

## Actions and the command palette

A command a person can run - a button, a menu row, a shortcut, an entry in the
command palette - is written once, as an `Action`, and everything that offers it
is handed that action rather than a label, a disabled flag and a callback of
its own:

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

`register` adds it to the nearest `ActionScope` (or to the document's own,
outermost one) and removes it when the registering scope goes. `Button`,
`IconButton`, `ToggleButton`, `ActionRow` and `MenuItem` take an `action` prop:
they show its label and glyph unless they are given their own, are disabled
while it is, show its first shortcut in their tooltip, and run it when pressed;
a `ToggleButton` is pressed while the action is `checked`. An action's `run`
does nothing while it is disabled.

`action_scope(&node_ref)` makes a scope that is live while the focus is inside
the node, so an action registered under it - an editor's, or a text area's -
only answers while the focus is there. A key press the focused control does not
handle (its `on_key` answers `false`) goes to the live actions, innermost scope
first, and the first enabled one with a matching `Chord` runs. With nothing
focused, every scope is live. A chord without Ctrl, Alt or Super (and not on a
media key) does not reach actions while the focus is in a text field, so typing
a letter into a field never switches a canvas tool. Actions are consulted only
while no menu or dialog is open.

An action built with `.global()` is consulted through `on_global_key` instead:
before the focused control, wherever it was registered, and while menus and
dialogs are open. The command palette lists global actions wherever it opens.
A global action still leaves an app's keys alone: while a plugin editor or a
Wayland program has the focus, the key goes to it and the action does not run.

`.intercepts()` (which implies `.global()`) is the opt-in to take a key away
from a focused app: the action is offered the press before the app, and a
press it takes never reaches the app, nor does its release. Reserve it for
shortcuts that must work over anything, such as Super+F
(`be_wayland::toggle_fullscreen_action`) and the media keys. Inside an app only
chords with Ctrl, Alt or Super, and media keys, are offered, since the app
counts as a text field. In `block-app --session`, `be_wayland::Compositor`
makes that offer with `Document::offer_app_key` before forwarding each press
to the focused program; modifier presses always reach the program. A program
that inhibits shortcuts (keyboard-shortcuts-inhibit) is a matter of the
compositor not making the offer.

`styled::CommandPalette` lists the actions that are live where the focus was
when it opened: typing filters them by every word, Up/Down/Page Up/Page Down
move the highlight over the enabled ones, Enter or a click runs the highlighted
one, and Escape closes it; disabled actions are listed last and greyed. Closing
it puts the focus back where it was before the action runs. An editor frame
opens it with Ctrl+Shift+P and from a row of its menu, and
`menu_actions()` (every action registered with
`in_menu()`) is what fills that menu, which the dock shows behind its More
button.

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
it, is for tests and host integration: `*_selected`, `*_open`, `*_pressed`,
`slider_value`, `text_input_value` and friends take `&Document` and a node.
`focus_*` takes just the node and focuses the control ambiently.

## Composing controls and embedding beui

The unstyled controls hand their interaction state to whoever renders their
content: `<unstyled::Button content={...}>` calls the content builder with a
`ButtonHandle` carrying `hovered`, `active`, `focused` and `disabled` signals, and the
text input and select equivalents do the same. Build reactive props out of
those signals — including effects that react to a child's hover or focus —
instead of querying the control's state back afterwards.

`tab_stop` is a prop on `<unstyled::Button>` and a focusable `<Interactive>`: set it to
`false` to keep a control reachable by pointer and programmatic focus while
removing it from sequential Tab navigation, which is how single-Tab-stop groups
work. `on_key` returns `true` only for keys it handled. `<unstyled::TextInput>`
also takes `on_key_override`, which lets a compound control (such as select's
search box) intercept arrows before the text input's own key handling runs;
returning `false` falls through to the normal behavior. `Document::focused_node`
reports the current focus.

Use `@node_ref=&a_node_ref` on any tag when an enclosing component needs the
`NodeId` of something nested inside its tree; `NodeRef::get` reads it back once
the tree is built.

A host sends `Event::Focus(false)` when its window or editor region loses focus. Text and paste arrive through `Event::Text`. An input method's edits arrive as `Event::Ime`, a few operations every platform's input method maps onto: set the composing text, commit text, finish composing, mark existing text as the composition, replace a range (an autocorrect suggestion), delete around the selection, and set the selection, with ranges in UTF-8 bytes of the field's text. A focusable's `on_ime` receives them; one without it gets committed and finished compositions through `on_text`. A text area hands them to text-editor-core as `EditorCommand::Ime`, which writes the composition into the document and tracks its range (`Core::composition`, drawn underlined); any other command ends it. The text area leaves keys to the input method while a composition lasts, and reports its caret through `Interactive`'s `ime_cursor` and the text around it through `ime_text`, so `FrameOutput::ime` places the candidate window beside the caret and the runner can show the input method the field's real text (`beui_core::app::ime_mirror::ImeMirror` keeps a platform text field such as the web's hidden textarea in step with it and turns its edits back into these operations). `ImeArea::keyboard` says whether the runner should show an on-screen keyboard: a focusable with `keyboard_on_focus=false` (a select's search box) takes text from a hardware keyboard as soon as it is focused but holds the on-screen keyboard back until it is tapped, and a plugin's region passes its editor's answer on through `ime_keyboard`. Copy and cut return text in `FrameOutput::copied_text`; the host writes this to its clipboard. Both the desktop runner and the block editor integration handle these outputs. Clipboard access for other custom hosts belongs to their platform integration.

Keyboard regression tests run without a window. `./scripts/buck test //crates/beui:test` runs the tests that drive whole documents, each `beui-*` crate has its own `:test`, and `./scripts/verify` runs the whole workspace's.

## Touch behavior

Beui accepts `Event::Touch` with a stable device and finger id, lifecycle
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
hover styling behind. Platform integrations may provide synthesized pointer
events alongside touch events; beui suppresses those duplicates while the
touch is active.

Standalone apps receive winit touch events automatically. Open the inspector
with Ctrl+Shift+I and enable “Emulate touch with mouse” to turn the primary
mouse button into a touch contact. Embedded beui plugins receive the same touch
data over the block plugin input protocol. `block_ui_test::BeuiTest` provides
`touch_start`, `touch_move`, `touch_end`, and `touch_cancel` for headless
gesture tests, `finger` for a gesture of more than one finger, and
`secondary_drag` for a right-button drag.
