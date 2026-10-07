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

The quickest introduction is the component catalog in
`crates/beui-demo`: a dock whose Components pane opens a page for
each group of styled components, for the unstyled components painted by
hand, and for the base nodes (frames, text, lists, grids, control flow,
interaction, layers and overlays). Every sample on a page shows the code it was written with: a
function marked `#[beui_macros::sample]` (above its `#[component]`) also
gets a `Name::SOURCE` holding its text exactly as written, so a new sample
cannot drift from its listing. The
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
inputs changes. `./scripts/verify` reports a function outside an
`impl` that writes a `view!` and returns a node or a child value without the
attribute. `#[component]` is also what makes the function usable as a tag,
gives it `@test_id`, `@node_ref` and `@sizing`, and makes
`component_state`, `component_accessibility`, `component_size`,
`component_rect` and `component_placed` available inside it.

A component that returns its own type implements `ChildValue` to hand the
scope to the node it hangs on, and `IntoChild` for the slot that takes it, as
`beui-view`'s `components/canvas.rs` does for `CanvasItem`.

A child needs no node at all. A `ChildValue` that is no node keeps a
`ChildScope` field instead, which the component's scope is moved into, so
dropping the value disposes exactly the effects that building it created and
the owner tree does the rest. That is how an item made of data rather than
nodes — a label, a key, a callback — can still be a component, with its own
scope, context, memos and cleanups, and still be written as a tag. Such a
component has nothing for `component_state`, `component_accessibility`,
`component_size`, `component_rect` or `component_placed` to watch, so calling any of them in its body
panics when it is built, and `@test_id` and `@node_ref` on its tag do not compile,
because they only take a component whose output implements `BuildsNode`.
`unstyled::MenuItem` is one: a menu item is a label, a disabled flag and its
own submenu items, so a menu is written as tags and each row follows
the signals its tag was given, and `unstyled::ChoiceOption` is the same for the
options of a tab bar, a listbox, a radio group and a select. Declare the type
with `value_child_type!` rather than `child_type!`, which additionally says how
a run of it is kept, so a `show` or a `for_each` can build one.

Functions that build no part of a view are ordinary functions. Deriving a
colour from theme tokens and interaction state, mapping a value to a label,
reading state back out of a built node — write those as plain functions, as
`beui-components-styled`'s `checkbox.rs` does with `box_fill` and `checkbox_checked`.

### A component usually ends with one `view!` and nothing after it

Prefer making the last line of a component a single `view! {}` producing the
node it returns:

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

Signals, memos, callbacks, and handle destructuring go above it. Avoid putting
anything below it or a second `view!` earlier in the body: `view!` builds
nodes the moment it runs, so a subtree built into a local and then used
conditionally has already been added to the document whether or not it ends up
in the tree, and a subtree built in one place but parented somewhere else
obscures which component's scope owns it. This is not enforced; break it where
another shape is clearer.

When part of the tree depends on something, express it in the view rather than
in Rust control flow around it. `Show` takes a condition, builds its child each
time it becomes true and disposes of it when it turns false, as SolidJS's does;
`ShowKeepAlive` is the opt-in for a child worth keeping while it is hidden, such
as a tab's panel whose scroll position and state should survive switching away:
it builds the child the first time it is shown and only hides it after that,
pausing the child's effects while it is hidden so they catch up once when it
is shown again.
`Dynamic` rebuilds a subtree when a value changes shape, `Keyed` rebuilds only when its key changes, and `ForEach` keeps a
keyed child per item. A component that is genuinely two different trees is two
components with a `Dynamic` or a `Show` choosing between them.

None of those four builds a node of its own. They keep a run of children in a
slot of the parent they are written in, so their children are laid out by that
parent, with its direction, spacing and alignment, and the children written
around them keep their places however the run changes. That is why a `ForEach`
has no spacing of its own, why `@sizing` belongs on the rows rather than on the
`ForEach`, and why `@test_id` and `@node_ref` on one of them do not compile:
there is no node to name. Each needs a parent that keeps its children in
slots - a list, a `Scroll` or a `Canvas` - so one written in a single-child slot like
`Frame`'s does not compile until a `List` goes around it. A component whose
whole view is a `Show` returns it typed `-> DynamicSegment<ListChild>` (or
whatever kind of child its parent takes) rather than a `NodeId`.

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
them. Prefer `Keyed` over `Dynamic` when only part of a value decides the
shape, and `VirtualList` for a collection large enough that building every row
is the cost.

## Crates

Beui is a family of crates. `beui` is the facade every app and editor depends
on: it re-exports the others under the paths this guide uses (`beui::reactive`,
`beui::styled`, `beui::unstyled`, `beui::Document`, ...) and assembles them,
so code outside beui names only `beui`. The rest depend on each other in one
direction:

| Crate | Owns |
| --- | --- |
| `beui-tree` | what makes a tree of components, whatever it is made of: `#[component]` and `view!`'s runtime, `Prop`, child slots (`Children`, `Run`, `SlotChild`, `SlotHost`) and `Show`, `ShowKeepAlive`, `Dynamic`, `Keyed` and `ForEach` |
| `beui-core` | `Document` and `Context`, the retained nodes and their layout, input and its dispatch, accessibility, paint output and damage, the font and image interfaces, the icon codepoints, the `App` contract, and the `Runner` every adapter drives it through |
| `beui-font-freetype` | `FreetypeFonts`: FreeType and HarfBuzz shaping and rasterizing, and the fonts beui compiles in |
| `beui-font-browser` | `BrowserFonts`: text measured with the browser's own fonts through a canvas, for the DOM renderer; no fonts in the module |
| `beui-view` | `beui::reactive`: `beui-tree` re-exported, with the base components (`Frame`, `List`, `Text`, ...) that wrap core's nodes; `beui_core::tree` is what makes a `NodeId` a child |
| `beui-components-unstyled` | `beui::unstyled` and `beui::datetime` |
| `beui-components-styled` | `beui::styled`: themes and styled controls |
| `beui-inspector` | the inspector, the simulated screen reader and the simulated mouse and keyboard |
| `beui-renderer-wgpu` | the wgpu renderer, its shaders and filters, and presenting to a surface |
| `beui-renderer-dom` | the DOM renderer: the display tree as nested absolutely positioned elements |
| `beui-adapter-winit`, `beui-adapter-android`, `beui-adapter-web` | each platform's `Adapter` and `Platform`: its event loop, window or view, input, IME, clipboard, file picker and accessibility adapter |
| `beui-adapter-drm` | Linux's displays and input devices through DRM/KMS and libinput: an `Adapter`, `Platform` and `Renderer` whose screens each keep a retained frame and are drawn when their display flips; callers run it with `beui::run_on` |
| `beui-adapter-plugin` | the `Adapter`, `Platform` and `Renderer` for one region of a block editor plugin, which the plugin framework drives frame by frame (guides/adding_a_plugin_editor.md); it depends on the plugin framework, so the facade does not offer it |

Core cannot see the crates above it, so the few places it used to reach up are
hooks the higher crates fill in:

- The inspector is a `Tools` object on the document. `beui::reactive::build`
  installs it (`beui::install_inspector`), and `Document::show` hands the frame
  to it; a document built by `beui_view::reactive::build` has none.
- The simulated mouse is an `InputSimulation` the inspector installs on the
  `Context`.
- Text is shaped by the `FontBackend` a `Context` is made with.
  `beui::context()` makes one over `FreetypeFonts` and only the fonts beui
  bundles, which is what tests use; `beui::system_context()`, which the
  runners use, adds the system's own fonts for whatever the bundled ones do
  not cover. The fonts live in a `FontLibrary` that contexts can share and
  that can be given more fonts later, which lays their text out again.
- The document's theme lives in a typed slot on the document
  (`Document::extension`); `beui::styled::DocumentTheme` reads and writes it.
- A `Drawing` holds whatever its renderer draws; `beui::drawing` makes one for
  the wgpu renderer.
- Each platform is a `beui_core::runner::Adapter`, a trait object the facade
  picks (`beui::window_adapter`, `beui::web_adapter`) and runs with a `Launch`
  (`RunOptions`, `Context`, the `App`). The adapter owns the event loop and
  turns the platform's input into `Event`s; everything one frame does is
  `beui_core::runner::Runner::frame`, shared by all of them: it runs the
  app, publishes accessibility, and tells the adapter's `Platform` (another
  trait object) to copy, paste, pick a file, or change the cursor, pointer
  lock, IME, fullscreen and back handling, only when one of them changed.
  A new platform implements those two traits and hands its window or element
  to the renderer loader the facade gives it.
- Every adapter draws through `beui_core::renderer::Renderer` trait objects and
  names no renderer crate: wgpu's `WindowSurface` (native windows),
  `CanvasSurface` (a browser canvas) and `DomRenderer` implement it, and the
  facade hands the runner the ones to load. A renderer is told where to draw
  with `attach` (a raw-window-handle `WindowHandle`) or is made with its element,
  and draws a frame in two steps, `prepare` and then `present`.
  `beui_core::renderer::Renderers` holds what a runner loaded, each with the
  `FontBackend` its text is measured with if it needs its own, and shows one.
  `Context::choose_renderer` (the inspector's Perf tab offers it when more
  than one is loaded) hides and detaches the shown one, attaches or resizes
  the chosen one, and swaps the context's fonts with `Context::replace_fonts`,
  which lays all text out again.

Inside the family, crates name each other directly (`beui_core::document::Document`),
and the component crates declare `extern crate beui_view as beui;` so the
`::beui::reactive` paths `#[component]` and `view!` expand to resolve there.

`beui-tree` knows nothing of `Document`, layout or painting, so the same
components and control flow can build any tree: a DOM, a 3D scene, a chat
message with buttons. Such a tree gives its child type `ChildValue` (where a
component's scope lives) and `SlotChild` (how a child is kept and discarded),
and gives what keeps children `SlotHost` (appending, filling a slot a `Show` or
`ForEach` holds, and owning their scopes) and `IntoSlotHost`, so a component
can `children.mount(...)` into it. The macros need `::beui::reactive` to
resolve: `beui-tree` names itself `beui` for its own tests, and a crate built on
it does the same or re-exports `beui_tree::reactive` as a `reactive` module of
a crate it calls `beui`. `@sizing`, `@test_id` and `@node_ref` expand to
`beui::reactive::ListChild`, `with_test_id` and `with_node_ref`, which only
`beui-view` has, and an untyped `Render`/`RenderFn` child or a `Child` prop is
`beui::reactive::Child`, a `NodeId`. `beui-tree`'s tests build a small DOM this
way.

## Component layers

Beui separates mechanism, behavior, and appearance. The dependency direction is
deliberate:

| Layer | Location and public path | Responsibility |
| --- | --- | --- |
| Base | nodes in `crates/beui-core/src/base`, components in `crates/beui-view/src/components`; re-exported from `beui::reactive` | Retained nodes for layout, painting, visibility, focus, pointer input, offset content, and text. |
| Unstyled | `crates/beui-components-unstyled`; `beui::unstyled` | Accessible interaction behavior composed from base components, without theme colors, typography, borders, or spacing. |
| Styled | `crates/beui-components-styled`; `beui::styled` | Application-ready controls that compose an unstyled control and paint its state with base components and `styled::theme` tokens. |

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
  instead. The base layer is small on purpose — `Frame`, `List`, `Layers`, `Grid`, `Text`,
  `Offset`, `VirtualList`, `Canvas`, `Drawing`, `Overlay`, `Interactive`, `Embed`,
  `Portal`, `BackHandler`, `Shift`, `Fade` — and it stays small because most things are
  compositions of those. `unstyled::Picture` is one: a `Drawing` with a size.
  Add a base component only when the retained tree genuinely lacks a primitive:
  a new way to lay out, paint, or receive input that cannot be expressed by
  arranging the existing nodes. `Portal` is one: it shows a subtree it does not
  own, which is how a node laid out in one place this frame is laid out
  somewhere else the next without being rebuilt. If a new concern can share `Frame`'s single-child box model,
  extend `Frame` rather than adding another pass-through node.

`Drawing` is the one base node that paints rather than arranges: it takes a
`Draw`, a callback handed the `Painter` and the rectangle the node was laid out
at. It is for content whose shape is computed rather than arranged - a plot, a
waveform, a canvas of strokes. Build the callback in a memo over the page it draws,
so the closure is replaced only when that page changes, and cull to
`painter.clip_rect()` inside it, so a document far taller than the viewport
costs the screenful it shows. Reach for it only when there genuinely is no
arrangement of nodes that says the same thing: a row of labels is a `List` of
`Text`, not a `Drawing`. It measures to nothing, so it takes its size from
whatever places it - a `Frame` with a width and a height, or a `CanvasItem` -
unless it is given a `size`, which it measures to, scaled down to the width it
is offered and keeping its shape, as an image does.

Text is never one of those. `Text` takes `spans` - byte ranges of its string
with a font, a colour, an underline or a strikethrough each, or a fixed-width
space, or the next of its `TextItem` children laid inline - and lays the runs
out and wraps them itself, breaking after whitespace or after a span marked
`break_after`, with `line_padding` above and below each line. It also paints
`marks` behind the text (a selection, a code background) and `carets` in front
of it, blinking ones included, and a `TextItem` given `at` is placed at that
index as if it were a caret, which is how something floats beside a position
in the text. `Document::text_geometry` answers where an index is and which index is
under a point from the node's last layout, without a document installed.
`unstyled::TextArea` is built from exactly this: a row per document line in a
`VirtualList`, a gutter of `Text`s beside it, and checkboxes, embedded widgets
and the caret's anchor as `TextItem`s.

The galleys a `Drawing` paints come from `layout_text(text, font, layout)`,
which lays text out with the shown document's fonts and answers `None` until
the document has been shown once. It is how a component measures text outside
`measure` and `paint` - to work out where a caret sits, or how wide a column
is - and the galleys it returns are cached, so asking for the same word twice
costs a hash lookup. A `FontId` carries `bold` and `italic` alongside its size
and family; both are synthesised by FreeType rather than loaded as separate
faces, and `Text` takes them as props.

`unstyled::Button` shows the split. It is a focusable `Interactive`,
and owns button semantics, disabled behavior, pointer and keyboard activation,
and accessibility: it takes the `label`, `glyph`, `role` or `Action` and builds
its own accessibility node from them. Its content closure receives a
`ButtonHandle` of reactive `hovered`, `active`, `focused` and `disabled` state
and the resolved `label`, `glyph` and `tooltip`. `styled::Button` wraps it and
uses that handle to choose fills and paint a focus outline, so every visual
treatment sits on the same interaction behavior. Handles of controls built on a
button, like `MenuButtonHandle` and `PopoverTriggerHandle`, carry that
`ButtonHandle` as `button` rather than copying its fields.

Pure presentation components such as styled text and cards compose base
components directly, because they have no interaction behavior to delegate.

The main base building blocks are `List`, `Frame`, `Text`, `Offset`, and
`VirtualList`. `List` arranges siblings in a line: it takes a
`direction`, which is vertical unless the tag says otherwise, an `align` for
the cross axis, a `justify` for the main axis, and `spacing`. Nothing wraps it,
so a row is written `<List direction=Direction::Horizontal spacing=8.0>` and a
row that centres its children adds `align=Align::Center`; a one-line alias per
combination is what `Row`, `Column` and `CenteredRow` were, and reading the
props beats remembering which names exist. `justify` places whatever main-axis
space the children leave (`Start`, `Center`, `End`, `SpaceBetween`,
`SpaceAround`, `SpaceEvenly`), so pushing the last item of a toolbar to the
far end needs no spacer. `Align::Baseline` lines a row's children up on the
first baseline of their text, which a node reports through
`Element::baseline`; a column treats it as `Start`. `wrap=true` flows
children onto more lines: a child too wide for a line by itself is measured
against the line's width, so long text wraps inside it, and each line's
`Percent` children share that line's leftover.

A child's `@sizing` is an `ItemSize` - `Intrinsic`, `Fixed` or `Percent`, the
last a weight in the bounded space left over - or a `Sizing` built from one:
`.min(..)` and `.max(..)` bound its length, `.shrink(weight)` lets it give up
length down to its `min` when the row overflows (the label beside a fixed-size
button), `.align(..)` overrides the list's `align` for that child, and
`.gap(..)` replaces the list's `spacing` before it:
`@sizing={ItemSize::Percent(100.0).max(320.0)}`.

`Frame` combines optional sizing, an aspect ratio it centres
its box within, padding, fill, outline, and visibility on one retained node.
Its `width`, `min_width`, `max_width`, `height`, `min_height`, `max_height`
and `aspect_ratio` each take an optional measurement, so a signal behind one
can hand it back to nothing and leave that axis measuring intrinsically again;
`width_fraction` and `height_fraction` take that share of the space the frame
is offered, when that space is bounded. `padding_horizontal` and
`padding_vertical` pad both sides of an axis and `padding_left`,
`padding_top`, `padding_right` and `padding_bottom` override one side.
`align_horizontal` and `align_vertical` place a child smaller than the frame
inside it rather than stretching it over the frame, which is how a badge sits
in a corner. `radius` rounds every corner and `radius_top_left` and its
siblings override one.
`Layers` lays every child over the same box, sized to the largest, later
children on top: content over a background, a badge over an icon. A child that
should not fill the box goes in a `Frame` that aligns it. Pointer input is the
same as anywhere else - every catcher under the pointer hears it unless the
top one sets `capture_presses`.
`Grid` lines cells up in shared columns, filling them row by row: `columns`
is a `Track` per column (`Fixed`, `Intrinsic` - as wide as its widest cell -
or `Fraction`, a weight in what is left), with `column_spacing` and
`row_spacing`, and each row is as tall as its tallest cell. A cell that spans
several columns is written `<GridCell span=2>`. It is for forms and property
panels whose labels should share one width, not for a list of rows, which is
a `List` or a `VirtualList`.
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
`Offset` takes exactly one child, lays it out at its full length along a
`direction` and shows it from an offset; a run of rows goes in a `List` inside
it. It answers no input at all, so nothing scrolls by putting one in a view
(see [Scrolling](#scrolling)). With `fit` it measures as long as its child,
so a box sized by what it holds can still scroll once it is squeezed. Rows far
outside its viewport are culled like any other node out of sight. `Fade` paints
its one child faded out toward each edge over the widths its `edges` name. `Shift`
lays its child out moved `by` a vector without moving the space it takes, and
paints nothing while the child is off the screen, which is how a sheet slides in. `VirtualList` is an ordinary box that stands for
one item per key, each of an estimated `item_size`, and builds only the ones its slice of
the viewport reaches (see [Long lists](#long-lists)).
`Scroll` takes a `direction`, so the same tag is a
column of rows or a strip of cards; a horizontal one answers Shift+wheel, a
sideways trackpad swipe, a touch drag and the left and right arrows.
A plain wheel is left to whatever is around it, the way a browser leaves a
horizontal strip alone, and a wheel only ever reaches the innermost scroll
under the pointer. The unstyled module contains
`Button`, `Pressable`, `Toggle`, `Choice`, `Slider`, `TextInput`, `TextArea`,
`Disclosure`, `Tree`, `Select`, `ContextMenu`, `MenuButton`, `Popover`, `Container`,
`PanZoom`, `PointerLock`, `Dock`, `Draggable`, `DropTarget`, `Tooltip`, `Floating`, `Scroll`, `Scrollbar`,
`Stack`, `Calendar`, `DateTimeField`, `TimeList`, `ColorArea`, `ColorWheel` and `Picture`. `TextArea` is the multiline one: it owns a
`text_editor_core::Core` through the `TextAreaState` its caller holds, lays the
document out with a gutter, wrapping, collapsible sections and markdown
checkboxes, and lays out the inline and block `TextWidget`s the caller
names - `block` builds what goes under a block widget's line, and
`selected_widget` what floats under an inline one while it is selected - which
is how a block editor puts an embedded block inside the text and drives the
same document from a toolbar of its own. A `completer` names a trigger
character and a search: typing it after a space opens the menu
`completion_menu` draws under the caret, filtered by what follows it, with
arrows, Enter and Escape; `styled::TextArea` uses it for emoji (`:rocket`). The state republishes
what it shows whenever one of its own commands runs; code that changes the
document behind it - adopting an edit that arrived from someone else - calls
`sync()`, or `external_edit()` when the edit should also break the undo group,
since nothing polls the document for changes. It shows a `placeholder` while
the document is empty, masks every character under `password`, and leaves a
caret handle under a touch tap that drags the caret and opens its menu.
`single_line` is the same control laid out on one unwrapped line with no
gutter, scrolled sideways to keep the caret in view, where Enter submits and
Tab leaves; `frame` wraps the field in the caller's chrome inside the area's
own focus and pointer handling. `TextInput` is that single-line mode over a
plain-text buffer it owns, driven by a `value` and reporting `on_change`, so a
fix to how text is edited lands in both. `MenuButton` is the button that opens a menu under itself, which is
what a toolbar reaches for where `Select` would imply the choice sticks - or,
when a finger opened it, the same items as rows in a sheet, so one button
serves a mouse and a touch (`IconMenuButton` is the same with an icon button's face);
`ContextMenu` is the same menu on a secondary press, and it also takes an
`open_at` point so a touch gesture can raise it where the finger was. A finger
held still for the long-press delay (`Context::set_long_press_delay`, which a
test sets to zero rather than waiting) is a secondary press where it rests, so
every context menu opens on tap-and-hold; the press the finger began is
cancelled and lifting it is not a click. An `Interactive` hears that
cancellation, and a second finger landing, as `on_cancel`, which is where a
gesture in progress is dropped rather than committed. A finger dragged across a
`Interactive` is read as a scroll of whatever holds it that scrolls that way
(where nothing does, it stays the catcher's drag) unless the catcher sets
`touch_drags`, which a canvas that draws or moves things under the finger does,
or `touch_drag_axis`, which keeps finger drags along one direction only and
leaves the other to the scroll around it, as a dock tab in a scrolling tab bar
does.
A finger that lands on no control is taken to the nearest `Interactive` that
takes presses within `TOUCH_REACH` of it, for the whole touch, so every control's
touch zone is bigger than it looks without anything growing; a direct hit always
wins, so a neighbour never takes a tap aimed at the control beside it. A
catcher that only watches presses over a whole region - a dock pane focusing
itself - sets `claims_touch=false`, so landing on it is not a direct hit and the
controls inside it keep their reach. A quick
tap with two or more fingers that did not move is a finger tap, which
`on_finger_tap` hears the way `on_shortcut` hears keys; the editor frame's top
bar undoes on two and redoes on three. `Sheet` is the panel that rises from the
bottom of a narrow screen. It scrolls what it holds itself, so what goes in it
is not wrapped in a vertical `Scroll` (one inside would measure nothing tall): a
swipe anywhere on it raises it to its top stop before it scrolls the content,
and lowers it once the content is back at its start. It is never taller than
what it holds, whatever stop it rests at, and pulled past its top it stretches
like an overscroll. It slides up as it opens, easing out, and slides down as it
closes, easing in from however fast it was flicked; a `ModalSheet` stays up
while it leaves however it was closed, and fades its scrim with it. Let go, it springs to the
stop nearest where the flick was heading, or closed below the lowest one, and
its content flings and bounces at its ends as a `Scroll` does; the handle does
the same for a mouse, and going back closes it.
The styled
module supplies themed buttons, icon buttons, menu buttons, links, text styles,
cards, checkboxes, switches, choices, text and number inputs, a multiline text
editor with its find and replace bar, menus, popovers, split buttons, tabs, trees, a calendar,
date and time fields, a color picker and a color input,
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

Layout already skips the work for nodes far from the view. Every node is
still measured, since sizes need them all, but `layout::layout` *culls* a node
whose rect ends more than `CULLING_MARGIN` outside the visible part of its
parent's painter: it keeps its rect, so `node_rect`, `reveal_node` and
placement signals still find it, but nothing inside it is laid out, painted or
interacted with (`Document::is_culled` says which). This holds for the children
of any container, and the node holding the focus and its ancestors and
descendants are never culled. Each placement records its `Sight`, the range of
visible regions it stays right for (a node that reads its space through the
painter is right only exactly where it was), and a parent's sight takes in its
children's, so a scroll lays a subtree out again only once a node in it crosses
the margin. A long scroll of ordinary content needs nothing more than this;
reach for a `VirtualList` when building every row is itself the cost.

A `VirtualList` is a box like any other. It takes `keys` the way `ForEach` does,
reports one `item_size` per key as its own length, and builds only the rows that
its slice of the enclosing viewport
reaches, so it goes wherever a tall child would: directly under a `Scroll`,
beside plain siblings in one, or nested a few containers deep inside one.

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

Rows belong to their keys, not to their positions. Scrolling reuses the rows
that stay in view and disposes the effects of the ones that leave, and the item
builder runs once per key for as long as that row stays in view. Changing `keys`
keeps every row whose key is still there: inserting a key anywhere builds only
its row, and only if it lands in view; removing one disposes only its row; and
the sizes the list measured move with their keys. A key inserted or removed
above the view changes the length above what is shown, so the list moves the
scroll's offset by that much and the rows on screen stay where they are. A
list over data that is only ever addressed by position can pass
`(0..count).collect()` as its keys, the way a `ForEach` over indices does.

A row therefore follows its data the way any component does, by binding
signals rather than reading them once while it is built. Changing `item_size`
keeps the rows too, but forgets the sizes the list measured, on the assumption
that the rows changed size with it; the rows in view are measured again as they
are laid out.

A list the layout stops reaching - scrolled out of view beside other content, or
inside a scroll that collapsed - is told so through `Element::unplaced`, which
the layout calls on every node it placed last pass and did not place this one.
The list releases its rows there rather than holding a screenful it cannot see.

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

`ScrollbarStyle::fading(length)` also fades the content out toward each edge
that has more content beyond it, over at most `length` points and only as far
as the content has scrolled, so a scroll whose rows happen to end exactly at
its edge still reads as scrollable. The styled layer's scrollbar style fades by
`theme::SCROLL_FADE`, so `styled::Scroll` and every styled control that scrolls
fade. `unstyled::Scroll` works the widths out from the position its offset
reports and wraps the offset in a base `Fade`, which puts them on the entry of
its child (`Painter::faded`, `Entry::fade`) the same way a clip is put there:
the wgpu renderer multiplies the alpha of everything in that space by it in the
shader, and the DOM renderer masks the child's frame with a gradient. Shapes a
node paints itself and custom `Drawing`s are not faded.

Under both sits the base `Offset`, which is named for what
it does rather than for what it is used for: it shows its one child along a
direction from an offset, with no bar, no theme, no fade and no input of its
own. A wheel, a touch drag and the arrow keys are `unstyled::Scroll`'s,
which puts its children in a `List` inside the offset, wraps that in a
focusable `Interactive` for the keys, the wheel and the drag, keeps the
momentum an unfinished fling carries, and drives the offset from all three. So reach for `Offset` when something needs its
content shifted under a viewport and nothing more, and for a `Scroll` whenever
something needs to scroll.

That offset is anchored to a node, not measured from the top of the content:
the scroll remembers the node at the top of its viewport and where it started,
so a row further up growing or shrinking - a wrapping label, an image that
finished loading - leaves what is being read exactly where it is. It finds that
node by walking down from its child through the nodes that lay their children
out in place (`Element::passes_scroll_anchor`: `List`, `Frame` and `Fade`),
taking the first child that reaches past the top of the viewport each time.

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

A slider given a `thumb` length maps the pointer to the thumb's centre, which
travels the track inset by half of it, and a press that lands on the thumb
drags it from where it was grabbed rather than jumping it to the pointer;
`ColorArea` takes the same prop for its two axes.

The curve applies everywhere the value and the track meet. Dragging maps the
position under the pointer through it, the knob sits where the value falls on
it, and keyboard steps move by a share of the track rather than a share of the
range, so an arrow key near the fine end moves a little and the same key near
the coarse end moves a lot. What a screen reader is told the step is follows
the value the next step would actually reach.

### Dates, times and colors

`beui::datetime` holds the plain values the pickers trade in: a `Date`, a
`Time` to the minute, a `DateTime` of the two, a `Weekday` and an `HourCycle`.
They carry no time zone; `DateTime::from_unix` and `to_unix` read and write
seconds as UTC, which is what block content stores.

`styled::DateTimeField` is the field for any of them, chosen by `parts`
(`DateTimeParts::Date`, `Time` or `DateTime`). It is a row of spin-button
segments, one per year, month, day, hour, minute and (on a 12-hour
`hour_cycle`) AM/PM, beside a button that opens a popover holding a
`Calendar`, a `TimeList`, or both. A click anywhere in the field, or Alt+Down
in a segment, opens the same popover over the field with an editable copy of
the segments on top, focused on the segment that was clicked, so typing goes
on while the calendar shows the month being typed; closing it puts the focus
back on the field's own segment. The popover is laid out for what it holds: a
calendar that grows with the field, a `TimeList` laid out as a grid with an hour
to a row, the two side by side, or, on a narrow screen, Date and Time tabs where
picking a day moves on to the time. Its `value` is an `Option<DateTime>`: an
empty field shows placeholders, a field is reported through `on_change` only
once every segment is filled, and a field left half filled goes back to its
value when the focus leaves it. A `Date` field keeps the time of the value it
was given, and a `Time` field the date. `styled::Calendar` is the month grid on
its own, with `min` and `max` limits; its title is a month button and a year
button, which open a grid of months and a grid of twenty years, and `show`
moves it to a month without selecting anything. All of the field's behaviour,
the popover and its focus and what each pick does, is
`unstyled::DateTimePicker`; the styled field gives it faces and the popover's
layout, and says when to page with `paged`.

`styled::ColorPicker` is a saturation and brightness area, hue and opacity
sliders, hex, RGB and HSL fields and a row of swatches; `styled::ColorInput` is a
hex field whose swatch opens one. Both keep the hue while the color passes
through grey or black, and a drag is reported through `on_preview` while it
moves and through `on_change` once, when it ends, the way `NumberInput`
reports a scrub - so an edit lands in the undo history once per gesture.
That state is `unstyled::ColorPickerState`, and `unstyled::HexText` keeps a hex
field in step with a color for both.

`styled::ColorWheel` is a hue ring around a triangle whose tip points at the
hue, and `styled::OklchColorWheel` is the same wheel over OKLCH: the triangle is
the lightness and chroma plane, black to white with its tip at the hue's cusp
lightness (`Oklch::cusp`) and a chroma past every hue's cusp, so it holds all of
sRGB. Turning the hue keeps lightness and chroma. A point outside sRGB is painted
and reported clamped to the most colorful color at its lightness and hue
(`Oklch::clamped`), and a line on the triangle marks where sRGB ends. Both share
`unstyled::ColorWheel`, which works in `WheelPoint`s (a hue and the triangle's
saturation and value) and leaves the color model to its caller, and
`ColorPickerState` is generic over the model (`Hsva` or `Oklch`). The faces are
images rasterised on the CPU at the screen's pixel density, so they work on
every renderer.

`unstyled::Popover` is what both open: a trigger and a modal overlay, built the
first time it opens, that traps Tab, closes on Escape, a press outside or its
handle's `close`, and gives the focus back to the trigger when it closes. It
dims nothing, so it is a `light` overlay: the pointer goes on hovering the
document around it, and a press outside closes it and then lands on whatever
was pressed - except a press on its own trigger, the overlay's `trigger`, which
is left to the trigger so a second click closes it rather than reopening it. What
it holds decides where the focus lands when it opens, by binding a `focused`
prop to the handle's `open`, as the calendar, the time list and the color area
all take.

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
describes clicks the control. The dwell is a `create_timer`, started when the
pointer arrives and stopped when it leaves, so a tooltip nobody is hovering
costs no frames.

### Time: timers and animation

beui has no per-frame hook. Work runs because a signal it reads changed, and
anything that genuinely depends on time asks for it with `create_timer(work)`.
The timer belongs to the reactive scope that created it and is dropped with
it. `start(delay)` schedules `work` (keeping an earlier deadline if one is
already set), `restart(delay)` replaces the deadline, `stop()` cancels it, and
`running()` says whether one is pending. The document runs due timers inside
its reactive scope at the top of a frame and asks the context for a repaint
at the next deadline, so nothing is drawn while no timer is due. `work`
returns `Some(delay)` to run again - an animation returns
`Some(Duration::ZERO)` for the next frame - and `None` once it has settled,
which is what lets a scroll fling or a spinner stop costing frames. Start a
timer from the event or effect that begins the motion rather than leaving one
running.

The context's `Motion` (`Context::set_motion`, mirrored into every document it
shows as `document.motion()` and the signal `watch_motion()`) says how much may
move. `Animated` is the default. `Instant` has no animations: a slide or snap
goes straight to its end (`rubber_band::animation_step` returns an elapsed time
that finishes any animation), and there is no fling, rubber-banding, caret blink
or spinner motion. `Still`, for e-ink, also stops content from following a
held gesture: a scroll drag, a window drag, a sheet pull or a back swipe only
lands when it is released. An animation added to beui checks it. The host sends
it to plugins in the protocol's `Theme`, and the plugin adapter applies it to
the plugin's context. The inspector's Simulation tab sets it.

`pixels_per_point()` is the document's scale as a signal, for layout that
depends on it.

### Overlays, and things that float

An overlay is laid out and painted above the rest of the document rather than
among it, and it comes in three modes.

A **modal** one - a menu, a select popup, a dialog - takes the document over
while it is open (a `light` modal one lets the pointer through outside itself,
and a press there closes it and still lands): it goes on the overlay stack, so input reaches it and
nothing else, it can trap focus, Escape closes the topmost one, and a press
outside it dismisses it.

Back - Android's back gesture, or the Back key or mouse button - goes to the
most recently made enabled `BackHandler` inside the topmost modal overlay, or,
with no modal open, outside every overlay; with no such handler it closes the
topmost modal. A base `BackHandler` only routes: it calls `on_back` when back
completes, or hands every phase of the gesture to `on_gesture` if it has one,
and moves nothing. The motion lives above it. `unstyled::BackSlide` wraps a
page: held, the page follows the finger across a good share of the screen;
let go, it carries on off the edge before `on_back` runs and the next page
slides in behind it, and a cancelled gesture eases it back. A back with no
gesture before it (a key) goes back at once. `styled::Dialog` and
`Fullscreen` slide away the same way and fade their scrim, and a `Sheet`
sinks with the gesture and slides on down from where it was. The document
reports whether anything would take back through
`FrameOutput::handles_back`; on Android the runner passes that to the
activity's `setBackHandled`, and when nothing takes it the system's own back
(to the home screen) plays instead.

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
wherever it decides they belong. The arrow keys, Home, End and typing move the
keyboard between rows without selecting them, and Enter or Space selects the
row it is on; `selection_follows_focus` makes every move select, as the beui
inspector does. `toggle` puts the keyboard on its row too.

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

Choosing a row never expands, collapses or scrolls anything: only the chevron
and the arrow keys do. The styled tree owns its `Scroll` (`padding` goes inside
it), and follows the `selected` key wherever it is instead. Given `ancestors`,
which answers with the keys above one, outermost first, it marks the chevron of
the deepest row still shown above a selection hidden inside a collapsed one, and
while the selection is hidden or scrolled out of view it floats a button over
the top or bottom edge that reports `on_reveal` - the caller expands the
ancestors - and then scrolls the row into view once it is laid out. The file
tree and the beui inspector both work this way. `styled::tree_row_node` and
`styled::tree_focused` reach the rows through the styled tree's node.

### Docking and windows

`styled::Docking` is the workspace layout: panes split from one another, a tab
bar on each pane, and tabs that can be dragged between panes or out into
windows that float over the rest of the dock. `unstyled::Docking` underneath it
owns the tree, the dragging and the keyboard, and paints nothing; the demo in
`crates/beui-demo` is laid out in one.

The children describe the layout the dock starts with and the tabs that exist;
a `DockingLayout<K>` holds where everything is now, and is what the caller saves
and restores:

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

`DockSplit` takes two children, `fraction` being the first one's share.
`DockPane` holds `DockTab`s and `DockGroup`s (a tab holding a tree of its own,
`pinned` to keep its tabs in it), names its starting `active` tab, and can start
with its tabs in a sidebar (`vertical`, `sidebar_width`). `DockWindow` floats one
tree at a `rect`. A tab's key is any `Clone + Eq + Hash` type; a key that is a
`TabId` or a `u64` names the tab's `TabId` too, which is what the test ids of its
close button and switcher row carry. Every container has an `id`, saved with the
layout, so that a layout saved by older code still finds its panes.

The children are the full set of tabs: a tab the code no longer declares is
closed, and a tab it starts declaring is placed. Closing is a request -
`on_close` gives the tab its close button, and the tab goes when the caller
stops declaring it, or stays, as the workspace keeps a program's window until
the program quits. A tab that appears joins the pane it was declared in, after
the tab declared before it, and is shown. If the user has closed that pane, it
joins the declared tab nearest it that is still open, then a window the code
declared it in, and failing both it floats in a window of its own. A pane with an
`empty` view is never closed: emptied, it shows that view, and dragging its grip
onto another pane moves its tabs and leaves it behind. A split follows the
`fraction` in the code until the user moves it. A pane, split or window the
code adds is placed beside its declared sibling when the saved layout lacks it.

`DockingLayout::snapshot` is the `DockingSnapshot` to save, and `restore` puts
one back; restore it in the same `batch` that declares its tabs, since a tab not
declared when the dock reconciles is closed. `reset` goes back to the layout the
children describe. The handle also shows a tab (`show`) and reads which is
focused (`focused`, `shown` for a stacked dock). A tab that stops being declared
for a moment - a `Show` around it that flips, a `ForEach` rebuilt from new keys -
loses its place, so keep a tab's declaration in one stable place.

The `DockState` the layout holds is a plain value underneath:
`state()` reads it, and `find`, `all_tabs`, `focused_tab` and `surface_tabs`
answer from it.

Tiled, `Docking` keeps its panes inset from its own edges.
`mode=DockMode::Stacked` draws the same state as one screen: the focused tab
fills the dock, edge to edge, with no tab bars, splitters or windows, and everything else in
the state is kept, so switching back to `DockMode::Tiled` restores the layout.
The dock draws the stacked screen's bar itself: the tab's icon and title, a
square counting the open tabs other than home, which opens a sheet of cards to
show or close them, and a back button when the caller names a `home` tab
(`home={Some(FILES)}`), which the back gesture also follows and whose own screen
has none. A tab's icon comes from the optional `icon` function, an icon-font
glyph (empty for none) that the tab bars, the drag preview and the stacked bar
all draw. The bar is the dock's, not the caller's; what a
tab adds to it is only its own actions, which its panel hands over while it is
built with `dock_actions(node)`, and which are shown while that tab is, and its
menu, which the panel hands over with `dock_menu(actions)`, a `Memo<Vec<Action>>`.
The menu is not tied to the stacked bar: a tiled pane offers the menu of the tab
it shows behind a More button just before its close button, at the end of its
tab bar or at the top of its sidebar, and so does a window. A menu is a list of
actions rather than a node, so a dock that lays out tabs built somewhere else
can draw it from rows it was sent, with actions made by
`ActionBuilder::detached`, which runs without being registered for shortcuts or
the palette, and pass the pick back.
Each tab's panel is built once and moved between the two, so what it holds
survives the switch. `recent_tabs` lists the tabs from the one shown last (the
order is part of the state, so it is saved with the layout), and
`stacked_tab` is the tab a stacked dock shows: the focused one, or the one
shown last when the focused pane is empty. The workspace stacks its dock on a
phone.

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
instead, which is also what "Pop out into a window" on a tab's own menu does. Every pane
and every window wears the same bar: a grip, the tabs, and a button that closes
them all, shown only when every tab in it can close. A middle click on a tab closes it. A finger picks a tab up by
dragging it out of its bar, across the way the bar scrolls; sliding along the
bar scrolls it. Dragging a docked pane's
grip carries the whole pane (`DockState::drop_leaf`), with the same drop targets
a tab has. A window holds one pane, so a tab dropped anywhere inside one joins
it rather than splitting it, and that pane's bar is the window's title bar:
anywhere on it that is not a tab drags the window. Right-clicking a grip moves
that pane's tabs into a sidebar beside its body (`DockState::set_vertical`),
whose edge drags or arrows to a new width (`set_sidebar_width`); a window in
that mode has no title bar, only the sidebar with the grip and the close button
at its top. Windows resize from any of
their eight grips and are raised by whatever takes the focus inside them. A
split keeps each side at least `MIN_PANE_LENGTH` where the area has room for
both, and a window is drawn no larger than the dock, so a layout made on a wide
screen stays usable on a phone. Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of the pane the focus is in,
registered with `on_shortcut` so they arrive even from inside a text input in a
panel. The bar between two panes is a tab
stop with a `Splitter` role: the arrow keys move it, and the tab bar is a
`Choice` inside a `Scroll` running the way the bar does, so the arrows, Home and End walk it like
any other tab list and scroll the tab they reach into view when a pane has more
tabs than it has room for.

A place in a tab bar holds an `Entry`: either a `Tab` or a `Group`. A group is a
tab that holds a dock tree of its own, so choosing it shows that tree in the
pane's body - one pane with a second tab bar under the first, or panes split
side by side - and the same drops work inside it as anywhere else. Dropping a
tab onto the middle of another tab groups the two (onto a group, it joins the
group); the outer edge of a pane showing a group still splits the outer pane,
so the drop zones of the group sit inside a thin band that belongs to its
parent. A tab's menu offers "Group with next tab" and "Split with next tab",
and a group's own menu ungroups it, closes every tab in it, or floats it into a
window as a whole. The tree tidies itself after every change: a group left with
a single tab turns back into that tab, and a tab bar left holding only a group
takes the group's tabs, or its split, in its place, so nothing is nested for
longer than it holds more than one thing. `entries`, `active_entry`, `locate`,
`group_tabs`, `tree_leaves`, `surface_of` and `is_nested` read groups back, and
`drop_entry`, `group_with_next`, `split_with_next` and `ungroup` change them;
`layout_tree` lays a group's tree out the way `layout_surface` lays out a
surface's. `find`, `all_tabs`, `surface_tabs` and `show` look through groups,
and showing a tab inside one selects the group in every bar above it.

A pinned group is one the tree never tidies away: `insert_pinned_group` puts
one in a tab bar, and it stays a group while it holds one tab or none, cannot be
ungrouped or closed from its menu, and admits only the tabs whose home it is -
the tabs it was given - so dragging any other tab onto it or into it does
nothing. Its tabs are pinned to it as well: one can be rearranged anywhere
inside the group, and the group can be moved with all of them, but a pinned
tab cannot be dragged, popped or carried out of it with its pane. "Unpin from
group" on a tab's menu (`set_tab_pinned`) lets it go, it may come back later,
and "Pin to group" pins it again once it is back. A drag that the state would
refuse (`admits`, `admits_leaf`) shows no drop marker, and letting go there
does nothing. `unpin` makes the whole group an ordinary group again. `tree` reads a surface's or a group's layout
out as a `DockTree`, which names tabs but no leaf, split or group ids, so it can
be compared, sent elsewhere and rebuilt: `from_tree` makes a state out of one
and `set_tree` replaces a tree in place, moving in any of its tabs that were
elsewhere and keeping the focused tab focused. `group_title` names a group in
its tab.

A tab's panel is built the first time the tab is shown and belongs to the dock
rather than to the pane showing it: the pane holds a `Portal` pointed at it, so
the panel keeps its nodes, its scroll position, its caret and its state when
the tab is hidden behind another, dragged to another pane, or floated into a
window. A panel no pane is showing is laid out by nobody, so it costs nothing
and a screen reader does not read it. Closing the tab is what removes it.

### Spinners

`styled::Spinner` is an indeterminate progress bar. It animates only while it is
laid out: a spinner behind a `Show` that is false, or in a tab nobody is
looking at, asks for no frames. It uses `node_placed`, which works for any
component that should only work while it is on screen.

### Drag and drop

Moving something from one place in a document to another is
`unstyled::Draggable` on the thing being moved and `unstyled::DropTarget` on
each place it may land. Nothing else is needed to wire them together: every
document keeps one drag board, a draggable puts its payload on it, and the
targets whose payload type matches hear about it.

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

The payload is a `Prop<Option<P>>`, so a source that may not be moved right now
- a read-only block, a disabled slot - binds a memo that answers `None`. A
press only becomes a drag once the pointer has travelled `threshold` from where
it went down, and a press that never does is reported through `on_click`
instead, so a row that is both clickable and draggable does not select itself
at the end of a drag. `on_drag_change` says when a drag starts and ends, which
is what a source that dims itself while it is being carried binds. Draggables
that overlap or nest - a fanned hand of cards, a card inside the pile that
holds it - set `capture_presses`, so a press reaches the topmost one under the
pointer and nothing beneath it, the way `unstyled::Button` wins a press.

While a drag is under way the `preview` is shown beside the pointer in a
passive overlay, so it paints above everything and takes no input. The pointer
it follows is the one the document saw, not the one the draggable's own
catcher saw: the board is fed the pointer at the start of every frame, even
where a floating overlay covers the source, so a drag carried over a floating
window keeps going. A finger drag waits for `TOUCH_DRAG_THRESHOLD` and for the
catcher to hold it rather than a scroll, so a Draggable in a scrolling list
takes finger drags only where it sets `touch_drags` or `touch_drag_axis`, and
its preview sits above the finger instead of under it.

A `DropTarget` is registered by the rectangle its content was laid out at. The
target under the pointer that accepts the payload takes the drop - `accepts`
filters by value, and a target of another payload type never sees the drag at
all - and when targets nest, the innermost one wins, so a slot inside a column
inside a sidebar can each take a drop of its own. Its `DropHandle` carries
`carrying`, true for every target that would accept what is being dragged, and
`over`, true for the one that would take it now; `on_over` reports the payload
and the pointer as it moves, which is how a target that works out where inside
itself the drop lands - the dock's tab bars and split zones, a timeline's
insertion point - draws its marker, and how a filmstrip reorders live. The drop
is handed over before the source hears `on_drag_change(false)`, so a source can
keep whatever it set up for the drag until the drop has used it.

A drag between editors is not this: a block dragged from the file tree to
another editor crosses plugins, so it goes through the host with
`Editor::drag` and `accept_drag` (guides/adding_a_plugin_editor.md).

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

### Picking a file

`beui::reactive::create_file_picker(picked)` gives a component a `FilePicker`:
`open(FileFilter::new(name, extensions, mime_types))` asks the platform for one
file, `picking()` is a signal that is true until the answer arrives (a button
takes it as `disabled`), and `picked` is called with the `PickedFile` (its name
and bytes) or with why it could not be read; a cancelled pick only clears
`picking`. `pick_file(filter, picked)` is the one-shot form underneath, handed
the raw `FilePick`. Nothing blocks while the dialog is open: the document queues
the request, `Context` hands it to the runner in `FrameOutput::file_picks`, and
the runner answers with `Context::file_picked` whenever the person is done -
winit from a thread that runs the native dialog, the web runner from an
`<input type=file>`, Android from `BeuiActivity`, and a plugin through the host
it runs in. The document delivers the answer at the start of a later frame,
inside its reactive scope, so `picked` can write signals like an event handler.
A host that runs beui itself does the same with the requests in `file_picks`.

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
tracer, a map - is a `Drawing` node whose callback paints a gpu `Drawing`:
`draw_gpu(drawing)` builds one that paints a `Shape::Drawing` over the node's
rectangle. The renderer runs the `Draw` behind that drawing where the shape sits
in the order everything else is painted in, so nodes written after it still
paint over it.

```rust
view! {
    <Drawing
        draw={Prop::Dynamic(Rc::new(move || draw_gpu(drawing.get())))}
        @sizing=ItemSize::Percent(100.0)
    />
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

`beui::drawing` wraps a `Draw` in a `Drawing`, and two drawings are the same shape when they
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
use beui::styled::{Button, ButtonVariant, Display, DocumentTheme, use_theme};
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

Run the demo with:

```text
./scripts/buck run //crates/beui-demo:demo
```

Beui's features:

- No features provides the document, components, layout, input model, and
  painting output. This is enough for headless logic tests.
- `render` adds the wgpu renderer without creating a window. Embedded hosts use
  this level.
- `window` adds the native runner and enables `render`; it is the default.
  On the desktop it is winit's. On Android it is beui's own
  (`crates/beui-adapter-android`, with its Java in its `android` directory): the app's
  activity extends `com.be3.beui.BeuiActivity`, whose `BeuiView` is the
  surface, takes touches, keys and the soft keyboard's input connection, and
  hosts AccessKit. It has to be a view that input reaches through the view
  system: TalkBack's touch exploration arrives as hover events on it, which
  NativeActivity's input queue never delivers. The library defines
  `#[unsafe(no_mangle)] fn android_main(app: beui::AndroidApp)`, which beui
  calls on a thread of its own, and which calls `beui::run_with`.

The soft keyboard takes its room out of the window rather than covering it: on
Android its inset joins the safe area, and on the web the pages' viewport meta
asks for `interactive-widget=resizes-content`. When the rectangle a document is
shown in changes size while the focus takes text, the document scrolls the
focused field into what is left, through every scroll it sits in.
- `web` and `dom` add the browser runner,
  `beui::run_web(element_id, renderers, options, app)`, which loads each
  `beui::WebRenderer` it is given into the element, shows the first that
  loads, and skips (with a console warning) any that fail. `web` enables
  `render` and adds `WebRenderer::Wgpu`, which draws into the element if it is
  a canvas and into a canvas it adds otherwise.
- `dom` adds `WebRenderer::Dom { icons_font }`, which draws with DOM
  elements instead of wgpu. Layout, input and focus are beui's as everywhere
  else. Each `Display` in the frame's display tree keeps one element across
  frames (by `Display::key`), holding its shapes and its children's elements
  placed and clipped by their `Entry`; a display whose `Rc` is unchanged is not
  looked at, so scrolling moves the rows' elements and rewrites nothing
  inside them. Text is measured and drawn with the browser's fonts
  (`beui-font-browser`), and the browser fetches the icon font from the URL
  `icons_font` names. A `Drawing`, a `Punch` and a `Filter` draw nothing there.
  `crates/beui-web-demo` is the demo with both renderers, DOM first:
  `./scripts/buck run //crates/beui-web-demo:web-serve` serves it on
  http://127.0.0.1:8070.

`beui::run_with` takes `RunOptions` (title, app id, starting size) where
`beui::run` takes only a title; both load wgpu, and
`beui::run_with_renderers` takes the `beui::WindowRenderer`s to load instead
(block-app's opens its device itself, to import Wayland clients' buffers), and
`beui::run_on` runs on an adapter of the caller's choosing, such as
`beui_adapter_drm::Drm`, which drives the displays and input devices itself. The rest of
`App` is optional:

- `setup(&Setup)` runs once, after the gpu exists and before the first frame.
  `Setup` hands over a `Waker`, and whatever the renderer and runner provide:
  `setup.get::<beui::GpuSetup>()` is the wgpu device, queue and surface format,
  for an app that paints with the gpu itself through `draw_gpu`, and on the
  desktop `setup.get::<Arc<beui::winit::window::Window>>()` is the window. `Waker::wake`
  can be called from any thread, and asks the runner for another frame: it is
  how work finishing elsewhere is pushed to the ui instead of polled for.
- `close_requested` is asked when the window is closed, and can refuse by
  returning `false` (to ask about unsaved work first, then call
  `Context::close_window`). `exiting` runs once on the way out.

From inside a frame the app can also ask the window for things through the
`Context`: `set_fullscreen`, `set_ime_area` for an input it draws itself,
`set_zoom_factor`, `close_window`, and `retain_events` to take events away from
the document before it sees them, which is how an app that hosts its own
surfaces (block-app hosting plugins) keeps a key meant for a plugin away from
the focused beui control.

A runner hands the document its events in batches, one per frame. A press that
follows typing waits for the next frame, so text typed before a click always
reaches the field it was typed into, however many events arrive between two
frames.

The web runner draws into the canvas it is given with WebGPU, or WebGL where
the browser has no WebGPU. It reads the keyboard through a hidden text area,
so pasting and composition behave like any other input on the page. Like the
desktop runner, it only asks the browser for a frame when an event arrived,
something asked for a repaint, or a `Waker` was woken.

### The inspector

Ctrl+Shift+I in a standalone beui window opens the node, component,
accessibility, and performance inspector. Its Comp tab lists the
`#[component]`s that built the tree rather than its base nodes: every
component records its name against the node it returns, so components that
return the same node nest there, outermost first. Ctrl+Shift+C enables node picking. Ctrl+Shift+F moves
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
paints a cursor the document reacts to, in the shape of the frame's
`CursorIcon`. One finger moves the cursor, and two fingers scroll smoothly. A
tap presses the primary button where it lands and holds it until the double-tap
timer runs out, which makes it a click. A press within that time keeps the
button down: moving it drags from where the tap landed, and lifting it in place
locks the button, so that the drag carries on across touches until a single tap
releases it. A stroke that moved the cursor is never a tap. The strip along the bottom holds the left, middle,
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

### Responsive design mode

The Sim tab's "Responsive design mode", or Ctrl+Shift+M, lays the document out
in a screen of a chosen width and height in points, the way a browser's
responsive design mode does. A toolbar above the app picks a device preset,
types the width and height, rotates the screen and sets its zoom: Fit shrinks
it to the room beside the inspector but never grows it, and a percentage draws
it that many real points per simulated point. The screen sits centred at the
top of a backdrop, with handles on its right edge, bottom edge and corner that
resize it; a drag holds the scale it started at, so the handle stays under the
pointer, and the screen refits on release. A screen drawn larger than its room
follows the pointer: the point under the pointer is always the point at the
same fraction of the simulated screen, so moving the pointer across the room
looks around the whole screen. The toolbar is a document of its own that spans
the toolbar and the room below it, so its selects can open over the app, and
the app gets no pointer while one is open. The state lives in the `Context`,
so the screen stays simulated after the inspector closes.

`Document::show` does it with `Context::scaled` and `Context::clipped`, laying
the document out at a rectangle chosen so that a real point is always the
document point times the scale; panning moves where the document is laid out
instead of adding an offset. That keeps every mapping a pure scale, which is
what an app that reads input or places surfaces outside `Document::show` needs:
`Context::screen_scale` and `Context::screen_input` give it the scale and the
frame's input in document points, the way block-app's host reads them for its
plugin surfaces.

### Filters

The Sim tab's Filters section puts a blur, a contrast reduction, and the
colour vision simulations over the shown rectangle. They are independent of the
screen reader simulation and of each other, and they need no cooperation from
the document: they are a post-processing pass in `Renderer`, so whatever a
document paints - a plugin's beui pane included - is filtered the same way.

`Context::apply_filter` is what turns them on. It records a `Filter` (the region
in points, a blur radius, a contrast multiplier, and a `ColorVision`) and how
much of the frame has been painted so far, which splits the frame in two: everything
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

While the simulation is on the document answers no pointer input directly, so a
click lands nowhere and the only way to reach something is the simulation. The
keyboard is split the way a platform screen reader splits it: the reader's
commands all hold Alt, and every other key goes to whatever the document has
focused. Landing on a control focuses it, so walking to a text field and typing
types into it, and the arrows, Enter and Space do what the focused control does
with them. The reader follows focus in turn: when the document moves focus on
its own - Tab, Shift+Tab, a dialog opening - the reader moves to the control
that took it and reads it. Keyboard and touch drive it at the same time, with no
mode to pick between them. From the keyboard, Alt with the left and right (or up
and down) arrows walks an item at a time, Alt+Shift with the arrows moves
between controls, Alt+Home and Alt+End jump to the ends, Alt+Enter or Alt+Space
activates, Alt+Minus and Alt+Plus adjust, Alt+Page Up and Alt+Page Down scroll,
and Alt+R repeats the current item. The document never sees those keys. Walking
past either end says so and reads the item again after it, so the readout never
leaves you without the thing you are standing on. By touch, dragging a finger reads
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
- `@sizing` selects the child's `ItemSize` or `Sizing` among its siblings in a list, and
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

Use `Frame`'s `width` and `height` props (and their `min_`/`max_` forms) to constrain a component's own size,
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

`Document::on_interacted` runs a callback once the frame's input has been
dispatched and before anything is laid out, for an app that wants work started
by that input under way while the document lays out. The one exception to the
single layout is `Document::on_laid_out`, a callback that runs inside the
document's reactive scope between layout and paint, for an app that has to
answer something layout decided: block-app waits there for its plugins to draw
at the sizes they were just given. Whatever it changes is laid out again before
the frame is painted, and a layout that moved something runs the callback again,
a few rounds at most.

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
`block_editor_beui::BeuiApp` and uses `block_editor_beui::beui_plugin!`.
The type it names holds no state: the
framework builds the view once, keeps the `Document` it produced, and shows it
every frame.

```rust
#[component]
pub fn Counter(editor: block_editor_beui::Editor) -> NodeId {
    let counter = editor.block_content::<CounterContent>();
    let count = counter.field(ObjectId::ROOT, CounterModel::COUNT);
    let increment = clone!(counter -> move || counter.operate(CounterModel::add(1)));
    view! { ... }
}

pub struct CounterApp;

impl block_editor_beui::BeuiApp for CounterApp {
    fn view(editor: block_editor_beui::Editor) -> NodeId {
        view! {
            <Counter editor={editor} />
        }
    }

    fn create_block(creation: &block_editor_beui::Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CounterContent::default()))
    }
}

block_editor_beui::beui_plugin!(CounterApp, "../manifest.json");
```

`Editor` is everything the instance was given: the host, the block, the
`Blocks` handle it reaches the graph through, the view the host is showing the content through, and
signals for everything the host tells it. The
host supplies input, fonts, clipboard integration, rendering, and the frame
rectangle. A plugin normally depends on beui without the window runner:

```toml
beui = { path = "../../beui", default-features = false, features = ["render"] }
```

Block content reaches the view through a `ContentProjection`:
`editor.block_content::<C>()` holds the content of the editor's own block,
`project`, `field`, `ids` and `object` derive signals from it, and `operate`
applies an edit and sends it to the host. The framework pumps every projection
once at the top of the frame, inside the document's reactive scope, and
`read` and `revision` track, so a memo or effect that reads them runs again
when the content changes. See the [reactive guide](reactive.md#blocks).

Nothing in an editor polls. What the host sends between frames is recorded
by the framework and delivered at the top of the next frame as signals on
the `Editor`: `replies()` ticks when the host answers a request (a file or
block pick, a pasted image, a fetch), and `on_reply(f)` runs `f` each time;
`drag()` and `files()` carry a drag or a file drop over the editor;
`audio()`, `history(block)`, `histories()`, `artifacts()`, `focused_block()`,
`web_view_events()` and `peers::<P>()` follow the host's state;
`block_types()` tracks the catalog; `placed()` is the content rect as a
signal. Work on another thread takes its `Waker` from `editor.woken()`,
whose signal ticks once the work has woken the editor, and an effect that
reads that signal collects the result.

The counter editor under `crates/editors/counter` is the reference integration.
The [plugin editor guide](adding_a_plugin_editor.md) covers the manifest,
creation flow, host connection, and current beui plugin capability limits.

A template the manifest marks `"dialog": true` is made through `creation_view` instead, one
more `#[component]` function that the framework builds a separate document of
and shows in the picker's creation dialog. It says what the dialog makes with
`creation.on_create(...)` and answers `creation.set_ready(true)` once it has been
filled in. Host services such as `BlockPicker` work there too, collected from
`creation.on_reply(...)` when the host answers.

## Develop an unstyled component

An unstyled component owns semantics and interaction, not appearance. Put it in
`crates/beui-components-unstyled/src/<name>.rs`, declare it in `lib.rs`, and re-export
the public component, handles, state readers, and supporting types there.

Compose it from base components. For an interactive control this normally means:

1. Model controlled values and transient interaction values with signals.
2. Use `Interactive` for pointer and touch interaction, and give it
   `focusable=true` for tab order, keyboard events, activation, and focus
   state. One node does both, so a control is a single `Interactive`.
4. Publish the correct AccessKit role and state with `component_accessibility`.
5. Give the caller a `Render<Handle>` or `RenderFn<Handle>` containing the
   reactive state needed to paint the control.
6. Return the root base node directly so component state and framework slots
   attach to the node callers receive.

A catcher shows the `cursor` it is given while it is hovered or held, and
one given none leaves the cursor to the catchers around it, so a catcher
that only listens - for a secondary press, a wheel - does not undo the
I-beam of the text field it wraps.

A catcher takes the wheel with `on_scroll` and a touch drag with
`on_scroll_drag`, and `scroll_axis` names the axis it takes them along, so a
vertical wheel over a horizontal strip passes through to whatever is around it
and a drag reaches the innermost catcher that scrolls that way.
`unstyled::Scroll` is built out of those three.

A press normally reaches every `Interactive` under the pointer. A control that
must win a press, or that reacts to presses outside its own rect, captures it:
`capture_presses` claims presses inside the catcher and `capture_at` claims
presses at positions its callback accepts. Before any node handles a press the
document asks the topmost nodes first, and only the captor receives it: focus
stays where it is, touch scrolling does not start, and no other catcher arms.
Paint such parts with `Painter::on_top`, which draws above the rest of the
document, or of the overlay being painted, or from a `CanvasItem` with
`clip=false`, which a canvas paints without cutting it to its own rectangle.
The touch selection handles of `unstyled::TextArea` use `capture_at` and are
carets with a handle, which `Text` paints on top; a single-line area paints
them from an unclipped item.

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

Put a styled component in `crates/beui-components-styled/src/<name>.rs`, declare it in
`lib.rs`, and re-export its public API there. An interactive styled component
wraps the matching unstyled component, passes its `label` on for the unstyled
one to put in the accessibility tree, and renders the unstyled handle with base
visual primitives:

```rust
#[component]
pub fn Checkbox(label: Prop<String>, checked: Prop<bool>, on_change: Callback<bool>) -> NodeId {
    view! {
        <Toggle checked label on_change={move |checked| on_change.call(checked)}>
            {move |handle: ToggleHandle| {
                view! {
                    <CheckboxFace handle />
                }
            }}
        </Toggle>
    }
}
```

The face component derives colors and visibility with memos over
`handle.checked`, `handle.hovered`, `handle.active`, and `handle.focused`, reads
its text from `handle.label`, then
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
when no provider covers them. Change it with `set_theme` and read it with
`theme`, both from the `styled::DocumentTheme` trait, for example to pick an
app's clear color. The
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
Put the node in `crates/beui-core/src/base/<name>.rs` and register the module in
`base.rs`, and its component in `crates/beui-view/src/components/<name>.rs`. A
base implementation has three parts:

- A node struct containing its retained state and child ids.
- An `Element` implementation for measurement, layout, painting, interaction,
  child traversal, and inspector metadata.
- `Document::create_*` and `Document::set_*` methods beside the node, plus a
  public `#[component]` wrapper in `beui-view`. The wrapper creates the node
  with `with_document` and binds reactive props to setters with `create_effect`.

`Document::create_*` returns a `NodeOf<XNode>`, a node id that carries the kind
of node it names, and the setters that reach the node's fields take that
handle, so a setter cannot be handed a node of another kind. `.id()` is the
plain `NodeId` for everything that takes any node. Code that only holds a
`NodeId` - from a `NodeRef`, or a walk of the tree - asks
`document.arena.kind_of::<XNode>(id)`, which answers `None` for a node of
another kind, and a query meant for such code takes a `NodeId` and answers
`None` or `false` itself, as `is_overlay_open` does.

`measure` takes `&self` and must not mutate; `layout` takes `&mut self` and may
update the node's own retained state, which is how a `VirtualList` realises
the rows its slice of the viewport calls for. Both receive `&mut Document` and are
reached through `beui_core::layout::measure` and `beui_core::layout::layout`, which take
the element out of the arena for the duration so an effect woken mid-walk cannot
alias it. Reach children through those two functions rather than calling another
element's methods directly, or the node you descend into is never handed its
constraint.

`unplaced` is the other half of `layout`. The layout calls it on every node it
placed last pass and did not place this one - a child its parent now skips, and
everything under that child - so a node that keeps retained state for what is
on screen can let it go. It runs in the same pass, after the dropped rects are
gone, and it may remove the node's own children, which is how a `VirtualList`
scrolled out of view releases its rows. A node that is laid out again gets
`layout` as usual and rebuilds whatever it released.

Measurements are memoised per available size and dropped whenever the node or
something under it changes its layout, so measuring a child repeatedly within a
pass is cheap, but a `measure` that is not a pure function of the node and its
constraint will return a stale answer. A memoised size also answers a smaller
offer it still fits: a node measured at a bounded width `a` to `s` answers any
bounded width between `s` and `a` with `s`, and a node measured at an unbounded
width answers an offer of exactly `s` with `s` (likewise for height). That is
what keeps a list from measuring a child again at the length it just measured,
so `measure` has to agree - offering a node more room than its size must not
change its size unless that room is unbounded.

`baseline` answers where the node's first line of text sits below its top when
it is offered a size, for a row aligned to `Align::Baseline`. It defaults to
none; `Text` answers from its first line, a node that wraps one child forwards
the child's answer through `beui_core::layout::baseline` (adding whatever it
shifts the child down by), and the answers are memoised beside the
measurements.

Layout stays linear in the number of nodes: a node measures each child a
bounded number of times per pass, and anything it computes across its children
is linear too - `base::share` hands a length out by weight between minimums and
maximums in linear time, and is what a list's shrinking and percent children
and a grid's fraction columns use. `nested_lists_measure_each_node_a_bounded_number_of_times`
holds a deep tree of lists to that.

A setter reaches its node through one of three arena accessors, chosen by what
reads the field. `get_mut_as` is for anything `measure` or `layout` reads: the
node and its ancestors are measured and laid out again. `paint_mut_as` is for
what only `paint` reads - a colour, a tint, a drawing: that node alone paints
again and nothing is laid out. `touch_mut_as` is for what neither reads -
handlers, a cursor, a tab stop: only the accessibility tree hears of it. A field
`paint` reads through something `layout` computed, like the origin a `Text`
places from its alignment, counts as layout. Only call them when the value
actually changes.

A node whose size cannot depend on its children - a `Frame` with a fixed width
and height, a `Canvas`, a canvas item - returns true from `relayout_boundary`.
A change under it stops there: its ancestors keep their placement, and it is laid
out again on its own from the rectangle and painter it was last placed with. Only
return true when `measure` never reads the children.

Every node is laid out and painted in a coordinate space of its own, starting at
its top left corner: `layout` and `paint` are handed a rectangle at the origin,
of the node's size, and the rectangles `layout` gives its children are in those
coordinates too. A node that only moves - a row pushed down by the one above it
growing, a panel beside a splitter - keeps its layout and its recorded shapes;
its parent records where it went. A scroll's `OffsetNode` and a `Canvas` add a
content space inside their own (`Document::enter_space`, and `Painter::shifted`
when painting), which carries the scroll offset or the pan: scrolling or panning
changes only that translation, so the rows keep their rects and their shapes and
neither is laid out nor painted again. `Document::node_rect`,
`node_rect`/`component_rect`, the rect `interact` is handed and a `CanvasView`'s
origin are in the document's coordinates. A node that needs to know where it sits
in them asks the painter: `painter.origin()` is where its space starts, and
`painter.clip_rect()` is what of it is visible, in its own space. Asking either
marks the node as depending on its place, so it is laid out, or painted, again
when that changes - an `Embed` reports its rectangle this way, and a
`VirtualList` realises the rows its clip shows. A `Drawing` callback that paints
in document coordinates rather than relative to the rectangle it is handed, as
`infinite_canvas` does through its camera, paints through
`painter.in_document()`.

Painting is retained per node. A node's `paint` runs again when the node
changed, when it was laid out again, when its size or the painter it is handed
changed, or when a repaint it asked for falls due; otherwise the shapes it
recorded last time stand, and its children are only visited when something
under them has to paint. `paint` must therefore be a function of the node, its
rectangle, the painter and the rectangles its own layout gave its children -
anything else it reads goes unnoticed when it changes. What a node damages is
where its shapes changed, so a repaint that paints the same thing costs no
pixels. A `Drawing` whose content changed in part hands its `Drawing` node
`draw_gpu(Some(drawing.redrawn(region)))` rather than a new `Drawing`: only that region, in the
drawing's own coordinates, is damaged (the plugin host does this with the
rectangles each plugin frame reports it changed). beui's own tests, and every test that drives a plugin through
`block-ui-test` (which turns on `beui::verify_paint`), paint every frame again
from scratch and fail when the retained painting differs from it or changed
outside the damage. With `BEUI_OVER_REPAINT=1` in the environment (or
`beui::detect_over_repaint(true)` on the thread that paints) they also report
every frame whose damage is more than four times the area of the shapes that
changed, to stderr and to `beui::take_over_repaints`.

What a node recorded is an immutable display list (`display.rs`) holding its own
shapes and its children's lists, and a frame hands the renderer the lists of its
roots rather than a flat list of shapes; `FrameOutput::shapes` flattens them on
demand, for tests and software rasterising. `Renderer` encodes a list once and
keeps its instances in a GPU buffer, only walks the lists that reach the damage,
and draws a content space through a translation and clip of its own, so a
scroll or a pan encodes nothing again. A scroll whose content only moved is not
damaged whole either: the frame reports it as `FrameOutput::moved`, and a host
that keeps its last frame, as `beui::run` does, copies that region by the
scroll with `Renderer::shift` and repaints only what the copy cannot supply -
the rows it exposes, and whatever else changed or does not move with the rows.
`FrameOutput::repaint` covers the moved region for hosts that do not copy.

Pointer input only visits a node when the pointer lies within the rects of it
and everything `children` returns under it, or when it leads to a node that
holds pointer state. A node that keeps state between events - hovered, pressed,
dragging - returns true from `engaged` until it lets go, so it still hears the
pointer leave, release or move away. Keyboard input goes to the focused node and
its ancestors rather than through this walk.

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
./scripts/buck run //:check
./scripts/verify
```

`./scripts/buck run //:check` is the fast complete-workspace compile check. `./scripts/verify`
is the full check, and CI runs it on a pull request and pushes whatever it changes to
the pull request's branch; it runs the workspace tests, lints, formatting, project structure checks, snapshot updates, and the formatter for
`view!` bodies that rustfmt cannot handle. Use a package-scoped Cargo command
only as a narrow diagnostic after one of the supported scripts has exposed a
failure. Run `./scripts/buck run //crates/block-app:smoke` as well when a change can affect native
startup or runtime integration.
