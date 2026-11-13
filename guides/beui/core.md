# Beui core

How a beui view is written and how the cores run it: views, the frame, layout
and measurement, base components, the HTML core and `beui-html`, and testing.

## Writing views

**Slots and child types.** A children slot names the child it takes:
`Children<ListItem>` (`List`, `Stack`), `Children<NodeId>` (`Scroll`),
`Children<CanvasItem>`, or `Child` / `Option<Child>` for one node. The child
types are all `Placed<P>`: a node plus the data its parent `P` keeps beside it
(a `Sizing`, a span), so a child built for one parent does not fit another and
the wrong kind is a compile error. A plain node converts into whatever a slot
asks for. A new parent implements `ParentKind` (and `BareChild` if a bare
`NodeId` should fit). A slot holds runs, not a flat list, which is how a
fragment, `Show` or `ForEach` fills places among siblings it does not own. So
control flow has no spacing, takes no `@test_id` or `@node_ref`, and does not
compile in a single-child slot until a `List` goes around it. A `Vec` of nodes
does not fill a slot; write a `ForEach` over the keys inside the same `view!`.
A component whose whole view is a `Show` returns `DynamicSegment<ListItem>`.

**Modifiers.** `@test_id` (a one-word `Prop<String>`, carried on the node's
accessibility node as AccessKit's `author_id`, so it reaches `drive` and tests
only where the tree shows the node) and `@node_ref`. Any `@path=value` expands to
`path(built, value)` after the component is built, in source order, and may
change the output type. A modifier works on the node itself; how a parent places
a child is a tag around it that builds the child that parent takes:
`<ListItem grow=1.0>` (a list or stack child's length: `grow`, `length`, `min`,
`max`, `shrink`, `align`, `gap`) and `<GridCell span=2>`. So
`<ListItem grow=1.0><Text>..</Text></ListItem>` is a `ListItem`, and only a list
takes it. Most `grow=1.0` children are the one child that takes what its siblings
leave; `<Spacer />` is an empty one that grows, to push the children after it to
the end.

**Other trees.** `beui-tree` knows nothing of documents, so its components and
control flow build any tree (`beui-html`'s DOM, a chat message): the tree's
child type implements `ChildValue` and `SlotChild`, what keeps children
`SlotHost` and `IntoSlotHost`, and the crate re-exports `beui_tree::reactive`
from a crate it calls `beui`, since the macros expand to `::beui::reactive`.
It supplies its own modifiers.

**Value components** return a value instead of a node (`unstyled::MenuItem`,
`ChoiceOption`); they get a scope as long as their builder's, so data items can
hold context, memos and cleanups. Declare the type with `value_child_type!`.
`ComponentRef::fill`, `component_accessibility` and `component_placed` panic in
them, as they do outside any component.

**Components in traits and impls.** Mark the trait or impl block
`#[component]` as well as each method; props go on the trait's declaration.
Use them as `<A::View editor />`. A component method takes no `self`.

**Props.** Pass a memo to a prop as it is, not as `move || memo.get()`, which
boxes a closure. A tag's children are built in a `move` closure, so inside a render closure that runs more than once,
clone a non-`Copy` prop in a block head:
`<Show condition>|| { let v = v.clone(); view! { <Text>{v}</Text> } }</Show>`.
A `Callback<T>` also takes a `Signal<T>`, which it sets. A `Callback<()>` prop takes a closure without arguments (`on_click={move || ..}`) and is called with `.call(())`.

**Ownership.** A node belongs to the scope it was built in and leaves the tree
when that scope is disposed, so a subtree that outlives its builder (one shown
through a `Portal`) is built in a scope that lives as long as it should.

**Updating from outside events.** A signal may be written from anywhere on the
UI thread (before or after `show`, a host callback, a polled future). The
document's effects wait for its next frame and the write wakes the window
([reactive.md, Zones](../reactive.md#zones)). `with_reactive_scope(document, f)`
is only for work that needs the document itself: `with_document`, creating
nodes or effects in its scope.

## One layout per frame

A frame is input, then layout, then paint. Input is dispatched against the
rects the previous frame painted (what the reader saw when they clicked), one
event at a time in arrival order, and the effects each event woke run before
the next, so a key typed after a click reaches what the click focused.

Effects do not flush during an event's dispatch or the layout walk, except
inside `Document::build_within`, which lets a node build its own subtree while
it is laid out; `VirtualList` builds rows that way, so one walk fills its
window. Everything else layout decides (a scroll's position,
`component_placed`, `measure`, `on_laid_out`) is reported after the walk.
Whatever the reports change is laid out again, walking only changed nodes and
their ancestors, until nothing changes, so a frame never paints a layout the
tree no longer has. A frame lays out at most eight times; tests fail past that
(`fail_unsettled_layout`). Prefer driving paint over
layout from a report: a scrollbar's thumb paints at the position rather than
being laid out, keeping scrolling to one layout a frame.

A new window size lays out from the root, but every node offered the space it
had keeps its measurement, placement and paint.

## Measurement

`measure(node)`, `measure_text(node)` and `lay_out_text` are in
`beui-view/src/measure.rs`. The signals may arrive a frame late: the HTML core
measures after the browser lays out. Native answers in the same frame, which virtual scrolling and scroll anchoring
rely on. `delay_measurement(true)` (`BEUI_DELAY_MEASUREMENT=1` for a driven
program) makes native late too; tests named `..._when_measurement_comes_late`
run key components both ways. A key handler that measures at event time fails
on HTML the first time; anchor an overlay to the node instead.

A component that builds from its own size measures its root after building it
and writes a signal its content reads. Keep the last answer when it turns
`None` (a hidden panel then does not rebuild at zero), and build content
placed by the size only once the first answer is in. It may query the space it
was given, but if it also decides that space on the same axis it never
settles: give it its room from the parent on that axis. Children whose rects
are a pure function of the parent's size are placed in the layout pass itself
with `CanvasArrangement`, without lagging a layout (the dock's tiled panes).

## Coordinates

`pos`, `measure` rects and text geometry are in window coordinates, through
every space's scale and translation. Pointer callbacks also carry `local` and
`size` in the node's own layout units, so a node in a zoomed
`CanvasItem scales=true` hears a press the same as unzoomed
([pan_and_zoom.md](../pan_and_zoom.md)). A length of the node's own compared
with window positions is multiplied by the space's scale first.

## Platforms, the runner and input

A platform only translates native input into `Incoming`; everything a frame
does is `Runner::frame`, and the runner's intake is the one place `Input`
becomes `Event`s (drive, the on-screen keyboard and tests push there too).
Input pushed without `Incoming::Time` happens at the frame clock's `now`, so
tests step time by stepping frames. [platforms.md](platforms.md) has the
runner, coalescing and each platform's input traps.

## Develop a base component

A primitive is declared once in `crates/beui-view/src/components/<name>.rs`,
whose `#[component]` calls `crate::core::<name>(..)`; each core has a body in
`src/native/<name>.rs` (over `beui-core`'s `Document`) and `src/html/<name>.rs`
(over `beui-core-html`). Hooks follow the same pattern from `src/reactive.rs`.

Natively the node lives in `crates/beui-core/src/base/<name>.rs` (registered in
`base.rs`): a struct holding its state and child ids, an `Element`
implementation, and `Document::create_*`/`set_*` methods. The `beui-view` body
creates it with `with_document` and binds each reactive prop to a setter with
`create_effect`. `create_*` returns a `NodeOf<XNode>`, so setters cannot be
handed another kind of node; code holding a plain `NodeId` asks
`arena.kind_of::<XNode>(id)`. Base nodes are normally the only layer that
mutates a `Document` directly. The `base` module is private: re-export what is
meant for composition from `beui::reactive`, and keep helpers for one unstyled
control crate-private.

The `Element` methods and arena accessors document their contracts. What spans
them:

- **Linear layout.** Anything computed across children is linear
  (`layout::share`). Measurements are memoised per offer, so `measure` must be
  a pure function of the node and its constraint. Reach children through
  `beui_core::layout`, never another element's methods.
- **Setters** pick the arena accessor by what reads the field (`get_mut_as`,
  `paint_mut_as`, `touch_mut_as`, `state_mut`), and call it only on a change.
- **Spaces.** Each node lays out and paints from its own origin, so a node that
  only moves keeps its layout and shapes, and a scroll or canvas content space
  (`Document::enter_space`) makes scrolling and panning change one translation.
- **Retained paint and damage.** A node records a tree (`display.rs`) of
  shapes, children by id and groups; the new tree is set against the old by
  identity to find damage, and a scrolled group becomes a copy
  (`WindowOutput::moved`) instead. A frame's `opacity` records a
  `Group::Mask` that fades its subtree as one picture: beui-renderer-wgpu
  draws it onto an offscreen layer and paints that at the opacity
  (`group.rs`), and a painting holds it as a `Content::Group` the rasterizer
  composites the same way, so overlapping shapes inside do not show through
  each other. Only the group's bounds within the damage are cleared, drawn and
  painted, and a group holding one shape that cannot overlap itself takes the
  opacity into its colour when the display is recorded (`paint.rs`, `folded`)
  instead. A punch inside a group also erases the target beneath it at the
  group's opacity. Edge fades (`fade_edges`) still fade shape by shape.
- **Checking paint.** Tests repaint every frame from scratch and fail when the
  retained painting differs or changed outside the damage (`verify_paint`).
  `BEUI_OVER_REPAINT=1` reports frames that damage over four times what
  changed.

## Cores

### One public API

Code that type-checks on one core type-checks on the other; only bodies differ.
be-tool-fix-rust-source enforces the layout:

- Every public item of beui-view and beui is declared once in shared code
  (node-free values in beui-types, the rest in beui-view's shared modules);
  `src/native/` and `src/html/` give only `pub(crate)` bodies. A handle that differs by core (`TextGeometry`, `Painter`, `Timer`)
  is one type with a private per-core field; a method only one core can give is
  not on it.
- One-core items (`beui::native`, `beui::html`) live only in the modules
  `ONE_CORE_MODULES` lists in `crates/be-tool-fix-rust-source/src/core_api.rs`,
  each with its reason.

### The contract both cores keep

- Whatever layout decides may come a frame late ([Measurement](#measurement)).
- On HTML the browser owns focus: `set_focus` and `focused` bindings go through
  DOM focus, so focus has moved only once code hears so.
- Each core does the best its platform allows: the browser's selection, find in
  page and scrolling into view; native's frame-perfect layout.
- Every feature exists on both. Where HTML cannot do it yet, its body is a
  visible stand-in (an outlined pink box, or an ignored prop) listed in
  plans/deferred/html_core.md.
- Colours are `Color`s that may reference declared variables, so a
  server-rendered page and both cores agree; on HTML they become `var(--…)`
  and follow the scheme without a redraw.

The inspector and app hosting are native only; `beui::can_open_inspector()`
says in advance whether one opens. On HTML a `Window` shows as its
overlay and `window_of` is always the main window.

### The HTML core

Each primitive is one element styled with inline CSS (`Overlay` a `<dialog>`
placed by CSS anchor positioning, `Drawing` an inline `<svg>`,
[decision](../../decisions/beui_drawing_is_inline_svg_on_html.md)). `Offset`
scrolls the element itself, so the browser's own scrolls are adopted
([decision](../../decisions/beui_html_scroll_uses_the_element_scroll_position.md)).
The root is fixed to the viewport and holds the colour variables, so every
overlay's element stays inside it. Containers with behaviour
of their own (`scroll`, `rows`, `canvas`, `overlay`, `portal`, `text`) are
modules of `beui-core-html`, each registering a pass with `frame::on_layout`
that runs before paint; `ResizeObserver`s call `frame::relayout()` so a resize
is corrected in the same frame.

Back is the browser's back, reaching what `Dismiss::Back` reaches natively
(guides/beui/overlays.md, Back and `BackSlide`).

A component's accessibility node becomes ARIA through the one table in
`beui-core-html`'s `aria.rs`. Drive lines come from the accessibility nodes,
not the ARIA, so they read the same on both cores.

**Running and server rendering.** `beui::html::run(element_id, build)` starts a
program; `render_to_string` on the host and `hydrate` in the browser serve a
page that shows before any script runs. crates/be-server-portal-ui is the worked
example (guides/running/web.md); crates/beui-demo-html runs beui-demo on this
core.

## HTML: strings and live DOM

`beui-html` builds ordinary HTML elements with `beui-tree` components and
control flow, independent of the GUI's layout and paint. It serializes to a
string anywhere (`render`, `render_fragment`, `Node::to_html`) and on wasm also
builds live DOM (`beui_html::dom`). Elements are components in
`beui-html/src/elements.rs` (import `beui_html::elements as html`); add missing
standard attributes there. Style with classes and one `<Stylesheet css=... />`.

### Live DOM and hydration

`dom::mount(container, || view! {..})` builds browser nodes; `dom::hydrate`
claims the nodes a server rendered from the same view. Keep the returned
`Mounted` for as long as the page is live. Changes then patch nodes in place:
a control-flow fill moves nodes only between its two anchor comments
(`<!--[-->` and `<!--]-->`), so rows a `ForEach` keeps keep their DOM nodes. A slot opened on an
element already in the page puts its anchors there at once, which re-places the
element's children: O(children), so open slots while building, not per frame.
Hydration walks tree and page in serialization order and trusts the server's
attributes; a mismatch warns in debug builds and builds that node fresh, never
panics. A text field typed into before the page hydrated gets an `input` event
once its handlers are attached, so its signal holds what was typed. Handlers are `Callback<beui_html::Event>`, whose methods compile on
both backends; the string backend ignores them. The live backend flushes after
each handler; signals set elsewhere need a flush the host installs with
`beui_reactive::wake_with`.

## Test and verify changes

Every test that runs a document runs it through
`beui_host::native::testing::Stepped`, the way a platform does: the real
`Runner` under a drive layer, on the headless `Simulation`, with a gpu-free
`Screen`, the clock stopped and one frame interval per frame.
`testing::Showing(document)` is a `Document` as a program.

- Its gestures (`click`, `drag`, `scroll`, `type_text`, `key`, `settle`, ...)
  are `drive`'s commands, and each settles as `drive` does. A failure prints the
  steps as `drive` commands, which replay against a `:dev` program.
- `frame(inputs)` pushes raw `Input`s in points through the runner's intake
  and paints one frame, so send what a platform would (`Input::Modifiers`
  before a chord); a `Context` takes no `Event` directly.
- `snapshot(name)`, `snapshot_layers(name)` and `snapshot_window(name, id)`
  compare paintings ([testing_a_gui.md](../testing_a_gui.md)); any other
  painting goes through `beui_paint_snapshot::assert_painting`.

beui's own tests use `Harness` (`crates/beui-host/src/tests.rs`), a `Stepped`
with settling off; a block editor uses `be_sdk_test::BeuiTest`.

**Both cores.** Behaviour that must match goes in a shared screen and script in
crates/beui-tests, run natively, natively with late measurement, and in
Chromium (testing_a_gui.md, 7). Browser paintings come from `render_to_string`
on `linux_x86_64_html`: `./scripts/test //crates/beui-view:html-test` for
primitives, `//crates/beui:html-test` for components, and
`//crates/be-server-portal-ui:test` for the portal's pages; build the page's
style from `stylesheet("/")` so text uses the bundled fonts. They show layout,
colour and text, not what needs browser layout (marks, carets).
