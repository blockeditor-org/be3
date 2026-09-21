# Beui components

Per-component reference for the parts of beui that need more than their props
to use well. [beui.md](beui.md) is the guide to read first; this one is for
looking up the component you are about to write. The re-exports in
`unstyled.rs` and `styled.rs` are the authoritative component lists.

## Drawing and measuring text

`Drawing` is the one base node that paints rather than arranges: it takes a
`Draw`, a callback handed the `Painter` and the rectangle the node was laid out
at. It is for content whose shape is computed rather than arranged - the text
editor lays a syntax-highlighted document out itself, byte by byte, and paints
the result as four layers. Build the callback in a memo over the page it draws,
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

`Text` carries its own decoration: `underline` is painted from the galley's
baseline, so switching it on never moves anything.

## Portal, Embed and Viewport

`Portal` shows a subtree that belongs to someone else: it takes a `NodeId`,
lays it out and paints it where the portal stands, and leaves it alone when the
portal goes away, so a subtree can move between places in the tree without
being built again. Exactly one portal shows a given node - claiming it takes it
from the portal that had it - and the subtree is kept alive by whoever built it,
with `in_new_scope` or a scope of their own, until they remove it.

`Embed` reserves a rectangle for something outside the document — an editor the
host composites behind the surface — publishing the rectangle and the clip it
was laid out in through the `EmbedSlot` it was given and cutting that rectangle
out of the surface so what is behind shows through. `punch=false` keeps the
surface whole, for something the host draws over it instead.

`Viewport` reserves a rectangle the renderer draws into rather than the
document: it fills the space it is given and paints the `Drawing` it is handed
in its place (see [Draw with the gpu](#draw-with-the-gpu)).

## Scrolling

Scrolling exists at all three layers, and app code wants the styled one.
`styled::Scroll` and `styled::VirtualList` are a scroll with the project's
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

`unstyled::Scroll` is the same arrangement without the appearance: it owns the
base offset, the input that drives it, the position it reports, and the list
that puts the bar on the scroll's cross axis, and it takes a `ScrollbarStyle`
saying what to put there. That is the seam the styled layer fills, with a
spacing and a builder that is handed a `ScrollHandle` of the live `position`
and `direction`:

```rust
ScrollbarStyle::new(SCROLLBAR_SPACING, |handle: ScrollHandle| {
    let ScrollHandle { position, direction } = handle;
    view! {
        <Scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position direction />
    }
})
```

Without one the scroll shows no bar, which is what the unstyled layer does on
its own. A control that scrolls something of its own takes a `ScrollbarStyle`
and passes it down, the way `unstyled::Select` hands one to the scroll behind
its options, so the styled control decides the bar and the unstyled one never
names a colour. The gutter is always reserved, and the bar paints nothing while
its content fits, so a scroll that grows past its viewport does not shift the
content beside it.

Under both sits the base `Offset` and `VirtualOffset`, which are named for what
they do rather than for what they are used for: they hold a run of items along a
direction and lay them out from an offset, with no bar, no theme, and no input
of their own. A wheel, a touch drag and the arrow keys are `unstyled::Scroll`'s,
which wraps the offset in a `Focusable` for the keys and a `ClickCatcher` for
the wheel and the drag, keeps the momentum an unfinished fling carries, and
drives the offset from all three. So reach for `Offset` when something needs its
content shifted under a viewport and nothing more, and for a `Scroll` whenever
something needs to scroll.

`Scroll` and `VirtualList` take the same `direction`, so the same tag is a
column of rows or a strip of cards; a horizontal one answers Shift+wheel, a
sideways trackpad swipe, a touch drag and the left and right arrows. A plain
wheel is left to whatever is around it, the way a browser leaves a horizontal
strip alone, and a wheel only ever reaches the innermost scroll under the
pointer.

That offset is anchored to a node, not measured from the top of the content:
the scroll remembers the item at the top of its viewport and the distance it
starts above the edge, so a row further up growing or shrinking - a wrapping
label, an image that finished loading - leaves what is being read exactly where
it is.

Use `VirtualList` for a collection large enough that building every row is the
cost.

## Slider scales

A slider spreads its range evenly along its track unless it is given a
`scale`. `SliderScale::Midpoint(value)` curves it so that the centre of the
track reads that value, which is how a control whose interesting values are
bunched at one end gets most of the track for them. The curve is exponential
rather than a power of the position, which is what keeps the fine end useful
without giving the whole of it away: the inspector's blur slider runs to
120 px with a midpoint of 12 and reads 3 px, 12 px and 39 px at the quarters
of its track, spending a tenth of the track below one pixel where a power
curve through the same midpoint spends a quarter of it there. The midpoint may
sit above the centre of the range as well, which gives the top end the fine
part of the track instead. A midpoint outside the range, or one that lands
where the centre already is, leaves the slider linear.

The curve applies everywhere the value and the track meet. Dragging maps the
position under the pointer through it, the knob sits where the value falls on
it, and keyboard steps move by a share of the track rather than a share of the
range, so an arrow key near the fine end moves a little and the same key near
the coarse end moves a lot. What a screen reader is told the step is follows
the value the next step would actually reach.

## Tooltips

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

## Overlays, and things that float

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

`MenuButton` is the button that opens a menu under itself, which is what a
toolbar reaches for where `Select` would imply the choice sticks; `ContextMenu`
is the same menu on a secondary press, and it also takes an `open_at` point so
a touch gesture can raise it where the finger was.

## Docking and windows

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
instead, which is also what "Pop out into a window" on a tab's own menu does. A
window holds one pane, so a tab dropped anywhere inside one joins it rather
than splitting it, and the pane's tab bar is the window's title bar: a grip,
the tabs, and the button that closes it. Anywhere on that bar that is not a tab
drags the window, so the grip and whatever room is left beside the tabs are
both handles. Windows resize from any of their eight grips and are raised by
whatever takes the focus inside them. Ctrl+Tab and Ctrl+Shift+Tab walk the tabs
of the pane the focus is in, registered with `on_shortcut` so they arrive even
from inside a text input in a panel. The bar between two panes is a tab stop
with a `Splitter` role: the arrow keys move it, and the tab bar is a `Choice`
inside a horizontal `Scroll`, so the arrows, Home and End walk it like any
other tab list and scroll the tab they reach into view when a pane has more
tabs than it has room for.

A tab's panel is built the first time the tab is shown and belongs to the dock
rather than to the pane showing it: the pane holds a `Portal` pointed at it, so
the panel keeps its nodes, its scroll position, its caret and its state when
the tab is hidden behind another, dragged to another pane, or floated into a
window. A panel no pane is showing is laid out by nobody, so it costs nothing
and a screen reader does not read it. Closing the tab is what removes it.

## Pan and zoom

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
what a caller paints a focus ring from. Because the view is the caller's, a
toolbar button, a fit command or a host that syncs several editors writes the
same signal the gestures do; the camera a plugin editor is handed
([pan and zoom](pan_and_zoom.md)) reaches a beui editor the same way. Anything
between the gesture and the camera - momentum, snapping, clamping the camera to
the content - belongs to the caller, apart from the scale limits `min_scale`
and `max_scale`.

## Hold the pointer

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
asks the host to ([plugin editor guide](adding_a_plugin_editor.md)). What comes
back is `Event::PointerMotion`, a delta with no position, which the document
delivers to the focused node while the pointer is held and nowhere at all while
it is not. So a control that is not the one holding the pointer never sees
motion, and a window that loses the keyboard gives the pointer back.

## Draw with the gpu

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
