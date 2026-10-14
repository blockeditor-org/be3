# beui components

A catalog of what beui already offers, by need: controls, scrolling and long lists, overlays and
focus, docking, drag and drop, pan and zoom, files, pointer lock and custom drawing. Read the
section for the thing you are about to build before building it. `guides/beui/overview.md` explains
the base, unstyled and styled layers referred to here; `crates/beui-demo` shows each component
running.

## What exists

The re-exports in `crates/beui-components-unstyled/src/lib.rs` (`beui::unstyled`) and
`crates/beui-components-styled/src/lib.rs` (`beui::styled`) are the authoritative lists.

- **Base** (`beui::reactive`): `Frame`, `List`, `Layers`, `Grid`, `Text`, `Offset`, `VirtualList`,
  `Canvas`, `Drawing`, `Overlay`, `Interactive`, `Embed`, `Portal`, `BackHandler`, `Shift`, `Fade`;
  layout ones are in `guides/beui/views.md`.
- **Unstyled**: `Button`, `Pressable`, `Toggle`, `Choice`, `Slider`, `TextInput`, `TextArea`,
  `Disclosure`, `Tree`, `Select`, `ContextMenu`, `MenuButton`, `Popover`, `Container`, `PanZoom`,
  `PointerLock`, `Dock`, `Draggable`, `DropTarget`, `Tooltip`, `Floating`, `Scroll`, `Scrollbar`,
  `Stack`, `Calendar`, `DateTimeField`, `TimeList`, `ColorArea`, `ColorWheel`, `Picture` (a
  `Drawing` with a size), and more.
- **Styled**: themed buttons, icon buttons, menu buttons, links, text styles, cards, checkboxes,
  switches, choices, text and number inputs, a multiline text editor with a find and replace bar,
  menus, popovers, split buttons, tabs, trees, a calendar, date and time fields, a color picker and
  color input, progress, scrolls and scrollbars, tooltips, a docking workspace, responsive layout,
  and `Separator`.

A control that can be turned off (`Button`, `IconButton`, `Link`, `Checkbox`, `Select`,
`TextInput`, `NumberInput`) takes a `disabled` prop: it stops answering the pointer and keyboard,
leaves the tab order, publishes itself as disabled to a screen reader and paints in muted colours.
A read-only editor binds `editor.read_only()` to it rather than leaving a live control that quietly
throws edits away.

## Buttons and presses

A control that has to win a press from a surface around it (an add button in a row that is itself
a button, a close cross on a tab) asks `unstyled::Button` for `capture_presses`. The press stops at
that button, so the row it sits in does not open as well.

A press made with modifiers held can be claimed from everything inside a region, which is how a
window manager takes Super+drag from the windows it holds:

- An `Interactive` given `claim_modifiers` takes every primary or secondary press in its rectangle
  while all of those modifiers are held; `claim_at` decides the same per point.
- The claim is searched from the top of the tree down, before `capture_presses`, so the outermost
  claimant wins. The press, its drag and its release reach the claimant alone: none of its
  descendants, and no forwarding catcher under it unless the claimant is that catcher. A Wayland
  program's window is a forwarding catcher like a plugin's region, so a press claimed over it
  never reaches the program.
- `Document::press_claims` lists the `claim_modifiers` rectangles laid out this frame, which a
  plugin reports to its host.

`on_press_outside` hears every primary press outside the `Interactive`'s visible rectangle,
wherever it goes and whatever takes it, at the same moment and from the same place a press outside
asks layers to close (a mouse's press, a finger's tap; `Document::pressed_outside` does both). A
long press counts as a secondary press, which asks the layers but not these. This is how the host
hands an embedded editor's active child back when someone clicks elsewhere.

A secondary drag stays with the node that took it while the pointer crosses a floating window above
that node; only the start of one is hidden from what a floating window covers.

## Text fields

`unstyled::TextArea` is the multiline editor:

- It owns a `text_editor_core::Core` through the `TextAreaState` its caller holds (a `Copy` handle
  owned by the scope that made it; `with_core` and `with_core_mut` reach the `Core`).
- It lays the document out with a gutter, wrapping, collapsible sections and markdown checkboxes,
  and lays out the inline and block `TextWidget`s the caller names: `block` builds what goes under
  a block widget's line, and `selected_widget` what floats under an inline one while it is
  selected. That is how a block editor embeds a block in the text and drives the same document from
  its own toolbar.
- A `completer` names a trigger character and a search: typing it after a space opens the menu
  `completion_menu` draws under the caret, filtered by what follows, with arrows, Enter and Escape.
  `styled::TextArea` uses it for emoji (`:rocket`). The menu is a passing layer with an
  `on_dismiss`, so Escape, back and a press outside all close it the same way.
- The styled find bar holds a `BackHandler` while open, so back closes it as Escape does.
- The state republishes what it shows whenever one of its own commands runs. Code that changes the
  document behind it (adopting an edit from someone else) calls `sync()`, or `external_edit()` when
  the edit should also break the undo group: nothing polls the document for changes.
- `placeholder` shows while the document is empty; `password` masks every character; a touch tap
  leaves a caret handle that drags the caret and opens its menu.
- `single_line` lays the same control out on one unwrapped line with no gutter, scrolled sideways
  to keep the caret in view, where Enter submits and Tab leaves. `frame` wraps the field in the
  caller's chrome inside the area's own focus and pointer handling.

`unstyled::TextInput` is that single-line mode over a plain-text buffer it owns, driven by a
`value` and reporting `on_change`, so a fix to how text is edited lands in both.

How `TextArea` is built from base `Text` is in `guides/beui/writing_components.md`, "Text".

## Menus

A menu is never asked for as a dropdown or as a sheet: every menu opens as a dropdown when a mouse
opened it and as the same items in a `ModalSheet` when a finger did, so both show the same
features. A `MenuStyle` carries both looks (the styled `menu_style()`); an unstyled one without a
sheet stays a dropdown.

- `MenuButton` opens a menu under itself: what a toolbar uses where `Select` would imply the choice
  sticks. `IconMenuButton` is the same with an icon button's face.
- `ContextMenu` is the same menu on a secondary press. It also takes an `open_at` point, so a touch
  gesture can raise it where the finger was, or `open_at_pointer` to open where the last pointer
  was, reading `Document::last_pointer` to know whether that was a finger, for a menu something
  else asks for (the app menu a plugin's button opens).
- A text selection's menu sets `selection` and stays beside the selection under a finger too.
- A finger held still for the long-press delay is a secondary press, so every context menu opens on
  tap-and-hold (`guides/beui/writing_components.md`, "Touch").

Menu rows are written as tags of `unstyled::MenuItem`, a component that returns a value rather than
a node (`guides/beui/writing_components.md`, "Components that return a value").

## Trees

`unstyled::Tree` puts each row in the tab order with the tree's keyboard and its `TreeItem`
accessibility, and nothing more. Where a pointer has to land to select a row is the face's
business, because a face that draws its own chevron outside the name wants pressing the chevron,
the indent and the name to mean three different things. The handle carries `select`, `toggle` and
`hover` for the face to call wherever it decides. The arrow keys, Home, End and typing move the
keyboard between rows without selecting them; Enter or Space selects the row it is on. `toggle`
puts the keyboard on its row too.

`styled::Tree` is the face app code uses:

- It draws the indent, a chevron that is a button of its own (tooltipped, labelled for a screen
  reader, outside the row's highlight, so pressing it expands the row without opening it), and the
  highlighted row, and hands its content builder a `TreeRowFace` of the row's `key`, `item`,
  `selected`, `focused` and `hovered`.
- It owns the drag gesture: a press that travels more than a few pixels reports `on_drag_start` and
  the click it would have become is dropped, so a row can be dragged out without selecting it.
- `row_test_id` names each row for tests, as `<id>.row` and `<id>.chevron`. `outline` gives one row
  an outline, which is how a drop target says whether it will take what is over it.
- Choosing a row never expands, collapses or scrolls anything; only the chevron and arrow keys do.
- It owns its `Scroll` (`padding` goes inside it) and follows the `selected` key. Given
  `ancestors` (the keys above one, outermost first), it marks the chevron of the deepest row still
  shown above a selection hidden inside a collapsed one. While the selection is hidden or scrolled
  out of view it floats a button over the top or bottom edge that reports `on_reveal` (the caller
  expands the ancestors) and then scrolls the row into view once laid out. The file tree and the
  beui inspector both work this way.
- `styled::tree_row_node` and `styled::tree_focused` reach the rows through the tree's node.

## Scrolling

Scrolling exists at all three layers; app code wants the styled one. `styled::Scroll` is a scroll
with the project's scrollbar beside it, so a scrolling panel is one tag:

```rust
view! {
    <Scroll @sizing=ItemSize::Percent(100.0)>
        <Rows />
    </Scroll>
}
```

- `direction` makes the same tag a column of rows or a strip of cards. A horizontal one answers
  Shift+wheel, a sideways trackpad swipe, a touch drag and the left and right arrows; a plain wheel
  is left to whatever is around it, the way a browser leaves a horizontal strip alone. A wheel only
  reaches the innermost scroll under the pointer.
- The focus ring follows the theme's accent unless `focus_color` names another.
- `on_change` reports the position.
- With `max_length` the scroll is as long as its content up to that length, and scrolls from there
  on: what a popup listing a few rows or many wants. A `Frame` with `max_height` around a scroll
  does not do this, because a `Frame` with a maximum fills its offered space up to it.

### The unstyled scroll and its bar

`unstyled::Scroll` is the same arrangement without the appearance. It puts its children in a `List`
inside a base `Offset`, wraps that in a focusable `Interactive` for the keys, the wheel and the
touch drag, keeps the momentum an unfinished fling carries, drives the offset from all three, and
reports the position. It takes a `ScrollbarStyle` saying what to put on its cross axis: a spacing
and a builder handed a `ScrollHandle` of the live `position`, the `direction`, and a `scroll_to`
that drives the offset as the wheel and arrow keys do:

```rust
ScrollbarStyle::new(SCROLLBAR_SPACING, |handle: ScrollHandle| {
    let ScrollHandle { position, direction, scroll_to } = handle;
    view! {
        <Scrollbar
            @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH)
            position
            direction
            on_scroll_to={move |offset: f32| scroll_to.call(offset)}
        />
    }
})
```

Without a style the scroll shows no bar. A control that scrolls something of its own takes a
`ScrollbarStyle` and passes it down (as `unstyled::Select` does to the scroll behind its options),
so the styled control decides the bar and the unstyled one never names a colour. The gutter is
always reserved and the bar paints nothing while the content fits, so a scroll that grows past its
viewport does not shift the content beside it.

`unstyled::Scrollbar` answers the pointer itself: it takes the position, direction and an
`on_scroll_to`, hit-tests a press against the thumb it would paint, drags the thumb, pages by a
viewport towards a press on the track either side, and hands its content a `ScrollbarHandle` of
`hovered` and `dragging`. A press on the bar stops any fling momentum.

- `thumb_start` and `thumb_length` are the fractions both layers work in, so what is painted and
  what is pressed cannot drift apart.
- `ScrollbarThumb` paints the thumb at those fractions of its rect rather than laying it out, so a
  new position reaches the bar without a relayout. A bar of your own puts one inside its track.
- The styled bar paints its track inset by `SCROLLBAR_INSET` from the scroll's outer edge and both
  ends, so it does not touch the separators around a panel; the end inset sits outside the
  unstyled bar, keeping the fractions measured over the painted track.

### Edge fades

`ScrollbarStyle::fading(length)` also fades the content toward each edge that has more beyond it,
over at most `length` points and only as far as the content has scrolled, so rows that end exactly
at the edge still read as scrollable. The styled style fades by `theme::SCROLL_FADE`, so every
styled scroll fades. `unstyled::Scroll` works the widths out from the position and wraps the
offset in a base `Fade`, which paints its child inside a mask group (`Painter::faded`,
`Group::Mask`): wgpu multiplies the alpha in the shader, and the DOM renderer masks the element with
a gradient. Custom `Drawing`s are not faded.

### Offset and scroll anchoring

The base `Offset` shows its one child along a direction from an offset, with no bar, theme, fade or
input. Use `Offset` when something needs its content shifted under a viewport and nothing more; use
a `Scroll` whenever something needs to scroll.

The offset is anchored to a node, not measured from the top of the content: the scroll remembers
the node at the top of its viewport and where it started, so a row further up growing or shrinking
(a wrapping label, an image that finished loading) leaves what is being read where it is. It finds
that node by walking down from its child through nodes that lay their children out in place
(`Element::passes_scroll_anchor`: `List`, `Frame` and `Fade`), taking the first child that reaches
past the top of the viewport each time.

The offset places its child once per layout. What the content's layout decides about the anchor
comes back as part of that layout: a `VirtualList` whose rows above the view measured differently
from their estimate reports the difference with `Document::report_anchor_shift`, and the nearest
enclosing offset sums that with how far the anchor drifted and any reveal it was asked for. When
the sum is not zero the offset moves by it and is laid out again in the frame's next layout, where
it does not re-anchor. Anything that moves the offset from outside its layout
(`set_offset_value`, a reveal) calls `on_steer`, which a `Scroll` uses to stop a fling or spring
the moment it is steered.

## Long lists

Layout already skips work for nodes far from view. Every node is still measured, but
`layout::layout` *culls* a node whose rect ends more than `CULLING_MARGIN` outside the visible part
of its parent's painter: it keeps its rect, so `node_rect`, `reveal_node` and placement signals
still find it, but nothing inside it is laid out, painted or interacted with
(`Document::is_culled` says which). This holds for the children of any container; the focused node
and its ancestors and descendants are never culled. Each placement records its `Sight`, the range
of visible regions it stays right for (a node that reads its space through the painter is right
only exactly where it was), and a parent's sight takes in its children's, so a scroll lays a
subtree out again only once a node in it crosses the margin.

A long scroll of ordinary content needs nothing more. Use a `VirtualList` when building every row is
itself the cost.

### VirtualList

A `VirtualList` is a box like any other. It takes `keys` the way `ForEach` does, reports one
`item_size` per key as its length, and builds only the rows its slice of the enclosing viewport
reaches, so it goes wherever a tall child would: directly under a `Scroll`, beside plain siblings
in one, or nested a few containers deep.

```rust
view! {
    <Scroll @sizing=ItemSize::Percent(100.0)>
        <Header />
        <VirtualList keys={row_ids} item_size=ROW_HEIGHT>
            {move |id: RowId| view! { <Row id /> }}
        </VirtualList>
        <Footer />
    </Scroll>
}
```

- `item_size` is an estimate. A row that measures differently is laid out at its measured size; the
  list remembers what each built row measured and reports the sum, so its length grows towards the
  truth and the last row can always be scrolled to.
- A row measured above the viewport would move everything below it, so the list keeps drawing from
  the row it drew from last frame and asks the scroll around it to move its offset by the
  difference; the scroll applies that before placing anything, so nothing on screen shifts.
- Outside a scroll it builds what fits in the box it is given (one sized by a `Stack` builds a
  screenful in one layout and a hundred pixels' worth in the next).
- Rows belong to their keys, not their positions. Scrolling reuses rows that stay in view and
  disposes the effects of ones that leave; the item builder runs once per key while that row stays
  in view. So a row follows its data by binding signals, not by reading them once.
- Changing `keys` keeps every row whose key is still there: inserting a key builds only its row,
  and only if it lands in view; removing one disposes only its row; measured sizes move with their
  keys. A key inserted or removed above the view moves the scroll's offset by the change, so the
  rows on screen stay put.
- A list over data addressed only by position can pass `(0..count).collect()` as its keys.
- Changing `item_size` keeps the rows but forgets the measured sizes; rows in view are measured
  again as they are laid out.
- A list the layout stops reaching (scrolled out of view beside other content, or inside a
  collapsed scroll) hears `Element::unplaced` and releases its rows.

## Sliders

A slider spreads its range evenly along its track unless given a `scale`.
`SliderScale::Midpoint(value)` curves it so the centre of the track reads that value, giving most
of the track to values bunched at one end. The curve is exponential, not a power of the position,
which keeps the fine end useful without giving it the whole track: the inspector's blur slider runs
to 120 px with a midpoint of 12 and reads 3, 12 and 39 px at the quarters. The midpoint may sit
above the centre of the range, giving the top end the fine part. A midpoint outside the range, or
at the centre already, leaves the slider linear.

The curve applies everywhere: dragging maps the pointer through it, the knob sits where the value
falls on it, and keyboard steps move by a share of the track, so an arrow key near the fine end
moves a little and near the coarse end a lot. What a screen reader is told the step is follows the
value the next step would reach.

A slider given a `thumb` length maps the pointer to the thumb's centre, which travels the track
inset by half of it, and a press on the thumb drags it from where it was grabbed rather than
jumping. `ColorArea` takes the same prop for its two axes.

## Dates, times and colours

`beui::datetime` holds the plain values the pickers trade in: `Date`, `Time` (to the minute),
`DateTime`, `Weekday` and `HourCycle`. They carry no time zone; `DateTime::from_unix` and `to_unix`
read and write seconds as UTC, which is what block content stores.

`styled::DateTimeField` is the field for any of them, chosen by `parts` (`DateTimeParts::Date`,
`Time` or `DateTime`):

- A row of spin-button segments (year, month, day, hour, minute, and AM/PM on a 12-hour
  `hour_cycle`) beside a button that opens a popover holding a `Calendar`, a `TimeList` or both.
- A click anywhere in the field, or Alt+Down in a segment, opens the popover over the field with an
  editable copy of the segments on top, focused on the clicked segment, so typing continues while
  the calendar shows the month being typed. Closing puts the focus back on the field's segment.
- The popover is laid out for what it holds: a calendar that grows with the field, a `TimeList` as
  a grid with an hour to a row, both side by side, or on a narrow screen Date and Time tabs where
  picking a day moves on to the time.
- `value` is an `Option<DateTime>`. An empty field shows placeholders; `on_change` fires only once
  every segment is filled; a half-filled field reverts when focus leaves. A `Date` field keeps the
  time of its value, and a `Time` field the date.
- All the behaviour (the popover, its focus, what each pick does) is `unstyled::DateTimePicker`; the
  styled field gives it faces and the popover's layout, and says when to page with `paged`.

`styled::Calendar` is the month grid alone, with `min` and `max`. Its title is a month button and a
year button, opening a grid of months and a grid of twenty years; `show` moves it to a month
without selecting anything.

`styled::ColorPicker` is a saturation/brightness area, hue and opacity sliders, hex, RGB and HSL
fields and a row of swatches; `styled::ColorInput` is a hex field whose swatch opens one. Both keep
the hue while the colour passes through grey or black. A drag reports `on_preview` while it moves
and `on_change` once when it ends, as `NumberInput` reports a scrub, so an edit lands in the undo
history once per gesture. That state is `unstyled::ColorPickerState`, and `unstyled::HexText` keeps
a hex field in step with a colour.

`styled::ColorWheel` is a hue ring around a triangle whose tip points at the hue.
`styled::OklchColorWheel` is the same over OKLCH: the triangle is the lightness/chroma plane, black
to white with its tip at the hue's cusp lightness (`Oklch::cusp`) and a chroma past every hue's
cusp, so it holds all of sRGB. Turning the hue keeps lightness and chroma. A point outside sRGB is
painted and reported clamped to the most colourful colour at its lightness and hue
(`Oklch::clamped`), and a line on the triangle marks where sRGB ends. Both share
`unstyled::ColorWheel`, which works in `WheelPoint`s (hue, and the triangle's saturation and
value) and leaves the colour model to the caller; `ColorPickerState` is generic over the model
(`Hsva` or `Oklch`). The faces are a `Drawing` painted by a shader on wgpu, behind
beui-components-styled's `gpu` feature; the DOM renderer fills it with magenta.

### Popover

`unstyled::Popover` is what the date and colour fields open: a trigger and an overlay, built the
first time it opens, that traps Tab, closes on Escape, a press outside or its handle's `close`, and
gives the focus back to the trigger when it closes. It dims nothing, so its layer passes input
through (`LayerPolicy::Pass`): the pointer goes on hovering the document around it, and a press
outside asks it to close and then lands on whatever was pressed, except a press on its own trigger
(the overlay's `trigger`), which is left to the trigger so a second click closes it rather than
reopening it. What it holds decides where the focus lands on open by binding a `focused` prop to
the handle's `open`, as the calendar, the time list and the colour area all take.

## Tooltips

`unstyled::Tooltip` shows something after the pointer has rested on its child for a moment;
`styled::Tooltip` is the bubble version:

```rust
view! {
    <Tooltip label="Reveal the block being shown">
        <IconButton glyph={ICON_MY_LOCATION.to_owned()} label="Reveal" on_click={find} />
    </Tooltip>
}
```

`IconButton` already wraps itself in one: the `label` it publishes to a screen reader is the text
the bubble shows. Wrap anything else whose meaning is not on screen (a bare glyph in a row, a
truncated cell) in a `Tooltip` of its own.

The bubble lives in an inert `Overlay` (`LayerPolicy::Inert`): it paints above what it hangs from
and takes no input, so the document under it keeps answering the pointer, Escape still reaches what
it was going to, and clicking the described control clicks the control. The dwell is a
`create_timer`, started when the pointer arrives and stopped when it leaves, so an unhovered
tooltip costs no frames.

## Time: timers and animation

beui has no per-frame hook. Work runs because a signal it reads changed, and anything that depends
on time asks for it with `create_timer(work)`.

- The timer belongs to the reactive scope that created it and is dropped with it. The `Timer`
  handle is `Copy`; calling it after its scope is gone does nothing.
- `start(delay)` schedules `work` (keeping an earlier deadline if one is set), `restart(delay)`
  replaces the deadline, `stop()` cancels it, `running()` says whether one is pending.
- `work` returns `Some(delay)` to run again (an animation returns `Some(Duration::ZERO)` for the
  next frame) and `None` once settled, which lets a fling or spinner stop costing frames.
- The document runs due timers inside its reactive scope at the top of a frame and asks the context
  for a repaint at the next deadline, so nothing is drawn while no timer is due.

Start a timer from the event or effect that begins the motion rather than leaving one running.

`pixels_per_point()` is the document's scale as a signal, for layout that depends on it.

`styled::Spinner` is an indeterminate progress bar that animates only while laid out: one behind a
false `Show`, or in a tab nobody is looking at, asks for no frames. It uses `node_placed`, which
works for any component that should only work while on screen.

## Overlays and layers

An overlay is laid out and painted above the rest of the document. Hand an `Overlay` what it shows
as `content={move || ...}` rather than as children: it calls that the first time it opens, so a
closed tooltip, select, menu or prompt costs its overlay node and nothing else. Children are built
with the overlay whether or not it ever opens; use them only for content that must exist while
closed. State the rest of the component reads (such as which rows a select's search hides) belongs
outside the content, derived from data rather than from nodes that may not have been built.

### The layer tree

Every open overlay is a layer, and the layers form a tree: a layer's parent is the layer holding
the node it hangs from (its anchor when anchored to a node, otherwise the overlay node itself), and
the document root is the bottom layer. `Document::layers()` is that tree as one ordered list,
bottom to top, and paint, hit testing, focus, Escape, back and key dispatch all walk it. It is
worked out once per change to the tree or the layers and shared until the next.

- Siblings that block everything sit above those that do not; otherwise they are ordered by when
  they were opened or raised (`Document::raise_overlay`), so raising a window never lifts it over a
  dialog.
- Children sit above their parent and rise with it.
- A `level` (`LayerLevel`) puts a layer and everything above it over every layer of a lower level:
  `Top` for things that must float over every window and dialog (a drag preview, a volume OSD,
  `styled::Toasts`, so a toast stays readable and pressable while a dialog is open), and `Lock` for
  the lock screen.
- Opening a layer raises it and every layer it hangs in, so a menu opened in a covered window comes
  up with its window. An inert layer rises alone, so a tooltip over a covered window leaves the
  window where it was.

### Layer policies

Each layer has one `policy` (`LayerPolicy`):

| Policy | For | Behaviour |
| --- | --- | --- |
| `BlocksAll` (default) | a menu, a select popup, a dialog | its content takes the pointer; a press anywhere else is shut out (it asks the layer to close and goes no further) and nothing beneath is hovered |
| `Pass` | a window, a toast, a popover | its content takes the pointer where it is, so a press on it does not land underneath; everything around stays live |
| `BlocksParent` | a modal `DockDialog` | modal to the node it hangs from alone: a press on that node is shut out and asks the layer to close, the focus cannot enter that node (Tab skips it; focusing inside it focuses the layer's first focusable), everything else stays live |
| `Inert` | a tooltip, a drag preview | takes no input at all |

`traps_focus` (on unless turned off) keeps Tab inside the layer and makes it a boundary for keys
(`guides/beui/input.md`). A menu, a dialog and a popover trap; a window does not. A layer that
blocks everything but traps nothing, such as the touch selection's Copy and Paste menu, leaves the
focus in the text field beneath.

### Dismissing

Closing is asked, not done: `on_dismiss` is called with a `Dismiss` reason (`Escape`, `Back` or
`OutsidePress`), and its owner closes the overlay by writing `open` false, or decides not to (a
modal `DockDialog` asked by a press on its parent flashes instead). An overlay with no `on_dismiss`
cannot be dismissed. Each request goes to one layer:

- **Escape** that nothing on the focus path took goes to the nearest dismissable layer holding the
  focus or, with nothing focused, the topmost trapping layer.
- **Back** (Android's back gesture, the Back key or mouse button) goes to the topmost layer that has
  an enabled `BackHandler` in it or is dismissable, stopping at the first layer that blocks
  everything; the root's handlers come last. `FrameOutput::handles_back` says whether anything
  would take it.
- **A press outside** goes to every dismissable layer it is outside of, from the top down, until it
  lands in one or is shut out by one that blocks everything (and then to the chain of blocking
  layers that one is nested in, so a press outside a submenu closes the whole menu). A blocking
  layer still lets the pointer reach the blocking layers it is nested in, so a menu's rows stay
  hovered and pressable under its open submenu, and pressing one closes the submenu. Which layers
  the pointer can reach is one rule, `Layers::reachable`, shared by the hit test and the forwarding
  of input to a forwarding catcher. A mouse asks on the press; a finger on the tap, so a scroll
  that starts outside a layer leaves it open.

Inside the layer it is sent to, a request goes first to the most recently made enabled
`BackHandler` in that layer, and to `on_dismiss` only when there is none, so Escape, back and a
press outside all play the same slide-away. An overlay closed because its parent closed is told
after the fact, as `Dismiss::WithParent`. The owner writing `open` false
(`Document::close_overlay`) runs nothing, so a prompt whose dismiss means "cancel" needs no guard
against its own Keep button. `Document::dismiss_overlay` asks an overlay to close as a press
outside would.

### Focus around layers

Every layer gives the focus back when it closes, however it closes (its owner writing `open` false,
a parent closing, or its node being removed while open). It remembers the node that had the focus
when it opened, or the last one focused outside it since, and refocuses that node if the focus is
inside the layer when it closes, or is nowhere and the layer traps focus, and the node still
exists. A close that moved the focus outside the layer on purpose keeps it there, and so does a
focus change that comes after the close. Components rely on this rather than refocusing their
triggers themselves: a select, the command palette and a dialog put the focus back on what opened
them, and a forwarding catcher (a plugin's region, a Wayland window) gets the keyboard back.

What a layer remembers is a focus scope, and the document keeps one stack of them
(`base/focus_scope.rs`): an open layer pushes one and closing pops it. Anything else that takes the
focus for a while and gives it back pushes its own with `Document::push_focus_scope(holder)` and
pops it with `pop_focus_scope(holder)`, which refocuses what had the focus only if the focus is
still inside `holder`. block-app's region does that when a plugin asks for the keyboard.

Focus asked for beneath the topmost trapping layer never lands there, so nothing beneath hears it
gained focus. It is remembered until the next input event: if the layer closes first, the focus
goes there rather than back to what the layer remembered, so a dialog's button can close the dialog
and focus something outside it in either order.

The focus is checked when something that could take it away changes (a layer opening or closing, a
frame hiding, a node removed or moved) by walking up from the focused node, never by searching the
tree; a node whose parent link is gone is outside the tree.

### Back and slide-away

A base `BackHandler` only routes: it calls `on_back` when back completes, or hands every phase of
the gesture to `on_gesture` if it has one, and moves nothing. The motion lives above it:

- `unstyled::BackSlide` wraps a page. Held, the page follows the finger across a good share of the
  screen; let go, it carries on off the edge before `on_back` runs and the next page slides in
  behind it; a cancelled gesture eases it back. A back with no gesture (a key, Escape, a press
  outside) carries the page off from where it rests in the same slide-away.
- Given `behind`, a function that builds the page back goes to, it builds that page under the
  moving one for the gesture's duration, shaded and coming in from the side, so `on_back` swaps in
  what was already showing. The moving page needs an opaque background, and a page that must keep
  its state (a scroll position) is built once and handed to a `Portal` in both places. The stacked
  dock shows its home tab behind the tab it leaves this way.
- `styled::Dialog` and `Fullscreen` slide away the same way and fade their scrim.
- On Android the runner passes `handles_back` to the activity's `setBackHandled`; when nothing takes
  it, the system's own back (to the home screen) plays.

The inspector's Back gesture slider plays the gesture without a phone (`guides/beui/apps.md`).

### Sheets

`Sheet` is the panel that rises from the bottom of a narrow screen.

- It scrolls what it holds itself, so its content is not wrapped in a vertical `Scroll` (one inside
  would measure nothing tall). A swipe anywhere on it raises it to its top stop before scrolling
  the content, and lowers it once the content is back at its start.
- It is never taller than what it holds, whatever stop it rests at, and pulled past its top it
  stretches like an overscroll.
- It slides up easing out as it opens and down easing in as it closes, from however fast it was
  flicked. A `ModalSheet` stays up while it leaves however it was closed, and fades its scrim with
  it.
- Let go, it springs to the stop nearest where the flick was heading, or closed below the lowest
  one; its content flings and bounces at its ends as a `Scroll` does. The handle does the same for a
  mouse, and going back closes it, sinking with the gesture and sliding on down from where it was.

### Lock screen

A layer at `LayerLevel::Lock` that blocks everything is a lock screen (`styled::LockScreen`):

- it is above every other layer, so anything opened while it is up opens beneath it;
- it is never dismissable, and the focus cannot leave it, not even to nothing;
- `Document::locked` is true, and while it is, the listeners above the tree (`on_global_key`,
  window and host actions) hear nothing but media keys (`Key::is_media`);
- a forwarding catcher beneath it loses any press it had captured, as one beneath any layer that
  blocks everything does.

### Floating

`unstyled::Floating` pins its child to the top or bottom edge of a node named by `NodeRef`, taking
no space in the layout around it:

```rust
let scroll = NodeRef::new();
view! {
    <List spacing=0.0>
        <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
            <Rows />
        </Scroll>
        <Floating anchor={scroll.clone()} edge=Edge::Bottom open={astray}>
            <Button label="Jump to the open block" on_click={reveal} />
        </Floating>
    </List>
}
```

## Screens

A document knows the screens it is shown on: `Document::screens()`, and `use_screens()` in a
component, a signal of `Screen`s (an `id` stable while the monitor is plugged in, a `name`, and a
`rect` in document coordinates). A window, browser tab, Android view, plugin region and inspector
panel are one screen the size of what the document is shown in; under `block-app --session` there
is one per monitor.

They come from the renderer: `Renderer::screens` hands the runner each monitor in physical pixels
(beui-adapter-drm's outputs, beui-adapter-plugin's from the monitors the host says its region
covers), the runner puts them on the `Context` (`Context::set_screens`, also how a test gives a
document two screens), and the document keeps the part of each inside the rect it is shown in.
Nothing else should keep its own list of monitors.

Overlays place themselves on one screen, not across the box around all of them:

- A `Center` or `FillScreen` overlay is on the screen it opened on: its trigger's, else where the
  pointer was last pressed or moved unless a key was pressed since, else the focused node's, else
  the pointer's, else the first.
- One anchored to a node is kept on the screen most of that node is on; one anchored to a point on
  that point's screen; one anchored to a rect (`OverlayAnchor::Rect`, document coordinates) on the
  screen most of that rect is on, placed against the rect as against a node.
- `Fill`, `At` and `Around` use the whole document, so something meant for every screen
  (`styled::KeepChanges`, `styled::LockScreen`, `styled::LevelOsd`) lays out one card per entry of
  `use_screens()`.

`styled::Toasts` takes any anchor this way, so linux-desktop stacks its toasts in the corner of a
screen above its bar. A toast goes after its `duration` unless it is `sticky` or `danger`: an error
stays until dismissed.

## Docking and windows

`styled::Docking` is the workspace layout: panes split from one another, a tab bar on each pane,
and tabs that can be dragged between panes or out into windows floating over the dock.
`unstyled::Docking` underneath owns the tree, the dragging and the keyboard, and paints nothing.
The demo in `crates/beui-demo` is laid out in one.

### Declaring a dock

The children describe the layout the dock starts with and the tabs that exist; a
`DockingLayout<K>` holds where everything is now, and is what the caller saves and restores:

```rust
let layout = DockingLayout::new();
view! {
    <Docking layout home=Key::Files>
        <DockSplit id="root" direction=Direction::Horizontal fraction=0.25>
            <DockPane id="files">
                <DockTab id=Key::Files title="Files"><FilesPanel /></DockTab>
            </DockPane>
            <DockPane id="editors" empty={move || view! { <Nothing /> }}>
                <ForEach keys={open}>
                    {move |key: Key| view! {
                        <DockTab id=key title={title_of(key)} on_close={move || close(key)}>
                            <Panel key />
                        </DockTab>
                    }}
                </ForEach>
            </DockPane>
        </DockSplit>
        <DockWindow id="inspector" rect={INSPECTOR}>
            <DockPane id="inspector">…</DockPane>
        </DockWindow>
    </Docking>
}
```

- `DockSplit` takes two children; `fraction` is the first one's share. A split follows the code's
  `fraction` until the user moves it.
- `DockPane` holds `DockTab`s and `DockGroup`s (a tab holding a tree of its own, `pinned` to keep
  its tabs in it), names its starting `active` tab, and can start with its tabs in a sidebar
  (`vertical`, `sidebar_width`).
- `DockWindow` floats one tree at a `rect`.
- A tab's key is any `Clone + Eq + Hash` type. A key that is a `TabId` or a `u64` names the tab's
  `TabId` too, which the test ids of its close button and switcher row carry.
- Every container has an `id`, saved with the layout, so a layout saved by older code still finds
  its panes. A pane, split or window the code adds is placed beside its declared sibling when the
  saved layout lacks it.
- A tab's icon comes from the optional `icon` function, an icon-font glyph (empty for none) drawn by
  the tab bars, the drag preview and the stacked bar.

The children are the full set of tabs: a tab the code no longer declares is closed, and one it
starts declaring is placed.

- Closing is a request: `on_close` gives the tab its close button, and the tab goes when the caller
  stops declaring it, or stays (the workspace keeps a program's window until the program quits).
- A new tab joins the pane it was declared in, after the tab declared before it, and is shown. If
  the user closed that pane, it joins the declared tab nearest it that is still open, then a window
  the code declared it in, and failing both it floats in a window of its own.
- A pane with an `empty` view is never closed: emptied, it shows that view, and dragging its grip
  onto another pane moves its tabs and leaves it behind.
- A tab that stops being declared for a moment (a `Show` around it that flips, a `ForEach` rebuilt
  from new keys) loses its place, so keep a tab's declaration in one stable place.

### The layout handle

`DockingLayout` is a `Copy` handle owned by the scope that made it; `OwnedDockingLayout::new()`
makes one outside a scope (a host, or a test before it builds) and derefs to it.

| Call | Does |
| --- | --- |
| `snapshot` / `restore` | the `DockingSnapshot` to save, and putting one back; restore it in the same `batch` that declares its tabs, since a tab not declared when the dock reconciles is closed |
| `reset` | back to the layout the children describe |
| `show`, `focused`, `shown` | show a tab; read the focused tab; `shown` for a stacked dock |
| `replace(key, with)` | puts a new tab in an open one's place (same pane or window, same index) and shows it; declare `with` instead of `key` in the same `batch` |
| `state()` | the plain `DockState` value; `find`, `all_tabs`, `focused_tab` and `surface_tabs` answer from it |
| `enter_fullscreen`, `leave_fullscreen` | see "Fullscreen" |
| `begin_switch`, `step_switch`, `commit_switch`, `cancel_switch`, `switching`, `switch_choice` | see "The switcher" |

A list that opens what it names into one tab (the demo's catalog, be-launcher's pull requests)
replaces the tab shown last on a plain click; Ctrl (or Cmd) and a click opens another.

### Stacked mode

Tiled, `Docking` keeps its panes inset from its edges. `mode=DockMode::Stacked` draws the same
state as one screen: the focused tab fills the dock edge to edge, with no tab bars, splitters or
windows, and everything else in the state is kept, so switching back to `DockMode::Tiled` restores
the layout. The workspace stacks its dock on a phone.

- The dock draws the stacked bar itself: the tab's icon and title; a square counting the open tabs
  other than home, which opens a sheet of cards to show or close them; and a back button when the
  caller names a `home` tab (`home={Some(FILES)}`), which the back gesture also follows and whose
  own screen has none.
- What a tab adds to the bar is only its own actions, which its panel hands over while built with
  `dock_actions(node)` (shown while that tab is), and its menu, handed over with
  `dock_menu(actions)`, a `Memo<Vec<Action>>`.
- The menu is not tied to the stacked bar: a tiled pane offers the menu of the tab it shows behind a
  More button just before its close button, at the end of its tab bar or the top of its sidebar,
  and so does a window. A menu is a list of actions rather than a node, so a dock laying out tabs
  built elsewhere can draw it from rows it was sent, with actions made by
  `ActionBuilder::detached` (not registered for shortcuts or the palette), and pass the pick back.
- Each tab's panel is built once and moved between modes, so what it holds survives the switch.
- `recent_tabs` lists the tabs from the one shown last (part of the saved state); `stacked_tab` is
  the tab a stacked dock shows: the focused one, or the one shown last when the focused pane is
  empty.

### Panels

A tab's panel is built the first time the tab is shown and belongs to the dock, not the pane
showing it: the pane holds a `Portal` pointed at it, so the panel keeps its nodes, scroll position,
caret and state when hidden behind another tab, dragged to another pane or floated into a window.
A panel no pane shows is laid out by nobody, so it costs nothing and a screen reader does not read
it. Closing the tab removes it.

### Dragging, bars and keys

- Dragging a tab picks a drop target under the pointer: a tab bar inserts it between tabs there,
  the middle of a pane joins that pane, an edge splits it. Holding Alt while dropping floats it
  into a window, as "Pop out into a window" on the tab's menu does. A drag the state would refuse
  (`admits`, `admits_leaf`) shows no drop marker and does nothing on release.
- Every pane and window wears the same bar: a grip, the tabs, and a close-all button shown only when
  every tab in it can close. A middle click on a tab closes it.
- A finger picks a tab up by dragging it out of its bar, across the way the bar scrolls; sliding
  along the bar scrolls it.
- Dragging a docked pane's grip carries the whole pane (`DockState::drop_leaf`), with the same drop
  targets as a tab.
- A window holds one pane, so a tab dropped anywhere in one joins it rather than splitting it, and
  that pane's bar is the window's title bar: anywhere on it that is not a tab drags the window.
- Right-clicking a grip moves that pane's tabs into a sidebar beside its body
  (`DockState::set_vertical`), whose edge drags or arrows to a new width (`set_sidebar_width`); a
  window in that mode has no title bar, only the sidebar with the grip and close button at its top.
- Windows resize from any of eight grips and are raised by whatever takes the focus inside them.
  The grips and the bar between two panes reach `HANDLE_REACH` points past what they draw (a
  window's overlay is that much larger on every side, a splitter's item that much wider on both),
  so they can be grabbed without changing how they look, and they capture the press over whatever
  lies beneath.
- A split keeps each side at least `MIN_PANE_LENGTH` where there is room for both, and a window is
  drawn no larger than the dock, so a layout made on a wide screen stays usable on a phone.
- Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of the pane the focus is in, registered with
  `on_shortcut` so they arrive even from a text input in a panel.
- The bar between two panes is a tab stop with a `Splitter` role; the arrow keys move it. The tab
  bar is a `Choice` inside a `Scroll` running the way the bar does, so arrows, Home and End walk it
  and scroll the tab they reach into view.

### Surfaces and window stacking

Each surface (the main one and one per window) lays its tree out over the rectangle it was given,
so panes and the bars between them are canvas items at computed rectangles, not nested boxes.
`layout_surface` is that calculation on its own, which drop targets and tests resolve against. A
window is a floating overlay, so it paints above the dock, takes the pointer where it is, and the
document underneath is shut out rather than answering a press through it.

How windows stack is the document's layer order alone (`Document::layers()`), not anything in
`DockState`. A press on a window and the focus moving into one raise its layer (over a fullscreen
tab too, unless the focus is in that tab). A drop target is looked for in the window whose layer is
on top at the pointer, never in one covered there by a layer that is not the dock's (a menu, a
dialog, the fullscreen tab).

### Groups

A place in a tab bar holds an `Entry`: a `Tab` or a `Group`. A group is a tab holding a dock tree
of its own, so choosing it shows that tree in the pane's body (one pane with a second tab bar under
the first, or panes split side by side), and the same drops work inside it.

- Dropping a tab onto the middle of another tab groups the two; onto a group, it joins the group.
  The outer edge of a pane showing a group still splits the outer pane, so the group's drop zones
  sit inside a thin band that belongs to its parent.
- A tab's menu offers "Group with next tab" and "Split with next tab"; a group's menu ungroups it,
  closes every tab in it, or floats it into a window as a whole.
- The tree tidies itself after every change: a group left with one tab turns back into that tab,
  and a tab bar left holding only a group takes the group's tabs, or its split, in its place.
- Reading: `entries`, `active_entry`, `locate`, `group_tabs`, `tree_leaves`, `surface_of`,
  `is_nested`; `layout_tree` lays a group's tree out as `layout_surface` does a surface's. `find`,
  `all_tabs`, `surface_tabs` and `show` look through groups, and showing a tab inside one selects
  the group in every bar above it.
- Changing: `drop_entry`, `group_with_next`, `split_with_next`, `ungroup`. `group_title` names a
  group in its tab.

A pinned group is never tidied away. `insert_pinned_group` puts one in a tab bar; it stays a group
while it holds one tab or none, cannot be ungrouped or closed from its menu, and admits only the
tabs it was given, so dragging any other tab onto or into it does nothing. Its tabs are pinned too:
one can be rearranged inside the group, and the group moved with all of them, but a pinned tab
cannot be dragged, popped or carried out with its pane. "Unpin from group" (`set_tab_pinned`) lets
a tab go; "Pin to group" pins it again once it is back. `unpin` makes the whole group ordinary.

`tree` reads a surface's or group's layout out as a `DockTree`, which names tabs but no leaf, split
or group ids, so it can be compared, sent elsewhere and rebuilt: `from_tree` makes a state from one,
and `set_tree` replaces a tree in place, moving in any of its tabs that were elsewhere and keeping
the focused tab focused.

### The switcher

The dock's switcher walks `recent_tabs` across every pane and window, as Alt+Tab walks a desktop's
windows.

- `DockingLayout::begin_switch(backwards)` chooses the tab shown before the current one (or,
  backwards, the one shown longest ago); `step_switch` moves the choice along and wraps;
  `commit_switch` shows the chosen tab (activating it in its pane and raising its window);
  `cancel_switch` leaves everything as it was. `switching` and `switch_choice` read it back.
  `DockState` has the same calls and `switch()`.
- While a switch lasts the dock opens an overlay in the middle of the document listing the tabs
  with icon and title, the choice highlighted, drawn by the `switch` render prop
  (`DockSwitchHandle`, a row per tab whose `pick` commits it). Nothing is shown until commit. A
  switch is not part of the saved layout.
- Escape cancels it, with whatever modifiers held and wherever the focus is (a program's window
  included): the overlay holds no focus, so the dock listens for Escape at the window while it is
  open and calls `Document::dismiss_overlay`. A dock in a plugin only hears a key a program has the
  keyboard for when it intercepts the chord, so linux-desktop also registers Alt+Escape as an
  intercepting action that cancels.
- The dock binds no other key. Whoever binds it picks the keys, usually a chord that begins or steps
  the switch while a modifier is held, and `held_modifiers()` letting go of that modifier to
  commit: linux-desktop uses Alt+Tab, the demo Alt+Q.

### Window-manager drags

`drag_modifier` (none unless asked for) makes the dock a window manager while those modifiers are
held:

- A primary drag anywhere in a floating window (content, bar or tabs) moves the window; a secondary
  drag resizes it from the edge or corner nearest the pointer (by thirds of the window).
- On a docked pane a secondary drag moves the nearest split; a primary drag does nothing (it is
  swallowed, since a press with the modifier is never meant for the content).
- Each window and docked pane claims those presses (`claim_modifiers`), so neither tabs nor content
  see them.

The code is `dock/modifier_drag.rs`. linux-desktop sets Super; the demo's Docking page has a switch
for Alt.

### Dialogs

A `DockDialog`, declared beside the dock's other children, is a window attached to a tab:
`<DockDialog id parent title size modal on_close>content</DockDialog>`.

- It hangs over the tab named by `parent` (or over another dialog, so dialogs of dialogs stack),
  centred on the parent's panel at the `size` its content asks for and kept on the parent's screen
  (`Placement::CenterOn`), in the `dialog_chrome` render prop (styled: a bordered card with a title
  bar and close button, `dock.dialog.<tab>` naming its body).
- It is not part of the `DockState`: never a tab, nothing drags it into the dock, a key declared as
  both a dialog and a tab is only a dialog, and it is not saved with the layout.
- It shows exactly while its parent tab does (`DockState::is_shown`), and rises with the window its
  parent is in, since its layer hangs from the parent's panel.
- A new dialog takes the focus. The dock tracks which dialog has it beside the focused tab: pressing
  a dialog gives it the focus, pressing the parent's pane or tab bar takes it back, and when a
  dialog closes the focus returns to its parent. The content's `use_dock_tab()` is a
  `DockTabControl` for the dialog, whose `focused` follows that and whose `show` focuses it.
- `modal` makes the dialog's layer `BlocksParent` until it closes or stops being modal. A press on
  the parent's panel is shut out and asks the dialog to close, which it answers by focusing the
  innermost modal dialog and flashing it (the handle's `attention`; `DockTabControl::attention`
  for content in another document, such as a program's window, to take the keyboard itself). The
  focus cannot move into the panel, and while the parent's tab is focused the focus goes to its
  innermost modal dialog.
- Escape or back with the focus in a dialog closes it through `on_close`.

### Fullscreen

One tab at a time can be fullscreen: `DockState::set_fullscreen` with a `DockFullscreen` naming the
tab and an area (a rect in document coordinates, or `None` for the screen it opens on),
`DockingLayout::enter_fullscreen` and `leave_fullscreen` from outside, or the `DockTabControl` that
`use_dock_tab()` hands the tab's own content.

- The panel moves into a floating overlay over that area, wrapped in Docking's `fullscreen` render
  prop (styled: the theme's background), covering tab bars, other panes, the dock's windows (even
  one the focus moves into) and anything else there; nothing under it hears the pointer, while the
  rest of the document stays live.
- Leaving puts the panel back in its pane without rebuilding it. Fullscreen is not saved with the
  layout.
- The dock binds no key to leave: whoever enters fullscreen offers the way out (the demo uses
  Escape). `DockTabControl::offer_fullscreen` adds Full screen / Leave full screen to the tab's
  right-click menu; only content with its own way out should call it.
- Showing or focusing the fullscreen tab (`DockingLayout::show`, the switcher, a press on its
  content) raises the fullscreen overlay above the floating windows, its own pane's window
  included, so a window brought in front and then left goes behind it again.
- The pane it left draws Docking's `away` render prop (styled: a note and a Leave full screen
  button), seen where the fullscreen area does not cover it, such as another screen.

## Drag and drop

Moving something within a document is `unstyled::Draggable` on the thing being moved and
`unstyled::DropTarget` on each place it may land. Nothing else wires them together: every document
keeps one drag board, a draggable puts its payload on it, and targets whose payload type matches
hear about it.

```rust
view! {
    <Draggable
        payload={card_id}
        preview={move |card: CardId| view! { <CardGhost card /> }}
        on_click={select}
    >
        {move |handle: DragHandle| view! { <CardFace card_id dragging={handle.dragging} /> }}
    </Draggable>
    <DropTarget on_drop={move |(card, _): (CardId, DragPoint)| move_card(card, column)}>
        {move |handle: DropHandle| view! { <ColumnFace column over={handle.over} /> }}
    </DropTarget>
}
```

`Draggable`:

- `payload` is a `Prop<Option<P>>`, so a source that may not move right now (a read-only block, a
  disabled slot) binds a memo that answers `None`.
- A press becomes a drag only once the pointer has travelled `threshold`; a press that never does
  is reported through `on_click`, so a clickable, draggable row does not select itself at the end
  of a drag.
- `on_drag_change` says when a drag starts and ends (for a source that dims while carried).
- Nested draggables (a card inside the pile that holds it) set `capture_presses`, so a press
  reaches the innermost; of overlapping ones only the topmost hears it.
- The `preview` is shown beside the pointer in an inert overlay at `LayerLevel::Top`. It follows
  the pointer the document saw, not the draggable's catcher: the board is fed the pointer at the
  start of every frame, even where a floating overlay covers the source, so a drag carried over a
  floating window keeps going.
- A finger drag waits for `TOUCH_DRAG_THRESHOLD` and for the catcher to hold it rather than a
  scroll, so a `Draggable` in a scrolling list takes finger drags only where it sets `touch_drags`
  or `touch_drag_axis`. Its preview sits above the finger.

`DropTarget`:

- It is registered by the rectangle its content was laid out at. The target under the pointer that
  accepts the payload takes the drop; `accepts` filters by value, and a target of another payload
  type never sees the drag. When targets nest, the innermost wins.
- `DropHandle` carries `carrying` (true for every target that would accept what is dragged) and
  `over` (true for the one that would take it now).
- `on_over` reports the payload and the pointer as it moves: how a target that works out where
  inside itself the drop lands (the dock's tab bars and split zones, a timeline's insertion point)
  draws its marker, and how a filmstrip reorders live.
- The drop is handed over before the source hears `on_drag_change(false)`, so a source can keep
  what it set up until the drop has used it.

A drag between editors crosses plugins, so it goes through the host with `Editor::drag` and
`accept_drag` (`guides/plugins/editor_api.md`). The host carries it on its own document's board: a
plugin's request to drag a block puts it there with `Board::begin`, each plugin region is a
`DropTarget` for it, and the board drops whatever it carries when the primary button or finger is
let go, wherever the press was, so a drag no `Draggable` began still ends. A cancelled touch ends it
without a drop.

## Pan and zoom

`unstyled::PanZoom` turns wheel, trackpad and middle-button gestures over a rectangle into camera
movement and owns none of the camera. The caller keeps a `PanZoomView` (the world point at the
middle of the viewport, and a scale), passes it in, and writes back what `on_change` reports:

```rust
let (view, set_view) = create_signal(PanZoomView::IDENTITY);
view! {
    <PanZoom view on_change={move |view| set_view.set(view)}>
        {move |handle: PanZoomHandle| {
            let PanZoomHandle { view, scale, .. } = handle;
            view! {
                <Canvas view>
                    <CanvasItem x=0.0 y=0.0 width=100.0 height=50.0 />
                </Canvas>
            }
        }}
    </PanZoom>
}
```

- A scroll pans, Shift+scroll pans sideways, a middle-button or two-finger drag pans, and
  Ctrl+scroll, a trackpad pinch or a two-finger pinch zooms around the pointer or the point between
  the fingers.
- It is a tab stop: the arrows pan, `+` and `-` zoom around the middle, `0` returns the scale to
  one.
- The handle carries the camera as a `CanvasView` ready for a `<Canvas>`, the `scale` for content
  that should grow with the zoom, whether a pan is in progress, and whether it has focus (to paint
  a focus ring from).
- Because the view is the caller's, a toolbar button, a fit command or a host syncing several
  editors writes the same signal the gestures do. The camera a plugin editor is handed
  (`guides/plugins/pan_and_zoom.md`) reaches a beui editor the same way.
- Anything between gesture and camera (momentum, snapping, clamping to the content) belongs to the
  caller, apart from the scale limits `min_scale` and `max_scale`.

## Picking a file

`beui::reactive::create_file_picker(picked)` gives a component a `FilePicker`:

- `open(FileFilter::new(name, extensions, mime_types))` asks the platform for one file.
- `picking()` is a signal true until the answer arrives (a button takes it as `disabled`).
- `picked` is called with the `PickedFile` (its name and bytes) or with why it could not be read; a
  cancelled pick only clears `picking`.
- `pick_file(filter, picked)` is the one-shot form underneath, handed the raw `FilePick`.

Nothing blocks while the dialog is open. The document queues the request, `Context` hands it to the
runner in `FrameOutput::file_picks`, and the runner hands each to `Platform::pick_file` with a
`FilePickAnswer` that the platform sends when the person is done: winit from a thread running the
native dialog, the web runner from an `<input type=file>`, Android from `BeuiActivity`, a plugin
through its host; a platform without a picker answers that it has none. The answer arrives at the
start of a later frame, inside the reactive scope, so `picked` can write signals like an event
handler. A host that runs beui itself does the same with the requests in `file_picks`.

## Holding the pointer

A control that looks around rather than pointing (a first-person scene, a modeller's orbit) is
`unstyled::PointerLock`. It is a tab stop that takes the pointer when pressed; while it holds it the
pointer stops moving, is hidden, and reports how far it moved instead of where it is:

```rust
view! {
    <PointerLock
        locked={looking}
        on_change={move |looking| set_looking.set(looking)}
        on_motion={move |motion: Vec2| camera.look(motion)}
        on_key={walk}
    >
        {move |handle: PointerLockHandle| view! { <Scene /> }}
    </PointerLock>
}
```

- The lock is controlled state, like a `Toggle`'s: it follows `locked` and reports what it wants
  through `on_change`.
- It lets go on Escape, on losing focus, and when removed. Keys it does not use go to `on_key`.
- beui only asks. `Document::show` publishes the wish as `WindowState::pointer_locked`, and whoever
  shows the document does it: the winit runner locks and hides the cursor, and a plugin region
  passes it to the host in its `WindowRequests` (`guides/plugins/architecture.md`). An adapter
  reads raw motion only while `Runner::window()` holds the pointer.
- What comes back is `Event::PointerMotion`, a delta with no position, delivered to the focused node
  while the pointer is held and nowhere while it is not. So a control not holding the pointer never
  sees motion, and a window that loses the keyboard gives the pointer back.

## Custom drawing

`Drawing` is the one base node that paints rather than arranges. It takes a `Draw` callback handed
the `Painter` and the rectangle the node was laid out at, for content whose shape is computed: a
plot, a waveform, a canvas of strokes. Use it only when no arrangement of nodes says the same thing:
a row of labels is a `List` of `Text`, not a `Drawing`.

- Build the callback in a memo over the page it draws, so the closure is replaced only when that
  page changes, and cull to `painter.clip_rect()` inside it, so a document far taller than the
  viewport costs the screenful it shows.
- It measures to nothing, so it takes its size from whatever places it (a `Frame` with a width and
  height, or a `CanvasItem`), unless given a `size`, which it measures to, scaled down to the width
  offered and keeping its shape, as an image does.
- Text inside a drawing is laid out with `layout_text` (`guides/beui/writing_components.md`,
  "Text").
- A callback that paints in document coordinates rather than relative to its rectangle (as
  `infinite_canvas` does through its camera) paints through `painter.in_document()`.

### Drawing with the gpu

A viewport whose pixels no arrangement of nodes can produce (a 3D scene, a ray tracer, a map) is a
`Drawing` node whose callback paints a gpu drawing: `draw_gpu(drawing)` builds one that paints a
`Shape::Drawing` over the node's rectangle. The renderer runs the `Draw` behind it where the shape
sits in paint order, so nodes written after it still paint over it.

```rust
view! {
    <Drawing
        draw={Prop::dynamic(move || draw_gpu(drawing.get()))}
        @sizing=ItemSize::Percent(100.0)
    />
}
```

`Draw` has the two halves a gpu frame has:

- `prepare` is handed the device, queue and an encoder: its own passes go there (rendering into its
  own textures, writing its own buffers).
- `paint` is handed the render pass beui is painting the document into, and draws there.
- Both get a `DrawAt`: the rectangle and clip in physical pixels, the size of the whole target, the
  scale, the target's texture format (what a pipeline built on the first frame is built against),
  and `renderer`, a number the same for every frame one renderer draws and different for any other.
  Key gpu resources shared between drawings by `renderer`, not by the device: wgpu devices of two
  instances can compare equal. The colour wheel (beui-components-styled's `color_wheel/gpu.rs`)
  shares one pipeline that way.
- A `Draw` must leave the pass's viewport and scissor as it found them: position what it draws from
  its rectangle, as beui's own shader does, rather than by setting a viewport.

`beui::drawing` wraps a `Draw` in a `Drawing`, and two drawings are the same shape when they are the
same `Rc`. That decides whether the frame changed, so a drawing whose contents moved is a new
`Drawing`, and a frame showing the same thing keeps the one it had, which leaves an idle editor
idle. A drawing that changed only in part hands its node `draw_gpu(Some(drawing.redrawn(region)))`
instead, damaging only that region in the drawing's own coordinates (the plugin host does this with
the rectangles each plugin frame reports changed).

The gpu resources belong to the `Draw`, not the drawing: keep them in an `Rc<RefCell<Option<..>>>`
the drawing clones and build them on the first `prepare` from the format `DrawAt` names. A headless
test, which never builds a `Renderer`, then never opens a device. The `scene_3d` editor is the
worked example.

The DOM renderer cannot run a `Draw` and fills its rectangle with magenta. A crate that draws with
the gpu does so behind a feature that beui's `render` feature turns on, as beui-components-styled's
`gpu` does. `App::setup` hands an app the device for this (`guides/beui/apps.md`).
