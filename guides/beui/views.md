# Writing beui views

How to write the tree a component builds: `view!` and its attributes, children, conditional and
repeated content, the base layout nodes, how a frame lays out, and how to update a document from
outside its own events. Read `guides/beui/overview.md` first.

## Imports

Import `view!`, `#[component]`, signals and the base components from `beui::reactive`. Styled
controls come from `beui::styled`, behavior-only ones from `beui::unstyled`, icon glyphs from
`beui::icons`.

## `view!` attributes

Three framework attributes work on every tag, in their own `@` namespace so a component can name
its props whatever it likes:

- `@test_id` names a node for tests and `drive`. It takes a `Prop<String>`, so a row whose identity
  is reactive can carry one: bind a memo and the name follows it, and the old name stops
  resolving. A test id is one word. It is a property of the node like its accessibility: the
  accessibility node carries it as AccessKit's `author_id` (several ids joined by spaces), and a
  named node with no role or text of its own still gets one, a `GenericContainer`. So test ids
  reach `FrameOutput::test_id_rect`, `drive` and a plugin's description by the same incremental
  path as the rest of the tree, and only where the tree shows the node: not under a hidden frame
  or a closed overlay. `Document::find_test_id` finds the node itself.
- `@node_ref` fills a `NodeRef` when enclosing code genuinely needs the resulting `NodeId`.
- `@sizing` sets the child's `ItemSize` or `Sizing` among its siblings in a list, and only a list
  accepts it (see "List" below). On the single root of a `view!` it builds a `ListChild`, which is
  what the row builder of a `Dynamic`, `ForEach` or `Keyed` in a list returns.

## Children

A children slot names the type of child it takes:

| Slot | Takes | Used by |
| --- | --- | --- |
| `children: Children<ListChild>` | any number of children, each carrying an `ItemSize` | `List`, `Stack` |
| `children: Children<NodeId>` | any number of plain nodes | `Scroll` |
| `children: Children<CanvasItem>` | only items a `Canvas` can place | `Canvas` |
| `children: Child`, `children: Option<Child>` | one node | `Frame` |

A plain node converts into whatever a slot asks for that takes one, so
`<List><Text string="hi" /></List>` needs no ceremony. Writing `@sizing` on the child of a slot
that does not size its children is a compile error.

A `Vec` of children does not fill a slot. Children are written out as tags, and however many a
collection asks for comes from a `ForEach` over its keys inside the same `view!`, so sizing, keying
and reconciliation stay in one place:

```rust
<List spacing=4.0>
    <Text style=TextStyle::HEADING string="Cards" />
    <ForEach keys={card_ids()}>
        {move |id: Uuid| view! { <CardRow id /> }}
    </ForEach>
</List>
```

A slot holds its children in runs rather than one flat list, so a child can stand for none, one or
many of them and change how many as it goes. That is how a fragment written among siblings takes
the places between them, and how `Show`, `Dynamic`, `Keyed` and `ForEach` fill a parent they do not
own. `List`, `Stack`, `Scroll` and `Canvas` keep their children that way.

## Conditional and repeated content

| Component | Does |
| --- | --- |
| `Show` | builds its child each time the condition becomes true and disposes of it when it turns false, as SolidJS's does |
| `ShowKeepAlive` | builds the child the first time it is shown and only hides it after that, pausing its effects while hidden so they catch up once when shown again; for a child worth keeping, such as a tab panel whose scroll position and state should survive switching away |
| `Dynamic` | rebuilds a subtree when a value changes shape |
| `Keyed` | rebuilds only when its key changes |
| `ForEach` | keeps a keyed child per item (key by identity; see `guides/beui/overview.md`, rule 4) |

None of these builds a node of its own. They keep a run of children in a slot of the parent they
are written in, so:

- their children are laid out by that parent, with its direction, spacing and alignment, and the
  children around them keep their places however the run changes;
- a `ForEach` has no spacing of its own, and `@sizing` belongs on the rows, not on the `ForEach`;
- `@test_id` and `@node_ref` on one of them do not compile: there is no node to name;
- each needs a parent that keeps its children in slots (a list, a `Scroll` or a `Canvas`), so one
  written in a single-child slot like `Frame`'s does not compile until a `List` goes around it;
- a component whose whole view is a `Show` returns `-> DynamicSegment<ListChild>` (or whatever kind
  of child its parent takes) rather than a `NodeId`.

The kind of child a run builds is the kind its parent takes: the rows of a `ForEach` in a list are
`ListChild`s and can carry `@sizing`, the rows of one in a `Scroll` are plain nodes, and the rows
of one in a `Canvas` are `CanvasItem`s. A row builder that returns the wrong kind is a compile
error.

## Layout

The main base building blocks are `List`, `Frame`, `Text`, `Offset` and `VirtualList`. Text is
covered in `guides/beui/writing_components.md`, "Text"; scrolling and `VirtualList` in
`guides/beui/components.md`.

### Sizing a component

- Use `Frame`'s `width` and `height` (and their `min_`/`max_` forms) to constrain a component's own
  size, and `max_width` for a box that fills its room but stops at a limit.
- Use `@sizing` to describe how it shares space with its siblings in a `List`.
- Say a width once: a `Frame` nested inside one that carries the width is laid out within it, so
  the outer box is the only place the number belongs.
- For container-responsive state use `Container`, `narrower_than` and `shorter_than`.
  `unstyled::Stack` and `styled::Stack` switch between a row and a column without rebuilding their
  children.

### List

`List` arranges siblings in a line. There are no `Row` or `Column` aliases; read the props:

- `direction`: vertical unless the tag says otherwise, so a row is
  `<List direction=Direction::Horizontal spacing=8.0>`.
- `align`: the cross axis; a row that centres its children adds `align=Align::Center`.
  `Align::Baseline` lines a row's children up on the first baseline of their text, which a node
  reports through `Element::baseline`; a column treats it as `Start`.
- `justify`: places the main-axis space the children leave (`Start`, `Center`, `End`,
  `SpaceBetween`, `SpaceAround`, `SpaceEvenly`), so pushing the last toolbar item to the far end
  needs no spacer.
- `spacing`.
- `wrap=true`: flows children onto more lines. A child too wide for a line by itself is measured
  against the line's width, so long text wraps inside it, and each line's `Percent` children share
  that line's leftover.

A child's `@sizing` is an `ItemSize` or a `Sizing` built from one. Children are intrinsic by
default.

- `ItemSize::Intrinsic`; `ItemSize::Fixed(points)` reserves a logical-point size;
  `ItemSize::Percent(weight)` shares the bounded space left over, by weight.
- `.min(..)` and `.max(..)` bound its length.
- `.shrink(weight)` lets it give up length down to its `min` when the row overflows (the label
  beside a fixed-size button).
- `.align(..)` overrides the list's `align` for that child.
- `.gap(..)` replaces the list's `spacing` before it.

```rust
@sizing={ItemSize::Percent(100.0).max(320.0)}
```

### Frame

`Frame` combines optional sizing, an aspect ratio, padding, fill, outline and visibility on one
retained node, around one child.

- `width`, `min_width`, `max_width`, `height`, `min_height`, `max_height` and `aspect_ratio` each
  take an optional measurement, so a signal can hand one back to nothing and leave that axis
  measuring intrinsically again. The frame centres its box within an aspect ratio.
- `width_fraction` and `height_fraction` take that share of the space offered, when it is bounded.
  A frame with a maximum fills the space it is offered up to that maximum.
- `padding_horizontal` and `padding_vertical` pad both sides of an axis; `padding_left`,
  `padding_top`, `padding_right` and `padding_bottom` override one side.
- `align_horizontal` and `align_vertical` place a child smaller than the frame inside it rather
  than stretching it (a badge in a corner).
- `radius` rounds every corner; `radius_top_left` and its siblings override one.

### Other layout nodes

| Node | Does |
| --- | --- |
| `Layers` | lays every child over the same box, sized to the largest, later children on top (content over a background, a badge over an icon). A child that should not fill the box goes in an aligning `Frame`. Pointer input reaches the topmost catcher under the pointer and the catchers around it, never one in a layer beneath. |
| `Grid` | lines cells up in shared columns, filling row by row. `columns` is a `Track` per column (`Fixed`, `Intrinsic`: as wide as its widest cell, or `Fraction`: a weight in what is left), with `column_spacing` and `row_spacing`; each row is as tall as its tallest cell. A spanning cell is `<GridCell span=2>`. For forms and property panels whose labels share one width, not for a list of rows. |
| `Separator` (styled) | the rule between things. Runs `Direction::Horizontal` unless told otherwise, takes a line's thickness across its `direction` and the list's space along it, so a divider in a row is `<Separator direction=Direction::Vertical />` with no `@sizing`. `length` pins the long axis for a row that centres rather than stretches. |
| `Offset` | lays exactly one child out at full length along a `direction` and shows it from an offset; answers no input (see "Scrolling" in `guides/beui/components.md`). With `fit` it measures as long as its child, so a box sized by its content can still scroll once squeezed. |
| `Fade` | paints its one child faded out toward each edge over the widths its `edges` name |
| `Shift` | lays its child out moved `by` a vector without moving the space it takes, and paints nothing while the child is off screen (how a sheet slides in) |
| `Portal` | shows a subtree that belongs to someone else (below) |
| `Embed` | reserves a rectangle for something outside the document (below) |
| `VirtualList` | a box standing for one item per key, building only the rows its slice of the viewport reaches (`guides/beui/components.md`, "Long lists") |
| `Drawing` | paints a computed picture (`guides/beui/components.md`, "Custom drawing") |

`Portal` takes a `NodeId`, lays it out and paints it where the portal stands, and leaves it alone
when the portal goes away, so a subtree can move between places without being built again.
Exactly one portal shows a given node (claiming it takes it from the portal that had it), and the
subtree lives as long as the scope that built it.

`Embed` reserves a rectangle for something outside the document, such as an editor the host
composites behind the surface. It publishes the rectangle and the clip it was laid out in through
the `EmbedSlot` it was given (a `Copy` handle owned by the scope that made it) and cuts that
rectangle out of the surface so what is behind shows through. `punch=false` keeps the surface
whole, for something the host draws over it instead.

## One layout per frame

A frame runs in fixed phases: input, then layout, then paint.

- Input is dispatched against the rects the previous frame painted, which is what the user was
  looking at when they clicked.
- Events are dispatched once, in arrival order, pointer and keyboard interleaved. The effects an
  event's handlers woke run after that event and before the next, so a key typed just after a
  click reaches whatever the click focused.
- `Document::on_interacted` runs a callback once the frame's input has been dispatched and before
  anything is laid out, for an app that wants work started by that input under way during layout.

### Building during layout

Effects never flush inside the dispatch of one event or inside the layout walk, with one
exception: a node being laid out may build its own subtree, and the effects that build owns run on
the spot. `Document::build_within(node, ..)` does it:

- it lends the walk's own `&mut Document` to the build, so the build's `with_document` calls reach
  the document through that one reference;
- it stops anything the build invalidates from marking the node or its ancestors, which are in the
  middle of being laid out;
- the build flushes with `ScopeContext::flush_owned`, which runs only the effects owned by that
  scope and the scopes under it. A write the build makes to a signal outside its subtree wakes that
  signal's effects once the walk has finished, which keeps a build confined to its node.

Two nodes build during layout:

- **`LayoutBuilder`** hands its render prop a `ReadSignal<Vec2>` of the size it is laid out at, on
  each axis its parent bounds (an axis measured unbounded keeps its last value). When the walk
  reaches it at a new size it writes the signal, flushes its content's scope and lays the content
  out in the same walk, so a builder inside the content it just built is built in that walk too.
  Measuring it never rebuilds anything: a parent that tries several sizes is answered by the
  content built for the last size it was laid out at. If the rebuilt content measures differently,
  its ancestors are laid out again once. Use it for a component that needs the space it was given
  (one that wraps text, or switches between a row and a column), and give it that space from its
  parent (a stretch, a fixed or percent size) on the axis it reads.
- **`VirtualList`** builds each row its window reaches that it does not have yet, as the walk
  reaches it, measures it and carries on, so one walk fills the window. A list laid out at the
  length it measured follows that length as its rows measure, and rows that measure shorter at the
  end pull in rows above the window to take their place. When its length changed it asks with
  `Document::invalidate_after_layout` for the next layout to measure it again.

The rule these rest on is the one container queries settle on: a component may query the space it
was given, but must not be the thing that decides that space on the axis it queries.

### Where a node ended up

`PlacementBuilder` hands its render prop a `ReadSignal<Rect>` of where it was placed, in document
coordinates, written after each layout with the other placement reports. Use it for anything that
positions content in document coordinates (a canvas camera, an overlay anchored at a point).
Moving it does not lay its content out again; only the effects that read the rect run. Content that
needs the rect to lay itself out takes a second layout.

A component that only needs where on itself it was pressed reads the pointer event instead:
`PointerPress`, `ScrollGesture` and `ZoomGesture` carry `local` and `size` beside `pos`, and a drop
target's `DragPoint` is in its own coordinates.

### Reports after layout

Everything else layout decides is told to the tree after the walk: a `Scroll`'s position,
`component_placed`, `node_rect` and `node_placed` for watching other nodes, and
`Document::on_laid_out` (the callback block-app waits for its plugins in).

- A node with something to report queues it with `Document::report_after_layout`; one whose own
  size changed while it was laid out uses `Document::invalidate_after_layout`.
- The reports run together in one batch. Whatever they changed is laid out again (walking only the
  changed nodes and their ancestors) until nothing changes, so a frame is never painted from a
  layout the tree no longer has.
- A frame lays out at most four times. When the reports after the third still change something,
  the fourth layout holds its own reports back for the next frame, the frame is painted from it and
  asks for another, and debug builds log what was still changing.

Each extra layout is a cost the frame pays, so keep what a report drives to painting where you
can: a scrollbar's thumb is painted where the position says rather than laid out, which keeps
scrolling to one layout a frame. Use a report only for what depends on how other nodes ended up,
such as `TreeReveal`'s button appearing once the selected row has scrolled out of view.

### Revealing a node

`reveal_node` and `reveal_focus` are resolved inside the scroll's own layout: they leave a request
on each enclosing `Offset`, which lays its content out, finds the target (or its nearest placed
ancestor) and moves itself until the target is in view. A node's parent is the node whose
`children` lists it; the arena learns it when that node changes, not from the layout walk.

## Update a document from outside its events

Component callbacks run with their `Document` installed, so signal writes from clicks and key
events need no special handling. A host-driven update happens outside that context and must enter
the document's reactive scope:

```rust
use beui::reactive::{WriteSignal, with_reactive_scope};
use beui::Document;

fn set_value(document: &mut Document, value: &WriteSignal<String>, next: String) {
    let value = value.clone();
    with_reactive_scope(document, move || value.set(next));
}
```

Keep the write handles the host needs next to its `Document`. Do not rebuild the document to
display new data. Work finishing on another thread wakes the runner with a `Waker`
(`guides/beui/apps.md`).
