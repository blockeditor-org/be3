# Beui

Beui is BE3's user interface toolkit. It is a solidjs-like framework: a
`Document` owns a persistent tree of nodes, `view!` builds that tree once, and
reactive signals update the nodes in place afterwards. There is no virtual DOM,
no diffing, and no per-frame rebuild. Building the view is not the render loop:
build it once, keep the `Document`, and call `Document::show` with it every
frame to lay out, dispatch input, expose accessibility, and paint.

The consequence worth internalising is that a component body runs **once**. The
closures and effects it leaves behind are what run again. Everything else in
this guide follows from that.

The quickest introductions are `crates/beui/examples/counter.rs` and the
component catalog in `crates/beui/examples/demo.rs`. The
[reactive guide](reactive.md) is the reference for signals, attribute syntax,
children and render props, controlled state, keyed lists, scopes, and context;
this guide is about using beui itself.

## The rules that matter most

These four cover most review comments on beui code.

### Every function that builds part of a view is a `#[component]`

If a function returns a `NodeId`, or a value built around one such as a
`CanvasItem`, annotate it `#[component]` and name it in `CamelCase`. The
attribute is not decoration. It gives the function its own reactive scope,
registered against the node it builds, so that removing that node disposes
exactly the effects the function created. A plain helper that
builds nodes leaves its effects in the caller's scope, where they outlive the
subtree they bind and panic with "node was removed" the next time one of their
inputs changes. `#[component]` is also what makes the function usable as a tag,
gives it `@test_id`, `@node_ref` and `@sizing`, and makes
`component_state`, `component_accessibility` and `component_size` available
inside it.

A component that returns its own type implements `ChildValue` to name the node
the scope hangs on, and `IntoChild` for the slot that takes it, as
`base/canvas.rs` does for `CanvasItem`.

A child needs no node at all. A `ChildValue` whose `anchor` is `None` keeps a
`ChildScope` field instead, which the component's scope is moved into, so
dropping the value disposes exactly the effects that building it created and
the owner tree does the rest. That is how an item made of data rather than
nodes — a label, a key, a callback — can still be a component, with its own
scope, context, memos and cleanups, and still be written as a tag. Such a
component has nothing for `component_state`, `component_accessibility`,
`component_size` or `component_rect` to watch, and `@test_id` and `@node_ref`
name a node it does not have, so all six panic rather than going quietly
nowhere. `unstyled::MenuItem` is one: a menu item is a label, a disabled flag
and its own submenu items, so a menu is written as tags and each row follows
the signals its tag was given, and `unstyled::ChoiceOption` is the same for the
options of a tab bar, a listbox, a radio group and a select. Declare the type
with `value_child_type!` rather than `child_type!`, which additionally says how
a run of it is kept, so a `show` or a `for_each` can build one.

Functions that build no part of a view are ordinary functions. Deriving a
colour from theme tokens and interaction state, mapping a value to a label,
reading state back out of a built node — write those as plain functions, as
`styled/checkbox.rs` does with `box_fill` and `checkbox_checked`.

### A component ends with one `view!` and nothing after it

The last line of a component is a single `view! {}` producing the node it
returns:

```rust
#[component]
fn LabeledValue(label: Prop<String>, value: Prop<String>) -> NodeId {
    let text = create_memo(move || value.get());
    view! {
        <List spacing=4.0>
            <Text string={label} />
            <Text string={text} />
        </List>
    }
}
```

Signals, memos, callbacks, and handle destructuring go above it. Nothing goes
below it, and there is no second `view!` earlier in the body: `view!` builds
nodes the moment it runs, so a subtree built into a local and then used
conditionally has already been added to the document whether or not it ends up
in the tree, and a subtree built in one place but parented somewhere else
obscures which component's scope owns it.

When part of the tree depends on something, express it in the view rather than
in Rust control flow around it. `Show` takes a condition and builds its child
lazily the first time it becomes true, `Dynamic` rebuilds a subtree when a value
changes shape, `Keyed` rebuilds only when its key changes, and `ForEach` keeps a
keyed child per item. A component that is genuinely two different trees is two
components with a `Dynamic` or a `Show` choosing between them.

None of those four builds a node of its own. They keep a run of children in a
slot of the parent they are written in, so their children are laid out by that
parent, with its direction, spacing and alignment, and the children written
around them keep their places however the run changes. That is why a `ForEach`
has no spacing of its own, why `@sizing` belongs on the rows rather than on the
`ForEach`, and why `@test_id` and `@node_ref` on one of them panics: there is
no node to name. Each needs a parent that keeps its children in slots - a
list, a `Scroll` or a `Canvas` - so a single-child slot like `Frame`'s takes a
`List` around one.

The exception is a component whose root is a base node it creates directly —
the base layer itself, where `List` calls `create_list` and binds setters with
effects. Above the base layer, one `view!` is the shape.

### Call components from `view!`, never their builders

`#[component] fn Button` also generates `ButtonBuilder` and a `Button()`
constructor returning it. That is the machinery `view!` writes into; it is not
an API to call by hand. Write

```rust
view! {
    <Button label="Save" variant=ButtonVariant::Primary on_click={save} />
}
```

and not `Button().label("Save").variant(...).build()`. The macro is what rejects
a missing required prop and a prop written twice at the line that wrote the tag
rather than from inside generated code, what routes `@test_id`, `@node_ref` and
`@sizing` to the right place, what enforces a component's child arity, and what
keeps render props unbuilt until the component calls them. Hand-written builder chains lose the
diagnostics, are invisible to the `view!` formatter that `./scripts/verify`
runs, and read nothing like the rest of the tree. The same applies to a
component you want to pass around: hand over a `Render`/`RenderFn` closure that
writes a `view!`, not a half-applied builder.

### Use fine-grained reactivity

Bind a signal or memo to a prop; do not read it while building and do not
rebuild a subtree to show a new value. A `Prop<T>` takes a plain `T`, a
`ReadSignal<T>`, or a `Memo<T>`, and the reactive forms install an effect that
writes just that one property when the value changes.

A `Prop<T>` is a handle rather than a value, so it clones for the price of an
`Rc` and a component that needs one in more than one place clones it with
`clone!` the way it would a signal:

```rust
let color = create_memo(clone!(value -> move || paint(value.get())));
view! {
    <Frame color>
        <Text string={value} />
    </Frame>
}
```

Do not wrap a prop in a `create_memo` just to read it twice. A memo adds a
node to the graph, and a `T` that is not `PartialEq` cannot go in one at all.

The common mistake is a list. Removing every row and building replacements on
each change throws away the nodes, their scopes, their measured text, and
whatever focus or caret lived in them, and costs time proportional to the whole
list for a one-item edit. `ForEach` keyed by item identity reconciles in place:
rows that stayed keep their nodes untouched, and only the bindings reading what
actually changed run.

```rust
<List spacing=8.0>
    <ForEach keys={items.keys()}>
        {move |id: Uuid| {
            let item = items.get(&id);
            view! { <ItemRow item /> }
        }}
    </ForEach>
</List>
```

The kind of child a run builds is the kind its parent takes, and the parent is
what decides it: the rows of a `ForEach` in a list are `ListChild`s and can
carry `@sizing`, the rows of one in a `Scroll` are plain nodes and are items of
that scroll, and the rows of one in a `Canvas` are `CanvasItem`s. A row builder
that returns the wrong kind for the parent is a compile error.

Key by identity, never by content: a key containing the row's text changes
whenever the text does, which destroys and rebuilds the row — exactly what
`ForEach` exists to avoid. Give the row the key and let it read its own item
signal. `KeyedStore` supplies both halves, one signal per item plus the order
signal, and `reconcile` writes only the items that differ.

The same instinct applies elsewhere. Derive with `create_memo` so a property
wakes only when the derived value changes. Split a struct held in one signal
with `#[derive(Store)]` so writing one field does not wake readers of the
others; `use_theme()` returns such a store, which is why a component binds
`theme.accent.clone()` rather than the whole theme. Use `create_selector` for
"am I the selected row?" so moving a selection wakes two rows instead of all of
them. Prefer `Show` over rebuilding, `Keyed` over `Dynamic` when only part of a
value decides the shape, and `VirtualList` for a collection large enough that
building every row is the cost.

## Component layers

Beui separates mechanism, behavior, and appearance. The dependency direction is
deliberate:

| Layer | Location and public path | Responsibility |
| --- | --- | --- |
| Base | `crates/beui/src/base`; re-exported from `beui::reactive` | Retained nodes for layout, painting, visibility, focus, pointer input, offset content, and text. |
| Unstyled | `crates/beui/src/unstyled`; `beui::unstyled` | Accessible interaction behavior composed from base components, without theme colors, typography, borders, or spacing. |
| Styled | `crates/beui/src/styled`; `beui::styled` | Application-ready controls that compose an unstyled control and paint its state with base components and `styled::theme` tokens. |

Styled components are composed out of unstyled ones, and unstyled ones are
composed out of base ones. Work at the highest layer that can express what you
need:

- **Adding a feature to the app?** Compose styled controls with base layout and
  painting primitives.
- **Need existing behavior with different appearance?** Use the unstyled
  component and paint it yourself. Do not reimplement focus, keyboard, pointer,
  touch, or accessibility handling in a styled component.
- **Need new behavior?** Add an unstyled component composed from base
  components.
- **Tempted to add a base component?** Almost always, add an unstyled one
  instead. The base layer is small on purpose — `Frame`, `List`, `Text`,
  `Offset`, `VirtualList`, `Canvas`, `Drawing`, `Overlay`, `Focusable`,
  `ClickCatcher`, `Embed`, `Portal`, `Viewport` — and it stays small because most things are
  compositions of those.
  Add a base component only when the retained tree genuinely lacks a primitive:
  a new way to lay out, paint, or receive input that cannot be expressed by
  arranging the existing nodes. `Portal` is one: it shows a subtree it does not
  own, which is how a node laid out in one place this frame is laid out
  somewhere else the next without being rebuilt. If a new concern can share `Frame`'s single-child box model,
  extend `Frame` rather than adding another pass-through node.

`Drawing` is the one base node that paints rather than arranges: it takes a
`Draw`, a callback handed the `Painter` and the rectangle the node was laid out
at. It is for content whose shape is computed rather than arranged -
`unstyled::TextArea` lays a syntax-highlighted document out itself, byte by
byte, and paints the result as four layers. Build the callback in a memo over the page it draws,
so the closure is replaced only when that page changes, and cull to
`painter.clip_rect()` inside it, so a document far taller than the viewport
costs the screenful it shows. Reach for it only when there genuinely is no
arrangement of nodes that says the same thing: a row of labels is a `List` of
`Text`, not a `Drawing`. It measures to nothing, so it takes its size from
whatever places it - a `Frame` with a width and a height, or a `CanvasItem`.

The galleys such a page paints come from `layout_text(text, font, layout)`,
which lays text out with the shown document's fonts and answers `None` until
the document has been shown once. It is how a component measures text outside
`measure` and `paint` - to work out where a caret sits, or how wide a column
is - and the galleys it returns are cached, so asking for the same word twice
costs a hash lookup. A `FontId` carries `bold` and `italic` alongside its size
and family; both are synthesised by FreeType rather than loaded as separate
faces, and `Text` takes them as props.

`unstyled::Button` shows the split. It composes `Focusable` and `ClickCatcher`,
and owns button semantics, disabled behavior, pointer and keyboard activation,
and accessibility. Its content closure receives a `ButtonHandle` of reactive
`hovered`, `active`, and `focused` state. `styled::Button` wraps it and uses
that handle to choose fills and paint a focus outline, so every visual treatment
sits on the same interaction behavior.

Pure presentation components such as styled text and cards compose base
components directly, because they have no interaction behavior to delegate.

The main base building blocks are `List`, `Frame`, `Text`, `Offset`, and
`VirtualList`. `List` is the only box that arranges siblings: it takes a
`direction`, which is vertical unless the tag says otherwise, an `align` for
the cross axis, and `spacing`. Nothing wraps it, so a row is written
`<List direction=Direction::Horizontal spacing=8.0>` and a row that centres its
children adds `align=Align::Center`; a one-line alias per combination is what
`Row`, `Column` and `CenteredRow` were, and reading the props beats remembering
which names exist. `Frame` combines optional sizing, an aspect ratio it centres
its box within, padding, fill, outline, and visibility on one retained node.
Its `width`, `max_width`, `height` and `aspect_ratio` each take an optional
measurement, so a signal behind one can hand it back to nothing and leave that
axis measuring intrinsically again.
`Text` carries its own decoration too: `underline` is painted from the galley's
baseline, so switching it on never moves anything. `Portal` shows a subtree that belongs to
someone else: it takes a `NodeId`, lays it out and paints it where the portal
stands, and leaves it alone when the portal goes away, so a subtree can move
between places in the tree without being built again. Exactly one portal shows
a given node - claiming it takes it from the portal that had it - and the
subtree is kept alive by whoever built it, with `in_new_scope` or a scope of
their own, until they remove it. `Embed` reserves a rectangle
for something outside the document — an editor the host composites behind the
surface — publishing the rectangle and the clip it was laid out in through the
`EmbedSlot` it was given and cutting that rectangle out of the surface so what
is behind shows through. `punch=false`
keeps the surface whole, for something the host draws over it instead.
`Viewport` reserves a rectangle the renderer draws into rather than the
document: it fills the space it is given and paints the `Drawing` it is handed
in its place (see [Draw with the gpu](#draw-with-the-gpu)).
`Offset` keeps a run of items along a `direction` and lays them out from an
offset; it answers no input at all, so nothing scrolls by putting one in a view
(see [Scrolling](#scrolling)). `VirtualList` is an ordinary box that stands for
`count` items of an estimated `item_size` and builds only the ones its slice of
the viewport reaches (see [Long lists](#long-lists)).
`Scroll` takes a `direction`, so the same tag is a
column of rows or a strip of cards; a horizontal one answers Shift+wheel, a
sideways trackpad swipe, a touch drag and the left and right arrows.
A plain wheel is left to whatever is around it, the way a browser leaves a
horizontal strip alone, and a wheel only ever reaches the innermost scroll
under the pointer. The unstyled module contains
`Button`, `Pressable`, `Toggle`, `Choice`, `Slider`, `TextInput`, `TextArea`,
`Disclosure`, `Tree`, `Select`, `ContextMenu`, `MenuButton`, `Container`,
`PanZoom`, `PointerLock`, `Dock`, `Tooltip`, `Floating`, `Scroll`, `Scrollbar`,
and `Stack`. `TextArea` is the multiline one: it owns a
`text_editor_core::Core` through the `TextAreaState` its caller holds, lays the
document out with a gutter, wrapping, collapsible sections and markdown
checkboxes, and reserves room for the inline and block `TextWidget`s the caller
names - which is how a block editor puts an embedded block inside the text and
drives the same document from a toolbar of its own. `MenuButton` is the button that opens a menu under itself, which is
what a toolbar reaches for where `Select` would imply the choice sticks;
`ContextMenu` is the same menu on a secondary press, and it also takes an
`open_at` point so a touch gesture can raise it where the finger was.
The styled
module supplies themed buttons, icon buttons, menu buttons, links, text styles,
cards, checkboxes, switches, choices, text and number inputs, a multiline text
editor with its find and replace bar, menus, tabs, trees,
progress, scrolls and scrollbars, tooltips, a docking workspace, and responsive layout.
`Separator` is the rule between them: it runs `Direction::Horizontal` unless the
tag says otherwise, takes a line's thickness across its `direction` and the
space its list gives it along it, so a divider in a row is
`<Separator direction=Direction::Vertical />` and neither needs an `@sizing`.
A `length` pins the long axis for a row that centres its children rather than
stretching them. A control that can be turned off -
`Button`, `IconButton`, `Link`, `Checkbox`, `Select`, `TextInput`,
`NumberInput` - takes a `disabled` prop: it stops answering the pointer and the
keyboard, leaves the tab order, publishes itself as disabled to a screen
reader, and paints in muted colours, which is what a read-only editor binds
`editor.read_only()` to rather than leaving a live control that quietly throws
edits away. The re-exports in `unstyled.rs` and `styled.rs` are the authoritative
lists.

### Scrolling

Scrolling exists at all three layers, and app code wants the styled one.
`styled::Scroll` is a scroll with the project's
scrollbar already beside it, so a panel that scrolls is one tag:

```rust
view! {
    <Scroll @sizing=ItemSize::Percent(100.0)>
        <Rows />
    </Scroll>
}
```

The focus ring follows the theme's accent unless `focus_color` names another,
and `on_change` still reports the position for anything else that wants it.

### Long lists

A `VirtualList` is a box like any other. It reports `count * item_size` as its
own length and builds only the rows that its slice of the enclosing viewport
reaches, so it goes wherever a tall child would: directly under a `Scroll`,
beside plain siblings in one, or nested a few containers deep inside one.

```rust
view! {
    <Scroll @sizing=ItemSize::Percent(100.0)>
        <Header />
        <VirtualList count={rows} item_size=ROW_HEIGHT>
            {move |index: usize| view! { <Row index /> }}
        </VirtualList>
        <Footer />
    </Scroll>
}
```

`item_size` is an estimate, and a row that measures differently is laid out at
the size it measured. The list remembers what each row it has built actually
measured and reports the sum, so its length grows towards the truth as rows come
into view and the last row can always be scrolled to. A row that is measured
above the viewport would move everything below it, so the list keeps drawing
from the row it drew from last frame and asks the scroll around it to move its
offset by the difference instead; the scroll applies that before it places
anything, so nothing on screen shifts. Outside a scroll a `VirtualList` builds
what fits in the box it is given, which is how one sized by a `Stack` builds a
screenful in one layout and a hundred pixels' worth in the next.

The rows are keyed by index, so scrolling reuses the rows that stay in view and
disposes the effects of the ones that leave. Changing `count` or `item_size`
rebuilds them.

`unstyled::Scroll` is the same arrangement without the appearance: it owns the
base offset, the input that drives it, the position it reports, and the list
that puts the bar on the scroll's cross axis, and it takes a `ScrollbarStyle`
saying what to put there.
That is the seam the styled layer fills, with a spacing and a builder that is
handed a `ScrollHandle` of the live `position`, the `direction`, and a
`scroll_to` that drives the offset the way the wheel and the arrow keys do:

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

The bar answers the pointer itself. `unstyled::Scrollbar` owns that: it takes
the position, the direction and an `on_scroll_to`, hit-tests the press against
the thumb it would paint, drags the thumb with the pointer, pages by a viewport
towards a press on the track either side of it, and hands its content a
`ScrollbarHandle` of `hovered` and `dragging` so the styled bar can paint those
states. `thumb_start` and `thumb_length` are the same fractions both layers
work in, so what is painted and what is pressed cannot drift apart. A press on
the bar rests whatever momentum a fling left, so the content stops where it is
put.

Without one the scroll shows no bar, which is what the unstyled layer does on
its own. A control that scrolls something of its own takes a `ScrollbarStyle`
and passes it down, the way `unstyled::Select` hands one to the scroll behind
its options, so the styled control decides the bar and the unstyled one never
names a colour. The gutter is always reserved, and the bar paints nothing while
its content fits, so a scroll that grows past its viewport does not shift the
content beside it.

Under both sits the base `Offset`, which is named for what
it does rather than for what it is used for: it holds a run of items along a
direction and lays them out from an offset, with no bar, no theme, and no input
of their own. A wheel, a touch drag and the arrow keys are `unstyled::Scroll`'s,
which wraps the offset in a `Focusable` for the keys and a `ClickCatcher` for
the wheel and the drag, keeps the momentum an unfinished fling carries, and
drives the offset from all three. So reach for `Offset` when something needs its
content shifted under a viewport and nothing more, and for a `Scroll` whenever
something needs to scroll.

That offset is anchored to a node, not measured from the top of the content:
the scroll remembers the item at the top of its viewport and the distance it
starts above the edge, so a row further up growing or shrinking - a wrapping
label, an image that finished loading - leaves what is being read exactly where
it is.

### Slider scales

A slider spreads its range evenly along its track unless it is given a
`scale`. `SliderScale::Midpoint(value)` curves it so that the centre of the
track reads that value, which is how a control whose interesting values are
bunched at one end gets most of the track for them. The curve is exponential
rather than a power of the position, which is what keeps the fine end useful
without giving the whole of it away: the inspector's blur slider runs to
120 px with a midpoint of 12 and reads 3 px, 12 px and 39 px at the quarters
of its track, spending a tenth of the track below one pixel where a power
curve through the same midpoint spends a quarter of it there. The midpoint may sit
above the centre of the range as well, which gives the top end the fine part
of the track instead. A midpoint outside the range, or one that lands where
the centre already is, leaves the slider linear.

The curve applies everywhere the value and the track meet. Dragging maps the
position under the pointer through it, the knob sits where the value falls on
it, and keyboard steps move by a share of the track rather than a share of the
range, so an arrow key near the fine end moves a little and the same key near
the coarse end moves a lot. What a screen reader is told the step is follows
the value the next step would actually reach.

### Tooltips

`unstyled::Tooltip` shows something after the pointer has rested on its child
for a moment, and `styled::Tooltip` is the bubble version of it:

```rust
view! {
    <Tooltip label="Reveal the block being shown">
        <IconButton glyph={ICON_MY_LOCATION.to_owned()} label="Reveal" on_click={find} />
    </Tooltip>
}
```

`IconButton` already wraps itself in one, so an icon-only button says what it
is without the caller doing anything: the `label` it publishes to a screen
reader is the text the bubble shows. Wrap anything else whose meaning is not
on screen - a bare glyph in a row, a truncated cell - in a `Tooltip` of its
own.

The bubble lives in a passive `Overlay`. It paints above everything, and
unlike a menu or a dialog it takes no input at all: it is not in the overlay
stack, so the document under it keeps answering the pointer, Escape still
reaches whatever it was going to reach, and clicking the control the tooltip
describes clicks the control. The dwell is measured with `each_frame`, which
runs a callback inside the document's reactive scope once per frame and is
disposed with the scope that registered it.

### Overlays, and things that float

An overlay is laid out and painted above the rest of the document rather than
among it, and it comes in three modes.

A **modal** one - a menu, a select popup, a dialog - takes the document over
while it is open: it goes on the overlay stack, so input reaches it and
nothing else, it can trap focus, Escape closes the topmost one, and a press
outside it dismisses it.

A **passive** one takes no input at all. It is painted above everything and is
not on the stack, so the document underneath goes on answering the pointer and
the keyboard as if it were not there. A tooltip is passive: hovering the thing
it describes must not become hovering the tooltip.

A **floating** one is painted above everything and answers the pointer where it
actually is. Only the part of the document it covers is shut out, so a press
that lands on it does not also land on what is underneath, and everything
around it stays live. `unstyled::Floating` is the one to reach for: it pins its
child to the top or the bottom edge of a node named by `NodeRef`, taking no
space in the layout around it.

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

A control that has to win a press from a surface around it - an add button in a
row that is itself a button, a close cross on a tab - asks `unstyled::Button`
for `capture_presses` instead. The captured press reaches that button and
nothing else, so the row it sits in does not open as well.

`unstyled::Tree` makes the same split the other way round. A row is a tab stop
with the tree's keyboard and its `TreeItem` accessibility, and nothing more:
where a pointer has to land to select it is the face's business, because a
face that draws a chevron of its own outside the name wants pressing the
chevron, the indent beside it and the name to mean three different things. The
handle carries `select`, `toggle` and `hover` for the face to call from
wherever it decides they belong.

`styled::Tree` is the face that split was made for, and the one app code
reaches for. It draws the indent, a chevron that is a button of its own -
tooltipped, labelled for a screen reader, and outside the row's highlight, so
pressing it expands the row without opening it - and the highlighted row
beside them, and hands its content builder a `TreeRowFace` carrying the row's
`key`, `item`, `selected`, `focused` and `hovered`. It owns the drag gesture
too: a press that travels further than a few pixels reports `on_drag_start`
and the click it would otherwise have become is dropped, so a row can be
dragged out without selecting what it left behind. `row_test_id` names each
row for tests, as `<id>.row` and `<id>.chevron`, and `outline` gives a single
row an outline of its own, which is how a drop target says whether it will
take what is over it.

### Docking and windows

`styled::DockArea` is the workspace layout: panes split from one another, a tab
bar on each pane, and tabs that can be dragged between panes or out into
windows that float over the rest of the dock. `unstyled::Dock` underneath it
owns the tree, the dragging and the keyboard, and paints nothing;
`crates/beui/examples/dock.rs` is the worked example, run with
`cargo run -p beui --example dock`.

The layout is a `DockState`, which the caller keeps in a signal and hands back
when the dock reports a change, the way `PanZoom` takes its camera:

```rust
let (layout, set_layout) = create_signal(DockState::new([TabId::new(1)]));
view! {
    <DockArea
        state={layout}
        title={Func::new(move |tab: TabId| title_of(tab))}
        closable={Func::new(|tab: TabId| tab != FILES)}
        on_change={move |next: DockState| set_layout.set(next)}
        on_close={move |tab: TabId| forget(tab)}
    >
        {move |tab: TabId| view! { <Panel tab /> }}
    </DockArea>
}
```

A tab is a `TabId` the caller mints, so whatever the tab stands for - a block,
a file, a tool - stays the caller's. The dock asks for a title, hands the
`TabId` back to the `content` builder for the panel to show, and reports the
tabs it removes through `on_close` so the caller can drop what it was holding.
Because the state is a plain value, the caller opens, closes, splits and floats
by writing it: `show`, `push`, `push_to_focused`, `split`, `remove`, `replace`
and `drop_tab` are the whole vocabulary, and `find`, `all_tabs`, `focused_tab`
and `surface_tabs` read it back.

Each surface - the main one and one per window - lays its tree out over the
rectangle it was given, so panes and the bars between them are canvas items at
computed rectangles rather than nested boxes. `layout_surface` is that
calculation on its own, which is what the drop targets and the tests are
resolved against. A window is a floating overlay, so it paints above the dock
and takes the pointer where it actually is, and the document underneath it is
shut out rather than answering a press through it.

Dragging a tab picks a drop target from what is under the pointer: a tab bar
inserts it between the tabs there, the middle of a pane joins that pane, and an
edge of one splits it. Holding Alt while dropping floats the tab into a window
instead, which is also what "Pop out into a window" on a tab's own menu does. A window
holds one pane, so a tab dropped anywhere inside one joins it rather than
splitting it, and the pane's tab bar is the window's title bar: a grip, the
tabs, and the button that closes it. Anywhere on that bar that is not a tab
drags the window, so the grip and whatever room is left beside the tabs are
both handles. Windows resize from any of
their eight grips and are raised by whatever takes the focus inside them. Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of the pane the focus is in,
registered with `on_shortcut` so they arrive even from inside a text input in a
panel. The bar between two panes is a tab
stop with a `Splitter` role: the arrow keys move it, and the tab bar is a
`Choice` inside a horizontal `Scroll`, so the arrows, Home and End walk it like
any other tab list and scroll the tab they reach into view when a pane has more
tabs than it has room for.

A tab's panel is built the first time the tab is shown and belongs to the dock
rather than to the pane showing it: the pane holds a `Portal` pointed at it, so
the panel keeps its nodes, its scroll position, its caret and its state when
the tab is hidden behind another, dragged to another pane, or floated into a
window. A panel no pane is showing is laid out by nobody, so it costs nothing
and a screen reader does not read it. Closing the tab is what removes it.

### Pan and zoom

`unstyled::PanZoom` turns wheel, trackpad and middle-button gestures over a
rectangle into camera movement, and owns none of the camera itself. The caller
keeps a `PanZoomView` - the world point shown at the middle of the viewport and
a scale - passes it in, and writes back what `on_change` reports:

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

A scroll pans, Shift+scroll pans sideways, dragging with the middle button or
with two fingers pans, and Ctrl+scroll, a trackpad pinch or a two-finger pinch
zooms around the pointer or the point between the fingers. It is a tab stop as
well: the arrows pan it, `+` and `-` zoom around the middle of the viewport,
and `0` returns the scale to one. The handle carries the camera as a
`CanvasView` ready for a `<Canvas>`, the `scale` for content that should grow
with the zoom, whether a pan is in progress, and whether it has focus, which is
what a caller paints a focus ring from. Because the view is the caller's, a toolbar button, a fit command or
a host that syncs several editors writes the same signal the gestures do; the
camera a plugin editor is handed (guides/pan_and_zoom.md) reaches a beui editor
the same way. Anything between the gesture and the camera - momentum, snapping,
clamping the camera to the content - belongs to the caller, apart from the
scale limits `min_scale` and `max_scale`.

### Hold the pointer

A control that looks around rather than pointing at something - a first-person
scene, a modeller's orbit - is `unstyled::PointerLock`. It is a tab stop that
takes the pointer when it is pressed, and while it holds it the pointer stops
moving, is hidden, and reports how far it moved instead of where it is:

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

The lock is controlled state, like a `Toggle`'s: the control follows the
`locked` prop and reports what it wants through `on_change`. It lets go on
Escape, when it loses focus, and when it is removed, and keys it does not use
itself go on to `on_key`, which is what a control that also walks with the
keyboard binds.

Beui does not hold the pointer itself - it asks. `Document::show` publishes the
wish as `FrameOutput::pointer_locked`, and whoever is showing the document does
it: the winit runner locks the cursor and hides it, and the plugin framework
asks the host to (guides/adding_a_plugin_editor.md). What comes back is
`Event::PointerMotion`, a delta with no position, which the document delivers to
the focused node while the pointer is held and nowhere at all while it is not.
So a control that is not the one holding the pointer never sees motion, and a
window that loses the keyboard gives the pointer back.

### Draw with the gpu

A viewport whose pixels no arrangement of nodes can produce - a 3D scene, a ray
tracer, a map - is a `Viewport` node holding a `Drawing`. `Viewport` fills the
space it is laid out in and paints a `Shape::Drawing` over its rectangle;
the renderer runs the `Draw` behind that drawing where the shape sits in the
order everything else is painted in, so nodes written after it still paint over
it.

```rust
view! {
    <Viewport drawing={drawing} @sizing=ItemSize::Percent(100.0) />
}
```

`Draw` has the two halves a gpu frame has. `prepare` is handed the device, the
queue and an encoder, and is where the passes of its own go - rendering into
its own textures, writing its own buffers. `paint` is handed the render pass
beui is painting the document into, and draws there. Both are handed a `DrawAt`
carrying the rectangle and the clip in physical pixels, the size of the whole
target, the scale, and the target's texture format, which is what a pipeline
built on the first frame is built against. A `Draw` must leave the pass's
viewport and scissor as it found them: position what it draws from the
rectangle it was given, the way beui's own shader does, rather than by setting
a viewport.

`Drawing::new` wraps a `Draw`, and two drawings are the same shape when they
are the same `Rc`. That is what decides whether the frame changed, so a drawing
whose contents have moved is a new `Drawing` and a frame that is showing the
same thing keeps the one it had, which is what leaves an idle editor idle. The
gpu resources belong to the `Draw` rather than to the drawing: keep them in an
`Rc<RefCell<Option<..>>>` the drawing clones, build them on the first `prepare`
from the format `DrawAt` names, and a headless test - which never builds a
`Renderer` - never opens a device at all. The `scene_3d` editor is the worked
example.

## Use beui in a standalone app

The default `beui` feature is `window`, which includes the winit runner and the
wgpu renderer. A standalone app builds its document once and implements
`beui::App`:

```rust
use beui::reactive::{
    Frame, List, build, component, create_memo, create_signal, view,
};
use beui::styled::{Button, ButtonVariant, Display, use_theme};
use beui::{App, Color32, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("Counter", CounterApp::new())
}

#[component]
fn Counter() -> NodeId {
    let (count, set_count) = create_signal(0i64);
    let decrease = set_count.clone();
    let label = create_memo(move || count.get().to_string());
    let theme = use_theme();

    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=8.0>
                <Display content={label} />
                <Button
                    label="Decrease"
                    variant=ButtonVariant::Secondary
                    on_click={move || decrease.update(|value| *value -= 1)}
                />
                <Button
                    label="Increase"
                    variant=ButtonVariant::Primary
                    on_click={move || set_count.update(|value| *value += 1)}
                />
            </List>
        </Frame>
    }
}

struct CounterApp {
    document: Document,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            document: build(|| view! { <Counter /> }),
        }
    }
}

impl App for CounterApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }
}
```

Run the repository examples with:

```text
cargo run -p beui --example counter
cargo run -p beui --example demo
```

Beui has three feature levels:

- No features provides the document, components, layout, input model, and
  painting output. This is enough for headless logic tests.
- `render` adds the wgpu renderer without creating a window. Embedded hosts use
  this level.
- `window` adds the desktop runner and enables `render`; it is the default.

### The inspector

Ctrl+Shift+I in a standalone beui window opens the node, accessibility, and
performance inspector. Ctrl+Shift+C enables node picking. Ctrl+Shift+F moves
keyboard focus into the panel and back out again, and Escape inside the panel
returns focus to the document, so the whole inspector is reachable without a
mouse. Its tree rows select and expand together: clicking a row, or pressing
Enter or Space on it, selects the node it lists and opens or closes its
children, and the arrow keys walk the tree.

The inspector's Sim tab converts one input device into the other, so a pointer
device can drive touch behavior and a touchscreen can drive pointer behavior.
Both run inside `Document::show`, so they work the same in a standalone window
and in a beui block editor plugin.

"Emulate touch with mouse" turns mouse presses into touch events.

"Simulate mouse with touch" turns the whole shown rectangle into a trackpad and
paints a cursor the document reacts to. One finger moves the cursor, a tap
clicks it, a tap followed by a press and drag drags with the primary button, and
two fingers scroll smoothly. The strip along the bottom holds the left, middle,
and right mouse buttons plus a keyboard toggle: a button stays held for as long
as its finger is down, another finger can work the trackpad at the same time,
and swiping up or down on the middle button scrolls a wheel tick at a time. The
keyboard toggle opens an on-screen keyboard that sends `Event::Key` and
`Event::Text`; its Shift, Ctrl, and Alt keys latch until the next key, and they
also apply to clicks, so Ctrl+Shift+I on it reopens the inspector.

The strip and the keyboard take their room out of the window rather than
covering it: `Document::show` asks the simulation how tall it is, trims that off
the bottom, and lays the document and the inspector panel out in what is left,
the way a phone's keyboard pushes a page up. Nothing is drawn over content that
is still live, so the bars are opaque.

### Filters

The Sim tab's Filters section puts a blur, a contrast reduction, and the
colour vision simulations over the shown rectangle. They are independent of the
screen reader simulation and of each other, and they need no cooperation from
the document: they are a post-processing pass in `Renderer`, so whatever a
document paints - a plugin's beui pane included - is filtered the same way.

`Context::apply_filter` is what turns them on. It records a `Filter` (the region
in points, a blur radius, a contrast multiplier, and a `ColorVision`) and the
number of shapes painted so far, which splits the frame in two: everything
painted before the call goes through the filter, everything painted after it
lands on top of the result untouched. The inspector calls it once a frame,
after the document, the panel, the overlays and the screen reader's focus
outline, and before the reader's readout - so the reader's outline blurs with
the page it marks while the words it is saying stay readable. The region is the
shown rectangle rather than the whole window, so the panel holding the sliders
is never filtered.

The blur is a dual Kawase chain: the frame is halved down a level at a time,
then tented back up, with the number of levels taken from the radius. A radius
of 120 points costs no more than a radius of 8, so the slider can go as wide as
it likes without the frame rate following it. The radius reads like CSS
`blur()`: the light from an edge reaches about three times it. Sampling is
clamped to the filter's region at every level, so nothing outside it bleeds in.

Contrast and the colour vision matrices run in the same pass that composites the
blur. The matrices are the Viénot, Brettel and Mollon linear-RGB
approximations, applied in linear light; the contrast reduction pulls toward mid
grey in gamma space, the way CSS `contrast()` does.

A filter keeps the damage rectangle the rest of the frame is drawn from, so a
filtered frame costs no more to repaint than an unfiltered one. Contrast and
colour vision are per-pixel, so they need nothing beyond the region that
changed; a blur spreads light out of it, so `Prepared::widen` grows the damage
by the chain's reach - the sum of what every pass can move a sample - clipped to
the filter's own region. The scene texture and the blur chain are retained the
way the frame is, and every pass is scissored to the widened rectangle: what
lies outside it was left correct by the frame before, because the reach bounds
what a change can touch at every level. `Renderer::prepare` returns the repaint
it settled on, and it upgrades a partial one to the whole frame when the filter
itself changed - a new radius or a filter switched off restyles everything the
region covers, damage or no damage.

### The screen reader simulation

"Simulate a screen reader" at the bottom of the Sim tab hands the document over
to what a screen reader would say. A readout takes a fixed strip off the bottom
of the shown rectangle, above the mouse simulation's own bar when both are on,
holding the last thing the reader said and how far through the document it is.
It is a strip the document does not get rather than a sheet over it, so the
height stays the same whatever the reader says - a bar that grew with the text
would relay out the page on every utterance - and a long phrase is clipped
rather than wrapped. The highlight marking where the reader is paints over the
document under the filters, so turning the blur or the contrast reduction up is
what leaves the shape of the page with nothing on it to read; the readout is
outside the filtered region and stays legible however far they go.

Nothing about the simulation reads the beui tree. It walks the AccessKit tree
the document publishes every frame, in reading order, and it changes the
document only by sending `ActionRequest`s back through
`Context::accessibility_action` - the same path a platform screen reader uses.
An item is anything with a name of its own, anything that answers Click, Focus,
Increment or `SetValue`, and any scroll area; a control's own text is folded
into its name instead of being read separately, the way a platform computes one.
What gets spoken is the name, the role, the value when it differs from the name,
and then the state - checked, expanded, selected, a slider's percentage,
"dimmed" for a disabled control. A control with nothing to name it is announced
as its bare role, which is the point: "button" on its own is the bug.

While the simulation is on the document answers no pointer or keyboard input
directly, so a click lands nowhere and the only way through the UI is the
simulation. Keyboard and touch drive it at the same time, with no mode to pick
between them. From the keyboard, the left and right (or up and down) arrows walk
an item at a time, Tab and Shift+Tab move between controls, Home and End jump to
the ends, Enter or Space activates, Minus and Plus adjust, Page Up and Page Down
scroll, and R repeats the current item. Walking past either end says so and
reads the item again after it, so the readout never leaves you without the thing
you are standing on. By touch, dragging a finger reads
whatever is under it, flicking left or right moves an item at a time, flicking
up or down adjusts a value, a double tap activates, two fingers tapping repeats,
and dragging two fingers scrolls; turning "Emulate touch with mouse" on as well
is what makes those gestures reachable from a mouse. The same commands sit in
the panel as buttons, so the whole simulation can be driven without either
device, and the Sim tab lists them under those buttons.

Ctrl+Shift+F still parks keyboard focus in the panel, and Escape returns it; the
panel does not otherwise hold focus while the simulation is on, so clicking one
of the command buttons leaves the keyboard commands working.

## Write views

Import `view!`, `#[component]`, signals, and base components from
`beui::reactive`; styled controls come from `beui::styled` and behavior-only
ones from `beui::unstyled`.

`view!` supports three framework attributes on every tag, in their own `@`
namespace so a component can name its props whatever it likes:

- `@test_id` gives a node a name for headless interaction tests. It takes a
  `Prop<String>`, so a row whose identity is itself reactive can carry one:
  bind a memo and the name follows it, and the name it left behind stops
  resolving.
- `@node_ref` fills a `NodeRef` when enclosing code genuinely needs the
  resulting `NodeId`.
- `@sizing` selects the child's `ItemSize` among its siblings in a list, and
  only a list accepts it. Children are intrinsic by default; fixed children
  reserve a logical-point size, and percent children share the remaining
  bounded space by weight. On the single root of a `view!` it builds a
  `ListChild`, which is what the row builder of a `Dynamic`, `ForEach` or
  `Keyed` returns.

A children slot names the type of child it takes, which is what confines
`@sizing` to a list. `children: Children<ListChild>` takes any number of
children that each carry an `ItemSize`, and `List` and `Stack` are written that
way; `children: Children<NodeId>` takes any
number of plain nodes, as `Scroll` does; `children: Children<CanvasItem>` takes
only the items a `Canvas` can place; `children: Child` and
`children: Option<Child>` take one node. A plain node converts into whatever a
slot asks for that takes one, so `<List><Text content="hi" /></List>` needs no
ceremony. Writing
`@sizing` on the child of a slot that does not size its children is a compile
error rather than an attribute that quietly does nothing.

A `Vec` of children does not fill a slot. Children are written out as tags, and
however many of them a collection asks for comes from a `ForEach` over its keys
inside the same `view!`, so the sizing, the keying and the reconciliation all
stay in one place:

```rust
<List spacing=4.0>
    <Heading content="Cards" />
    <ForEach keys={card_ids()}>
        {move |id: Uuid| view! { <CardRow id /> }}
    </ForEach>
</List>
```

A slot holds its children in runs rather than one flat list, so a child can
stand for none, one or many of them and change how many as it goes: that is how
a fragment written among siblings takes the places between them, and how
`Show`, `Dynamic`, `Keyed` and `ForEach` fill a parent they do not own. `List`,
`Stack`, `Scroll` and `Canvas` all keep their children that way.

Use `Frame`'s `width` and `height` props to constrain a component's own size,
`max_width` for a box that fills the room it is given but stops at a limit, and
`@sizing` to describe how it participates among siblings in a `List`. Say a
width once: a `Frame` nested inside one that carries the width
is laid out within it, so the outer box is the only place the number belongs.
`Container`, `narrower_than` and `shorter_than` provide container-responsive
state; `unstyled::Stack` and `styled::Stack` switch between a row and a column
without rebuilding their children.

### One layout per frame

A frame is input, then layout, then paint. Input is dispatched against the rects
the previous frame painted, which is what the reader was looking at when they
clicked, and the tree is laid out exactly once afterwards.

That holds even though `component_size` and `component_rect` feed measurements
back into the tree, because both are delivered during the layout walk rather
than after it. `component_size` reports the space a component's parent offered
it, handed over before the component is measured; `component_rect` reports where
it was placed, handed over before its subtree is laid out. Both flow downward,
so a component that rebuilds from either is rebuilt before anything under it is
placed, and the pass that caused the change absorbs it.

The contract this rests on is that laying a node out only ever changes that node
and the tree below it. Changing an ancestor or an already-placed sibling would
leave that node holding a rect computed from a tree that no longer exists; a
debug assertion catches it. The practical form of the rule is the one container
queries on the web settle on: a component may query the space it was given, but
it must not be the thing that decides that space on the axis it queries. A
`Container` sized by its own contents on the axis it reports is a cycle, and
beui resolves it in favour of the constraint its parent offered.

A signal written while the tree is being laid out has to take effect before the
walk moves on, so anything a node writes as it is laid out — a `Scroll`
reporting its position to a sibling scrollbar, a row built into a virtual list —
is written inside `settle`. A write left queued lands at whatever the next
`settle` happens to be, by which time the node that reads it may already have
been placed, and it is then laid out from the previous frame's value. The layout
walk settles after every node and blames the node that left something queued.

### Update a document from outside its events

Component callbacks run with their `Document` installed, so signal writes from
clicks and key events need no special handling. A host-driven update happens
outside that context and must enter the document's reactive scope:

```rust
use beui::reactive::{WriteSignal, with_reactive_scope};
use beui::Document;

fn set_value(document: &mut Document, value: &WriteSignal<String>, next: String) {
    let value = value.clone();
    with_reactive_scope(document, move || value.set(next));
}
```

Keep the write handles the host needs next to its `Document`. Do not rebuild the
document to display new data.

## Use beui in a block editor plugin

A beui editor is a `#[component]` function. It implements
`block_editor_plugin::BeuiApp` and uses `block_editor_plugin::beui_plugin!`
instead of the egui `App` and `plugin!`. The type it names holds no state: the
framework builds the view once, keeps the `Document` it produced, and shows it
every frame.

```rust
#[component]
pub fn Counter(editor: block_editor_plugin::Editor) -> NodeId {
    let counter = editor.block::<CounterBlock>();
    let count = counter.project(CounterBlock::count);
    view! { ... }
}

pub struct CounterApp;

impl block_editor_plugin::BeuiApp for CounterApp {
    fn view(editor: block_editor_plugin::Editor) -> NodeId {
        view! {
            <Counter editor={editor} />
        }
    }

    fn create_block(creation: &block_editor_plugin::Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(CounterBlock::default()).id())
    }
}

block_editor_plugin::beui_plugin!(CounterApp, "../manifest.json");
```

`Editor` is everything the instance was given: the host, the runtime's client,
the block, the view the host is showing the content through, and
`each_frame(...)` for work that is neither a block projection nor a signal. The
host supplies input, fonts, clipboard integration, rendering, and the frame
rectangle. A plugin normally depends on beui without the window runner:

```toml
beui = { path = "../../beui", default-features = false, features = ["render"] }
```

Block data reaches the view through `block-reactive`: `BlockSource::new` watches
a block, `project` and `project_keyed` derive signals from its current value, and
one `pump()` at the top of the frame — inside the document's reactive scope —
re-derives them. See the [reactive guide](reactive.md#blocks).

The counter editor under `crates/editors/counter` is the reference integration.
The [plugin editor guide](adding_a_plugin_editor.md) covers the manifest,
creation flow, host connection, and current beui plugin capability limits.

A plugin with `"creation": "Dialog"` implements `creation_view` instead, one
more `#[component]` function that the framework builds a separate document of
and shows in the host's creation dialog. It says what the dialog makes with
`creation.on_create(...)` and answers `creation.set_ready(true)` once it has been
filled in. Host services such as `BlockPicker` work there in the same way they
do from an egui creation UI, polled from `creation.each_frame(...)`.

## Develop an unstyled component

An unstyled component owns semantics and interaction, not appearance. Put it in
`crates/beui/src/unstyled/<name>.rs`, declare it in `unstyled.rs`, and re-export
the public component, handles, state readers, and supporting types there.

Compose it from base components. For an interactive control this normally means:

1. Model controlled values and transient interaction values with signals.
2. Use `Focusable` for tab order, keyboard events, activation, and focus state.
3. Use `ClickCatcher` for pointer and touch interaction.
4. Publish the correct AccessKit role and state with `component_accessibility`.
5. Give the caller a `Render<Handle>` or `RenderFn<Handle>` containing the
   reactive state needed to paint the control.
6. Return the root base node directly so component state and framework slots
   attach to the node callers receive.

A catcher takes the wheel with `on_scroll` and a touch drag with
`on_scroll_drag`, and `scroll_axis` names the axis it takes them along, so a
vertical wheel over a horizontal strip passes through to whatever is around it
and a drag reaches the innermost catcher that scrolls that way.
`unstyled::Scroll` is built out of those three.

A press normally reaches every `ClickCatcher` under the pointer. A control that
must win a press, or that reacts to presses outside its own rect, captures it:
`capture_presses` claims presses inside the catcher and `capture_at` claims
presses at positions its callback accepts. Before any node handles a press the
document asks the topmost nodes first, and only the captor receives it: focus
stays where it is, touch scrolling does not start, and no other catcher arms.
Paint such parts with `Painter::on_top`, which draws above the rest of the
document, or of the overlay being painted. The touch selection handles of
`unstyled::TextInput` use both.

Do not put theme colors, fixed visual spacing, typography choices, or decorative
shapes in this layer. A new skin should be able to use the unstyled control
without undoing visual decisions.

State that belongs to the component's own handlers should be captured directly.
Use `set_component_state` only when tests or host integration need to read the
state from the component's `NodeId`, and expose a focused helper such as
`toggle_checked(&Document, NodeId)`. Controlled state must listen to its prop
and report user changes through its callback; see `unstyled::Toggle` and
`unstyled::TextInput` for the established pattern.

## Develop a styled component

Put a styled component in `crates/beui/src/styled/<name>.rs`, declare it in
`styled.rs`, and re-export its public API there. An interactive styled component
wraps the matching unstyled component, supplies its accessibility label when
needed, and renders the unstyled handle with base visual primitives:

```rust
#[component]
pub fn Checkbox(label: Prop<String>, checked: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    view! {
        <Toggle checked on_change={move |checked| on_change.call(checked)}>
            {move |handle: ToggleHandle| {
                view! {
                    <CheckboxFace handle label />
                }
            }}
        </Toggle>
    }
}
```

The face component derives colors and visibility with memos over
`handle.checked`, `handle.hovered`, `handle.active`, and `handle.focused`, then
composes `Frame` and `Text`. Keyboard and pointer handling stay in the unstyled
control. `styled/checkbox.rs` is a short, complete example of the pair.

Read colors from the nearest theme with `styled::use_theme()`, which returns a
`ThemeStore` — one `ReadSignal` per token. Bind a token straight to a prop with
`theme.accent.clone()`, or read tokens inside a memo that also reads interaction
state, so the control repaints when either changes. Because each token is its
own signal, a component wakes only for the colors it actually uses. Sizes,
radii, and font sizes are constants in `styled::theme`. Add a field to `Theme`,
with a value in every built-in theme, when a color is part of the theme rather
than unique to one component.

Styled controls must visibly expose keyboard focus, and must keep labels and
accessible roles stable when visual state changes. Use glyphs from `beui::icons`
with `styled::Icon` or `styled::IconSized`; do not use Unicode characters as
ad-hoc icons. The [keyboard guide](beui_keyboard.md) records the expected
behavior for each control family.

### Themes

`styled::Theme` holds the color tokens, and `Theme::DARK` and `Theme::EINK` are
the built-in themes. Every `Document` owns a theme that styled components use
when no provider covers them. Change it with `Document::set_theme` and read it
with `Document::theme`, for example to pick an app's clear color. The
inspector's Sim tab switches it at runtime.

`ThemeProvider` overrides the theme for the subtree written between its tags and
follows the `Prop<Theme>` it is given:

```rust
view! {
    <ThemeProvider theme={theme_signal}>
        <Settings />
    </ThemeProvider>
}
```

## Develop a base component

Base nodes are the only layer that should normally mutate a `Document` directly.
Put the node in `crates/beui/src/base/<name>.rs` and register the module in
`base.rs`. A base implementation has three parts:

- A crate-private node struct containing its retained state and child ids.
- An `Element` implementation for measurement, layout, painting, interaction,
  child traversal, and inspector metadata.
- `Document::create_*` and `Document::set_*` methods plus a public
  `#[component]` wrapper. The wrapper creates the node with `with_document` and
  binds reactive props to setters with `create_effect`.

`measure` takes `&self` and must not mutate; `layout` takes `&mut self` and may
update the node's own retained state, which is how a `VirtualList` realises
the rows its slice of the viewport calls for. Both receive `&mut Document` and are
reached through `crate::layout::measure` and `crate::layout::layout`, which take
the element out of the arena for the duration so an effect woken mid-walk cannot
alias it. Reach children through those two functions rather than calling another
element's methods directly, or the node you descend into is never handed its
constraint.

Measurements are memoised per available size and dropped whenever the arena
changes, so measuring a child repeatedly within a pass is cheap, but a `measure`
that is not a pure function of the node and its constraint will return a stale
answer.

Only invalidate retained state when a setter actually changes a value. A
spurious mutation invalidates layout or paint caching for the entire document.
Return children from both interaction traversal and `children`, give the node a
stable `kind` for the inspector, and add a concise `detail` when it makes the
tree easier to understand.

The `base` module itself is private. Re-export base components intended for
composition from `beui::reactive`, as the existing `Frame`, `Text`, and `Scroll`
components are, and keep implementation-only primitives crate-private when they
exist solely to support an unstyled control.

## Test and verify changes

Beui behavior is tested headlessly. For library behavior, add a test under the
relevant `tests` directory and declare it in that directory's `tests.rs`; this
repository keeps one test per file. The document tests use their `Harness` to
build a `Document`, send `Event` values through a `Context`, and inspect
component state, layout, accessibility, or painting output.

For a beui block editor, use `block_ui_test::BeuiTest`, built from an `Editor`
(or a `Creation`, for a dialog) so the test holds the same handle the component
was handed. Give every interacted node an `@test_id`, call `run` after queued
gestures, assert the resulting block state, and snapshot only when the painting is meaningful. `BeuiTest` also
supports key presses, text, hover, pointer clicks, and touch gestures. See the
[GUI testing guide](testing_a_gui.md) and the counter editor tests for examples.

From the workspace root, use:

```text
./scripts/check
./scripts/verify
```

`./scripts/check` is the fast complete-workspace compile check. Always finish a
coherent change with `./scripts/verify`; it runs the workspace tests, lints,
formatting, project structure checks, snapshot updates, and the formatter for
`view!` bodies that rustfmt cannot handle. Use a package-scoped Cargo command
only as a narrow diagnostic after one of the supported scripts has exposed a
failure. Run `./scripts/run --smoke` as well when a change can affect native
startup or runtime integration.
