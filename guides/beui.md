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
interaction, layers and overlays). Inside block-app it is a block type, beui demo, whose editor (`crates/editors/beui-gallery`) shows the same catalog and stores nothing. Every sample on a page shows the code it was written with: a
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
attribute is what makes the function usable as a tag, gives it `@test_id`,
`@node_ref` and `@sizing`, makes `ComponentRef::fill`, `component_accessibility`
and `component_placed` available inside it (they panic anywhere else), and
gives it a scope of its own, so the contexts it provides reach only what it
builds rather than its siblings. `./scripts/verify`'s fix-rust-source
rejects a function that writes a `view!` without it (see below), since such a
function would build into its caller's scope.

Nodes and effects live and die together whether or not a function is a
component: a node belongs to the reactive scope it was created in, and
disposing that scope removes the nodes it still owns along with its effects
(see "Effects, entries, and ownership" in guides/reactive.md). A subtree that
must outlive the code that placed it, such as one shown through a `Portal`, is
built in a scope that lives as long as it should.

The child types of the node parents - `ListChild`, `GridCell`, `Layer`,
`CanvasItem` and `TextItem` - are all `Placed<P>` for the parent's node type
`P`, a node and the `P::Data` the parent keeps beside it (a `Sizing`, a span, or
nothing), so each is a distinct type and a child built for one parent does not
fit another. A new parent implements `ParentKind` for its node type, and
`BareChild` too when a bare `NodeId` should go in it unchanged.

A child needs no node at all. A component that returns a value rather than a
node still runs in a scope of its own, which lives as long as the scope that
built it, exactly as a node's does: a value built inside a `show` branch or a
`for_each` row goes with that branch or row, whatever happens to the value.
That is how an item made of data rather than nodes — a label, a key, a
callback — can still be a component, with its own scope, context, memos and
cleanups, and still be written as a tag. Such a component has nothing for `ComponentRef::fill`, `component_accessibility`,
or `component_placed` to watch, so calling any of them in its body
panics when it is built, and `@test_id` and `@node_ref` on its tag do not compile,
because they only take a component whose output implements `BuildsNode`.
`unstyled::MenuItem` is one: a menu item is a label, an icon, a detail, a
`checked` state (`Some` makes it a check box item, or a radio item with
`radio`, ticked when true; an `action`'s own tick by default), a
disabled and a danger flag, a separator above it, an `on_click` and its
own submenu or page items, so a menu is written as tags and each row follows
the signals its tag was given, and `unstyled::ChoiceOption` is the same for the
options of a tab bar, a listbox, a radio group and a select. Declare the type
with `value_child_type!`, which makes it a `ChildValue` and says how a run of it
is kept, so a `show` or a `for_each` can build one; a generic type names its
parameters first, as in `value_child_type!(<K> DockNode<K>)`.

Functions that build no part of a view are ordinary functions. Deriving a
colour from theme tokens and interaction state, mapping a value to a label —
write those as plain functions, as
`beui-components-styled`'s `checkbox.rs` does with `box_fill`.

### A component usually ends with one `view!` and nothing after it

Prefer making the last line of a component a single `view! {}` producing the
node it returns:

```rust
#[component]
fn LabeledValue(label: Prop<String>, value: Prop<String>) -> NodeId {
    let text = create_memo(move || value.get());
    view! {
        <List spacing=4.0>
            <Text>{label}</Text>
            <Text>{text}</Text>
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
in Rust control flow around it. `Show` takes a condition, builds its children each
time it becomes true and disposes of them when it turns false, as SolidJS's does.
Both it and `ShowKeepAlive` accept sibling children and components returning
`Children<_>` without a wrapper. `ShowKeepAlive` builds its children once and,
while hidden, takes them out of the tree but keeps their identities and pauses
their effects until shown again. A memo is worked out when it is read, so what
reads one from outside is never stale. Showing it again runs each effect whose
sources changed while it was hidden once, with their current values, even when
a memo it reads came back to what it was, so a controlled prop such as
`focused` that went false and true again takes the focus back.
Use it for a tab panel whose scroll position and state should survive switching away.
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

A function containing `view!` must be `#[component]`, whatever type it returns.
The source check exempts test code, framework trait entrypoints, and views
inside document/HTML rendering calls or factories storing deferred render
callbacks. Extract other helpers into
components and call them through `view!`, including helpers returning child
runs rather than a single node.

The exception is a component whose root is a base node it creates directly —
the base layer itself, where `List` calls `create_list` and binds setters with
effects. Above the base layer, one `view!` is the shape.

Render-slot closures can write markup directly: `<ForEach keys={keys}>|key: Key|
<Row key /></ForEach>`. Do not wrap that markup in `{ view! { ... } }`; the
formatter removes a redundant macro when the block has no setup statements.

### Call components from `view!`, never their builders

`#[component] fn Button` also generates a hidden `__Button_Builder` and a
`Button()` constructor returning it. That is the machinery `view!` writes into; it is not
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

### Components in traits and impls

A trait or impl block marked `#[component]` turns the methods in it that are
also marked `#[component]` into components, used as tags through the type:
`<A::View editor />` for a type parameter `A: BeuiApp`, `<Shelf::Row />` for a
method of `impl Shelf`. Without the mark on the block a method's `#[component]`
fails to compile, and fix-rust-source names the block.

```rust
#[component]
pub trait BeuiApp: 'static {
    #[component]
    fn View(editor: Editor) -> Root;
    #[component]
    fn Preview(editor: Editor) -> NodeId { view! { <Frame /> } }
}

#[component]
impl BeuiApp for CounterApp {
    #[component]
    fn View(editor: Editor) -> Root { view! { ... } }
}
```

The trait owns the props: `#[prop(...)]` is written on its declaration, and an
impl writes the same signature without them. Each method becomes a hidden
`__View` that holds the body, whichever side writes it, and a provided
`View()` constructor that returns a builder generated next to the trait;
an inherent impl gets the same, with the builder next to the impl. A body
runs as a component named after the type that wrote it (`CounterApp::View`,
or `BeuiApp::Preview` for a default), so the inspector tells each
implementation apart. A component method takes no `self`.

### Use fine-grained reactivity

Bind a signal or memo to a prop; do not read it while building and do not
rebuild a subtree to show a new value. A `Prop<T>` takes a plain `T`, a
`Signal<T>`, a `ReadSignal<T>`, or a `Memo<T>`, and the reactive forms install an effect that
writes just that one property when the value changes. A prop holds a signal or
memo it is given as it is; only a closure (`Prop::dynamic`) is boxed, so pass a
memo rather than `move || memo.get()`.

A `Prop<T>` is `Copy` whenever `T` is, like the signal behind it, so a
component reads one in as many places as it needs. A `Prop<String>` or other
non-`Copy` prop is cloned into each closure that keeps it:

```rust
let fill = create_memo(move || paint(value.get()));
view! {
    <Frame fill>
        <Text>{value}</Text>
    </Frame>
}
```

A tag's children are built in a `move` closure, so a child `{value}` moves
`value`. Inside a render closure that runs more than once (`Show`, `ForEach`),
clone it in a block head,
`<Show condition>|| { let value = value.clone(); view! { <Text>{value}</Text> } }</Show>`,
and work out anything a later prop still needs from it, such as a `format!`,
above the `view!`.

A callback prop (`Callback<V, R>` or `ClickCallback`) takes either a closure or
a callback the component already has, so a wrapper passes its own callback
straight on rather than wrapping it in a closure that calls it. A `Callback<T>`
also takes a `Signal<T>`, which it sets: `on_hover_change={hovered}`.

State a control both shows and changes (`checked`, `value`, `open`,
`selected`) is one `Binding<T>` prop rather than a value and an `on_change`:
pass a signal (`<Slider value={volume} />`), leave it out for the control to
own, or pass `Binding::new(read, write)` for derived state
(guides/reactive.md, "Bindings").

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
    <ForEach keys={items.keys()}>|id: Uuid| {
        let item = items.get(&id);
        view! { <ItemRow item /> }
    }</ForEach>
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
`beui::styled`, `beui::unstyled`, `beui::dock`, ...) and assembles them,
so code outside beui names only `beui`. Plain values (`beui-types`) sit at its root
(`beui::Sides`, `beui::Align`), never under `beui::reactive`. What reaches the
native document directly (`Document`, `with_document`, `try_with_document`,
`enter`, `with_reactive_scope`) lives in `beui::native`, so code that needs it
is visibly native; prefer a `beui::reactive` API (`TreeNode`, `set_focus`,
`use_preferences`, ...) where one exists. The rest depend on each other in one
direction:

| Crate | Owns |
| --- | --- |
| `ui-types` | the vocabulary beui shares with the plugin protocol, which depends on nothing but serde and `geometry`: `Key` (with its names for drive, `Key::from_name` and `Key::name`, its evdev code, `Key::evdev`, and what every platform names keys with: `Key::from_typed`, the one rule for a character key, `Key::from_value` for a W3C key value, and `Key::evdev_from_code` and `Key::from_evdev` for a physical place), `Modifiers`, `KeyChord`, `PointerButton`, the touch, pen and back phases, `ImeEvent`, `CursorIcon`, `Pasted` and `PickedFile`; `Input`, the raw input every platform, drive's gestures, the on-screen keyboard and the plugin protocol produce; `WindowRequests` and `ImeArea`, what an app asks of its window; the window vocabulary of window-hosting layers (`WindowId`, `WindowPlacement`, `HostPlacement`, `WindowEdge`, `WindowRole`, `WindowRequest`, `WindowReport`, which `beui_core::window` re-exports); the preferences (`ThemeKind`, `Motion`, `Preferences`, `PreferenceLayer`); and the display and pointer settings `beui-platform-drm` applies and block settings store (`DisplayMode`, `Rotation`, `InputDevice`, `PointerSettings`) |
| `beui-types` | the plain values components share whichever core draws them, with no nodes in them: colours (`Color32`, `Hsva`, `Oklch`, and `Color`, `ColorVar` and `color_var!`, which both cores resolve), `Image` and `Thumbhash`, the layout values (`Direction`, `Align`, `Justify`, `ItemSize`, `Sizing`, `Sides`, `Extent`, `Track`, `ScrollPosition`, `Corners`, `Outline`, `CanvasView`, and `Measured`, what `measure` answers), the text values (`FontId`, `TextAlign`, `Overflow`, `SpanStyle`, `TextMark`, `TextGlyph`, `TextCaret`), the layer values (`Placement`, `LayerPolicy`, `LayerLevel`, `Dismiss`), raw pointer and wheel input (`PointerId`, `PointerInput`, `PointerStage`, `WheelInput`), the declarative hit areas (`Area`, `HitRegion`, `Claim`) and the gestures (`PointerPress`, `ScrollGesture`, `DragGesture`, `ZoomGesture`), `KeyPhase` and `SystemBars`, with `geometry`, `ui-types`' input values and lucide's `icons` re-exported. beui-core re-exports each where it used to live; unstyled and styled reach them as `beui_view::types` and apps as `beui::`, and values that hold a node (`NodeId`, `OverlayAnchor`, `KeyAt`, `KeyEvent`, `ImeCursor`) through `beui::reactive` |
| `beui-tree` | what makes a tree of components, whatever it is made of: `#[component]` and `view!`'s runtime, `Prop`, child slots (`Children`, `Run`, `SlotChild`, `SlotHost`) and `Show`, `ShowKeepAlive`, `Dynamic`, `Keyed` and `ForEach` |
| `beui-core` | `Document` and `Context`, the retained nodes and their layout, input and its dispatch, accessibility, paint output and damage, the font and image interfaces, the icon codepoints, the `App` contract, and the `Runner` every platform drives it through |
| `beui-font-freetype` | `FreetypeFonts`: FreeType and HarfBuzz shaping and rasterizing, and the fonts beui compiles in |
| `beui-font-browser` | `BrowserFonts`: text measured with the browser's own fonts through a canvas; no fonts in the module, and nothing uses it yet |
| `beui-view` | the API both cores share: `beui::reactive`, `beui-tree` re-exported with the rest of the framework (actions, timers, `NodeRef`, `measure`, and the node functions and `Inspect` through which components reach the tree); `beui::primitives`, the base components (`Frame`, `List`, `Text`, ...), each declared once in `src/components/` with a body per core in `src/native/` and `src/html/`; `CORE`; and under HTML, `running` (`beui::html`) |
| `beui-core-html` | the HTML core (plans/html_core.md), which beui-view builds on under `cfg(beui_html)`: an arena from `NodeId` to a `beui-html` element, what makes a `NodeId` a child there (`tree`), values as inline CSS (`css`), the frame loop, clock and timers (`frame`), measurement, the containers' behaviour (`scroll`, `rows`, `canvas`, `overlay`, `portal`), text and its geometry (`text`), browser focus, key listeners, the hidden IME text agent, page preferences, viewport and font revision, `run`, `hydrate` and `render_to_string`, and `automate`, which answers `drive` in a browser (see [Cores](#cores)) |
| `beui-web-input` | what both web cores read from the browser: keys (`key`, `keystroke`, `prevents_default`), wheel deltas and pinches, `ImeMirror`, and the hidden text agent |
| `beui-components-unstyled` | `beui::unstyled` and `beui::datetime` |
| `beui-components-styled` | `beui::styled`: themes, styled controls, the styled `Frame`, `Text`, `Span` and `Icon`, and every primitive with no styled version re-exported unchanged |
| `beui-inspector` | the inspector, the simulated screen reader and the simulated mouse and keyboard |
| `beui-dock` | `beui::dock`: the dock, as `dock::unstyled` (the tree, the dragging, the keyboard and the layout state, painting nothing) and `dock::styled` (`Docking` painted with theme tokens) |
| `beui-keyboard` | `beui::keyboard`: the on-screen keyboard, as `keyboard::unstyled` (layouts, tables and the gesture engine) and `keyboard::styled`; the facade installs it on every document |
| `beui-renderer-wgpu` | the wgpu renderer, its shaders and filters, and presenting to a surface |
| `beui-platform-winit`, `beui-platform-android`, `beui-platform-web` | each platform's `Platform` and `PlatformServices`: its event loop, window or view, input (the web's through `beui-web-input`), IME, clipboard, file picker and accessibility adapter (winit's is its own `accessibility.rs` over `accesskit_unix`, `accesskit_windows` and `accesskit_macos`, since `accesskit_winit` has no release for winit 0.31) |
| `beui-platform-drm` | Linux's displays and input devices through DRM/KMS and libinput: a `Platform`, `PlatformServices` and `Renderer` that shows one window per display (see Displays under beui-platform-drm below), each display keeping a retained frame of its window, drawn when the display flips (the app's frames are paced to the flips too, by `pacing.rs`: at most one per flip, none while nothing changes), handing the flip a fence (or, without atomic fences, waiting for it in the event loop) instead of blocking on the GPU; callers run it with `beui::run_on` |
| `beui-layer-space` | the VR simulator `Layer` that `BEUI_LAYERS=space` stacks over a native or headless platform: the app's windows as panels in a 3D scene ("Space: the VR simulator layer" below) |
| `beui-layer-xr` | space on an OpenXR headset (`BEUI_LAYERS=xr`): the runtime, the Vulkan device it picks, its session, swapchains and frame pacing, and `XrSpace`, the `Layer` that drives `Space` from them ("Headsets" in the space section below) |
| `beui-layer-drive` | `beui::drive`: drive as a `Layer` (`Drive`, one layer or a tap under each of several layers sharing a session), its command `Inbox` and `Reply`, its socket protocol and the gestures it and the editor tests share (guides/running/drive.md) |
| `beui-platform-headless` | `beui::Headless`, the platform that draws offscreen with no window or display server; `beui::Simulation`, the headless window and its stand-in services; and `beui::testing`, the harness that steps an app frame by frame under a drive layer |
| `beui-platform-plugin` | the `Platform`, `PlatformServices` and `Renderer` for a block editor plugin module: `launch` starts the module's one app and registers its runner with the plugin framework as the module's `Regions` handler, whose batched calls then drive its frames; every region the host shows is a window of that app, which the app opens from the `Views` handle the platform provides through `Setup` (guides/adding_a_plugin_editor.md); it depends on the plugin framework, so the facade does not offer it, and it reads no `BEUI_*` variable |

Core cannot see the crates above it, so the few places it used to reach up are
hooks the higher crates fill in:

- The inspector is a `Tools` object on the document. `beui::reactive::build`
  installs it (`beui::install_inspector`), and `Document::show` hands the frame
  to it; a document built by `beui_view::reactive::build` has none.
- The simulated mouse is an `InputSimulation` the inspector installs on the
  `Context`.
- Text is shaped by the `FontBackend` a `Context` is made with. A backend
  only shapes one line (`FontBackend::shape`, text with no `\n`); core breaks
  and aligns lines once, in `RichLayout`, which `Fonts::layout` (and so
  `Painter::layout`) uses for wrapped, aligned or multi-line text.
  `beui::context()` makes one over `FreetypeFonts` and only the fonts beui
  bundles, which is what tests use; `beui::system_context()`, which the
  runners use, adds the system's own fonts for whatever the bundled ones do
  not cover. The fonts live in a `FontLibrary` that contexts can share and
  that can be given more fonts later, which lays their text out again.
- The document's theme store lives in a typed slot on the document
  (`Document::extension`, which components reach as `tree_local`). Both cores
  make a tree-local value in the document's own scope, so it outlives the
  component that first asked for it.
- A gpu `Drawing` holds whatever its renderer draws; `beui::drawing` makes one
  for the wgpu renderer. It is native only.
- Each platform is a `beui_core::runner::Platform`, a trait object the facade
  picks (`beui::native_platform`, `beui::web_platform`) and runs with a `Launch`
  (`RunOptions`, `Context`, the `App`). The platform owns the event loop and
  turns native input into `Event`s; everything one frame does is
  `beui_core::runner::Runner::frame`, shared by all of them: it runs the
  app, publishes accessibility, and tells the platform's `PlatformServices` (another
  trait object) what the app asked of the window. Everything the window
  shows for the app is one `beui_core::window::WindowState`: its `requests`
  (`ui_types::WindowRequests`: the cursor, pointer lock, back handling, wanting
  the keyboard, intercepted keys, the paste kinds the focus takes, the IME
  area and keyboard toggles, the same type a plugin sends its host), `typed`
  and `appearance`, which the
  `Context` fills during a frame and the main window's `WindowOutput::window` carries. While
  `Context::touch_emulation` is on its cursor is `CursorIcon::Touch`
  whatever the app asked: a finger-sized circle, defined once in
  `beui_core::touch_cursor` as an image at any scale (winit's custom cursor,
  the DRM sprite), an SVG (the web's CSS `url(...)` cursor) and a painter
  call (the mouse simulation's cursor). Every other icon is drawn, for hosts that
  draw their own cursor (the mouse simulation, space), by
  `beui_core::drawn_cursor::paint`: an icon-font glyph with its hotspot on the
  point, or an I-beam for text. A plugin region passes it to its host
  unchanged (the protocol's `CursorIcon` is beui's, from `ui-types`), and a platform with no custom cursors
  falls back to `CursorIcon::Touch.name()`, a crosshair. The runner hands it to `Platform::show_window` with the state it
  showed before, only when they differ, so a platform compares just the
  fields it uses; a new field is one more field there and in the platforms
  that act on it. Typing moves the platform's own copy of the field (the
  web's hidden textarea, Android's editable) even when the app turns the
  text away or a composition ends with the field unchanged, so `typed`
  counts the frames that delivered IME or text input, and the change is
  when the web and Android platforms bring that copy back to the app's
  text, comparing against what they last sent. The IME
  area it hands over is in the window's own units (winit's logical pixels,
  CSS pixels, a plugin region's points: physical pixels over the platform's
  scale, so points times the zoom), and `Runner::window` is the state last
  shown. What happens once rather than holds (a copy, a paste, a file pick,
  `set_fullscreen`, closing) has a call or field of its own.
- What the window did goes the other way, through `Runner::reports()`, a
  `WindowReports` handle a platform may clone and keep (the web platform
  hands one to its `fullscreenchange` listener): `resized` with its size in
  physical pixels, `scaled` with its pixels per point (winit's scale factor,
  the browser's device pixel ratio, Android's density, a plugin region's
  scale factor, the DRM displays' scale), and `fullscreen`. A window drawn at
  a raster other than that scale hears it as an `Incoming::Window` of
  `WindowReport::Rastered` instead (a DRM display's window, a plugin region's
  window at the region's raster). The runner
  applies them at the start of the next frame, resizing the renderers and
  setting `Context::set_pixels_per_point` before the app lays out, so no
  platform resizes a renderer or passes a scale itself. A report made during
  a frame (a simulated window answering) makes that frame ask for the next.
  Fullscreen goes both ways: `Context::set_fullscreen` asks the window to
  enter or leave it, the runner passing on each request that differs from
  what the window last reported, and the window reports what it actually
  did, whether the app asked or not, which the app reads as
  `Context::fullscreen` and a component as `use_fullscreen()`. The DRM
  platform is always fullscreen; Android and plugins say nothing.
  The clipboard is the platform's: `Context::copy_text` writes it, and
  `Context::copy_image` (`reactive::copy_image`) writes a PNG in its place
  (`Platform::copy_image`: winit through arboard, the web as a `ClipboardItem`,
  a plugin through the shell interface's `copy-image`, the headless
  `Simulation`, which pastes it back as `Pasted::Image`; Android and DRM drop
  it). `Context::watch_copies` hears every text copy (which is how be-wayland hands the
  app's copies to Wayland programs), and a paste is the only way to read it.
  The focus says what it takes: an `Interactive`'s `pastes` lists the
  `PasteKind`s (text, image) in the order it prefers them, and one with only
  `on_text` takes text. `request_paste` (a Paste menu) and Ctrl+V, which the
  document turns into a paste when nothing on the key's way took it or the
  focus forwards it, ask `Platform::paste` once a frame for the first of those kinds the
  clipboard holds, and its answer goes through the runner's intake as `Input::Paste`, after
  any input pushed before it, and arrives at the focus as `Event::Paste`
  (`Pasted::Text`, or `Pasted::Image` as a PNG `PickedFile`), handed to
  `on_paste`, or a text paste to `on_text` without one. A platform that
  delivers Ctrl+V's paste itself (`Platform::delivers_paste_shortcut`: the
  browser, from its paste event's data and the window's `paste_kinds`, and a
  plugin's host) is not asked again for the shortcut. The winit
  platform reads images too; Android and DRM read text only.
  An `Interactive` with `clipboard_keys` (a terminal) takes Ctrl+C, Ctrl+V,
  Ctrl+X and Ctrl+A as plain keys while it or a node inside it has the focus
  (`Document::clipboard_keys`, sent as `WindowRequests::clipboard_keys`, which a
  plugin's region passes on to its own catcher). Natively that changes nothing,
  since those arrive as keys anyway; the web platform, which decides in `keydown`
  before the app has seen the key, keeps the last flag it was shown and stops
  the browser's copy, paste and cut for them, and its inspector for Ctrl+Shift+C,
  but leaves Ctrl+Shift+V to make the browser's `paste` event, so a node that
  lets that key go gets its paste as everywhere else. beui's inspector leaves
  Ctrl+Shift+C alone too. A copy the node makes on a key, through `copy_text`,
  still reaches the web clipboard, since it is written within the key's user
  gesture. A node that takes these keys and leaves Ctrl+V unanswered gets no
  paste on the web.
  `PlatformServices`' methods all have defaults (no clipboard, no file picker), so a
  platform implements only what it has.
  A new platform implements those two traits and hands its window or element
  to the renderer loader the facade gives it.
- A platform only translates: it hands `Runner::push` an `Incoming`, which is
  a `ui_types::Input` with its positions and deltas in physical pixels
  (`Incoming::from`), in the window's logical pixels (`Incoming::logical`) or
  in points (`Incoming::points`), an `Incoming::FileDrag` (files hovered over
  the window, the hover cancelled, or a `DroppedFile` dropped: its name and
  its host path or its bytes), or `Incoming::Time`, below. `FileDrag` and
  `DroppedFile` live in beui-core rather than ui-types because they never
  cross the plugin protocol: a plugin's host turns them into its own
  `FileDrop` messages. An `Input` is
  what any platform reports: pointer moves and buttons, a `Keystroke` (the
  key a shortcut matches, its text and its physical place as an evdev code,
  with no modifiers; guides/beui_keyboard.md), a `ScrollDelta` in pixels,
  lines (`SCROLL_LINE` points) or pages (`SCROLL_PAGE`), touches, pen samples,
  text, IME edits, a paste, the modifiers held and the window's
  focus. The runner's intake is the one place it becomes `Event`s, and drive's
  gestures, the on-screen keyboard and typed pastes push `Input` there too, so
  they are normalized as a window's input is. A trackpad's two-finger
  scroll is a scroll and its pinch an `Event::Zoom` on every platform: winit's
  `PinchGesture`, and in the browser a Ctrl-wheel (how Chrome and Firefox
  report a pinch, which the web platform turns into `Input::Zoom` at
  `PINCH_SPEED` per pixel) or Safari's `gesturechange`, whose scale over the last one's
  is the factor; the web platform ignores a gesture while fingers are down,
  since those touches pinch already, and prevents the default of all of them
  so the browser never zooms the page. Fingers touching a trackpad push
  `Input::ScrollStarted` and lifting them `Input::ScrollEnded`, which a plugin's
  host forwards unchanged. Between the two,
  `ScrollGesture::held` is set and a `Scroll` follows the fingers as a touch drag
  does, stretching past an end; the lift springs it back, or flings it at the
  speed of its last samples and bands it if that carries it past an end. A
  start alone, fingers resting without moving (macOS's `MayBegin`), goes to the
  press claimant under the pointer, so it catches a `Scroll` or sheet that is
  flinging or bouncing and holds it until the fingers move or lift. A wheel
  without a start (a mouse) clamps at the ends. winit pushes them on a
  `MouseWheel` phase of `Started` and `Ended`, and on macOS drops the momentum events the
  system sends after a lift (from a `Started` within 100 ms of it to their
  `Ended`, in `beui-platform-winit/src/wheel.rs`): beui's fling replaces them,
  and they would scroll clamped at an end instead of banding. The browser
  reports neither, so a web wheel scroll never stretches or flings. Every pointer sample is an event of its
  own: the web platform pushes each of a `pointermove`'s
  `getCoalescedEvents()` rather than only the last, and `on_drag` runs once
  per sample, so a stroke keeps every point the device reported between two
  frames. After a pressed move it pushes `Input::Predicted` with the last of
  `getPredictedEvents()`, where the pointer is expected to be by the next
  frame; the runner attaches it to the move just before it as
  `PointerEvent::predicted`, and the drag it makes carries it as
  `PointerPress::predicted`. A prediction is only good until the next sample
  replaces it, so a component draws it as a transient tail (the canvas's pen
  tool does) and never keeps it; the other platforms report none, and a
  plugin's host forwards it too. Each sample also has
  the time it happened: a platform pushes `Incoming::Time(Instant)` just after
  the event it stamps, and the runner gives it to that event's
  `PointerEvent::time`, the session it moves and the `PointerPress::time` its
  catchers hear (touch fling velocity is measured with it too). The web
  platform reads `timeStamp` from each coalesced `PointerEvent`, Android sends
  every historical sample of a `MotionEvent` with its own event time, the DRM
  platform reads libinput's, and winit, which reports none, stamps an event
  when it arrives. An event pushed without one, as tests and drive push
  them, happens at the frame clock's `now`, so a test steps time by stepping
  frames. An `Interactive` with `every_sample` (a `Prop<bool>`, set while it
  needs them, as the canvas's pen does for a stroke) asks for every sample
  where a host would otherwise coalesce them: `Document::wants_every_sample`
  is true while one is in the document, and a plugin reports it to its host
  (`FrameReport::every_sample`). Within beui itself every sample always reaches
  `on_drag`. `Incoming::logical` marks
  input whose pixels are the window's logical ones (physical pixels
  over the scale it last reported: the browser's CSS pixels), so the runner
  converts them with the same scale the frame lays out at and no platform
  reads its scale at event time. Text is passed as the platform gives it;
  the runner drops control characters. It may send `Input::Modifiers` on every event; the runner
  passes it on only when it changes. At the start of each frame the runner
  turns its input into points. It stamps keys, pointer buttons and
  intercepted keys with the modifiers held, and drops a held modifier's
  repeats and the text of a shortcut. It turns the primary mouse button into
  a finger while `Context::touch_emulation` is on, holds `PointerGone` back
  while a button is held (sending it after the release, unless a move
  inside the pointer's window's reported size came first: the pointer came back), and lets go of the modifiers when the keyboard is
  lost (`Input::Focus(false)` means only the keyboard). `Input::PointersCancelled`
  says every press, touch and pen session is over and its release will not
  come: the runner lets go of its buttons and emulated touch, and the
  document cancels the touches, ends the pen and the mouse
  (`Event::PointersCancelled`) and cancels a pointer grab, which stops autoscrolling. Only the platforms
  whose platform can lose a release send it: winit and the web when their
  window loses focus (alt-tab mid-press, a broken capture on Windows, a grab
  by another X11 client), and winit on Wayland also when the pointer leaves
  with a button held, since a compositor grab sends `wl_pointer.leave` and no
  release (`ActiveEventLoopExtWayland::is_wayland`). Android reports
  `ACTION_CANCEL`, the web `pointercancel` for touches and pens, DRM reads
  libinput, and a plugin's host ends each pointer through `Replay`.
- `Platform::pick_file` gets a `FilePickAnswer`, which can be sent from any
  thread and wakes the runner; the pick reaches the app at the next frame.
  Accessibility trees are published only between
  `Runner::accessibility_requested` and `accessibility_deactivated`, and
  `Context::accessibility_requested` says whether they are, so a host asks what it
  embeds to describe itself only while something is listening. A document
  describes again only the nodes a change or a move reached, and passes over a
  subtree nothing in it changed (`FrameWork::walked_nodes` counts what it
  visited). Each node is placed relative to its accessible parent in the
  coordinates of the space it was laid out in, and each space a node lays
  children into (`Document::enter_space`, a scroll's content, a canvas's
  spaces) is a `GenericContainer` whose transform is the space's translation
  and scale, so a scroll or pan changes only that node.
  `Frame::wait` is how long the platform may sleep before the next frame
  (`None` until something wakes it), and `Frame::next` the same once it
  knows whether the frame it presented asks to be presented again.
  Closing is the runner's: when the app calls `close_window` (or a driver's
  close is accepted) the runner exits after that frame and `frame` returns
  `None` with `Runner::exited` set, which every platform answers by stopping
  its loop (Android also finishes its activity, the web platform closes the
  tab where the browser lets a script, and a plugin region's `Frame` says
  `stopped` from then on, `Stopped::Closed` or `Stopped::Failed`, which the
  host shows in the region's place). A platform whose window is closed from
  outside calls `Runner::os_close`, which asks the app's `close_requested`
  and exits if it agrees; the web platform asks on `beforeunload`, and a
  refusal is the browser's leave-the-page prompt.
  A panic out of the app's `update` stops the runner rather than unwinding
  through the event loop: the app hears `exiting`, `frame` returns `None` from
  then on, `Runner::failure` says why, and the platform ends its loop and
  returns that as an error (`Runner::finish`), so whatever owns the screen is
  let go of in order. An app that can carry on after a panic in one part
  catches it there itself.
- Every platform draws through a `beui_core::renderer::Renderer` trait object
  and names no renderer crate: wgpu's `WindowSurface` (native windows) and
  `CanvasSurface` (a browser canvas) implement it, and the facade hands the
  runner a function that loads one (`Loaded`: the renderer, and the
  `FontBackend` its text is measured with if it needs its own, which
  `Runner::start` installs with `Context::replace_fonts`). The runner keeps it
  in a `Rendering`, which the platform attaches and detaches through
  `Runner::rendering`. A renderer is told where to draw with `attach` (a
  raw-window-handle `WindowHandle`) or is made with its element, and draws a
  frame in two steps, `prepare` and then `present`.

Inside the family, crates name each other directly (`beui_core::document::Document`),
and the component crates declare `extern crate beui_view as beui;` so the
`::beui::reactive` paths `#[component]` and `view!` expand to resolve there.

`beui-tree` knows nothing of `Document`, layout or painting, so the same
components and control flow can build any tree: a DOM, a 3D scene, a chat
message with buttons. Such a tree gives its child type `ChildValue` (where a
component's scope lives) and `SlotChild` (how a child is kept), and gives what
keeps children `SlotHost` (appending, and filling a slot a `Show` or `ForEach`
holds) and `IntoSlotHost`, so a component
can `children.mount(...)` into it. The macros need `::beui::reactive` to
resolve: `beui-tree` names itself `beui` for its own tests, and a crate built on
it does the same or re-exports `beui_tree::reactive` as a `reactive` module of
a crate it calls `beui`. Modifiers are ordinary functions: `@name=value`
expands to `name(built, value)` after the component is built. Paths such as
`@my_module::decorate=value` work too. Multiple modifiers run in source order,
including repeated names, and can change the output type. They also follow
expression and string children: `{child} @decorate=value`. A modifier without
`=` uses its path as its value, like prop punning. Ordinary props are set on the
builder before any modifier runs, regardless of their position among modifiers.
The GUI prelude (`beui::reactive::*`) supplies `sizing`, `test_id` and `node_ref`;
other trees supply their own functions. Qualify a modifier when a local variable
shadows its name. An untyped `Render`/`RenderFn` child or a `Child` prop is
`beui::reactive::Child`, a `NodeId`. `beui-tree`'s tests build a small DOM this
way.

## Cores

beui has two cores under the same components. The native core is
`beui-core`: retained nodes beui lays out, focuses, hit-tests and paints
itself (with wgpu), on every native platform and in block-app's web build. The
HTML core is `beui-core-html`: each primitive is a DOM element, and the browser
lays out, focuses, selects and paints. beui-view holds both behind one API
(`src/components/` declares each primitive once; `src/native/` and `src/html/`
are the bodies), and unstyled, styled and the code above them are the same
source on both.

The core is chosen at compile time by `--cfg beui_html`, which the buck
platforms `wasi_html` (the browser) and `linux_x86_64_html` (the host, for
server rendering and HTML paintings) set (guides/buck2.md); every other
platform builds the native core. Code reads which one it is built for from the
constant `beui::CORE` (`Core::Native` or `Core::Html`) and switches with
`if CORE == Core::Html`; `#[cfg(beui_html)]` belongs only in beui-view, beui and
beui-core-html. Such a switch is a deliberate choice for one component, never a
way around a primitive one core lacks. What only the native document can do is
in `beui::native` (`Document`, `with_document`, `Embed`, ...); the dock, the
inspector, the on-screen keyboard and plugin hosting are native only, and on
HTML a `Window` shows as its overlay. The HTML facade adds `beui::html`
(below).

The contract both keep:

- **Answers may come late.** Whatever layout decides (`measure`,
  `measure_text`, a scroll's position, an overlay's side) is a signal that is
  `None` until the first layout and may arrive a frame after the change that
  caused it. Native answers in the same frame and must keep doing so (virtual
  scrolling and scroll anchoring stay frame-perfect); callers are written for
  the late answer.
- **The browser owns focus on HTML.** Focus is real DOM focus (`tabindex`,
  `:focus-visible`); `set_focus`, Tab order and `focused` bindings go through
  it, so code never assumes focus moved until it hears so.
- **Each core does the best its platform allows**: the browser's own
  selection, find in page and scrolling into view on HTML; native's own
  selection and frame-perfect layout.
- **No feature exists on only one core.** A new prop or hook gets a body in
  both; where one cannot do it yet, the HTML body is a visible stand-in (an
  outlined pink box, or a prop it ignores), listed in plans/html_core.md.

Measurement is `measure(node) -> ReadSignal<Option<Measured>>` (`rect` in
window space, `size` in the node's own units), `measure_text(node)` for a
`Text`'s `TextGeometry`, and `measure_line(text, font)` for text not in the
tree, all described under [Measurement](#measurement). Colours are `Color`s
that may reference declared variables, so both cores and a server-rendered page
agree on them ([Colours](#colours)).

What each primitive is on the HTML core (the bodies are in
`beui-view/src/html/`, with their larger parts in `beui-core-html`; see
[Develop a base component](#develop-a-base-component)):

| Primitive | On HTML |
| --- | --- |
| `Root` | `div.beui-root`, fixed to the viewport and clipping what overflows it, so the document never scrolls (only `Scroll`s do; the page needs no sizing styles of its own); it holds the colour definitions, and every overlay's element stays inside it, so they see the same variables |
| `Frame` | a one-cell CSS grid; `translate` and `rotate` are a transform, `opacity` CSS opacity, `fade_edges` a mask image |
| `List`, `Spacer` | flexbox and a flex child; `@sizing` becomes flex grow, shrink and basis |
| `Grid`, `Layers` | CSS grid; `Layers` stacks every child in one cell |
| `Text` and its spans | a `div` of `span`s and text nodes with `white-space: pre-wrap`; `marks` are CSS Custom Highlights, `carets` positioned boxes, `glyphs` `::before` content, `Break` an `::after` newline no browser copies; a `TextItem` with `at` is a stand-in |
| `Offset` | an `overflow: hidden` element moved through `scrollTop`/`scrollLeft`, never a transform |
| `VirtualList` | rows in flow between two spacer elements, measured by `ResizeObserver` |
| `Canvas` | an inner element translated to the view, `--beui-scale` scaling items |
| `Overlay` | a `<dialog>` (`showModal()` for one that blocks everything, else a manual popover) placed with CSS anchor positioning, a one-cell grid like `Frame` while open; `window` and `shown_in` are ignored |
| `Portal` | moves the shown subtree's element (`moveBefore` where the browser has it) |
| `Interactive` | an element with `tabindex` and listeners; `pastes`, `key_active`, `claims_touch` and the forwarding props do nothing yet |
| `Selectable` | the browser's own selection; the root is `user-select: none`, so nothing outside a `Selectable` (or a native text field) selects |
| `Drawing` | a `div` holding an inline `<svg>` the `Draw` callback paints, sized as natively (see below) |

`Drawing` is the escape hatch for what is not an arrangement of nodes, and
`Embed` (native only) the one for what is not beui's. On HTML a `Drawing` runs
its callback into an SVG `Painter` (`beui-core-html`'s `drawing.rs`) and sets
the result as its element's markup (`beui_html::Node::set_markup`), rebuilding
only that SVG when the callback, its measured size or a colour it resolved
changes. SVG rather than `<canvas>` because a server-rendered page shows it
before wasm loads. The callback is handed the drawing's measured size, or its
`size` before it is measured, so a drawing with neither (one that fills what
places it) draws nothing in a server-rendered page. The HTML `Painter` has the
methods with an SVG equivalent: rectangles (rounded per corner, filled or
stroked inside like native), `line`, `fill_path` and `stroke_path` (fill rule,
joins, caps, miter limit, dashes), `image` (a PNG data URL, cropped, rounded,
tinted, pixelated when not `smooth`), `text`, `transformed`, `rotated`,
`with_clip_rect`, `clip_rect`, `pixel_grid` and `resolve_color`. Colours a
`Painter` takes are `Color`s, so references become `var(--…)` and
`color-mix()` there and follow the scheme without a redraw. A gpu `Drawing`
(`beui_view::drawing`, `Painter::drawing`) is native only; the colour wheel
paints its faces as an image on the CPU instead (`color_wheel/cpu.rs`, used
without the `gpu` feature). Text editing is not an escape hatch:
`TextArea` and `TextInput` are the same unstyled code on both cores, built on
`Text`'s `marks`, `glyphs`, `carets` and `hang` and on `measure_text`, typing
through a hidden textarea on HTML.

An app on the HTML core starts with `beui::html::run(element_id, build)` in
the browser. To serve a page that shows before any script runs, render it on
the host with `beui::html::render_to_string(build)` (a program built for
`linux_x86_64_html`, run at build time by the `prerendered_page` rule in
buck/app/defs.bzl), put `beui::html::stylesheet()` in a `<style
id=STYLES_ID>` in its head, and call `beui::html::hydrate(element_id, build)`
with the same view in the browser: it claims the server's elements instead of
building new ones (beui-html's hydration, [Live DOM and
hydration](#live-dom-and-hydration)), and `run` and `hydrate` read the
browser's preferences before they build so the first frame matches the page.
`beui::html::choose_theme` sets a scheme the app picked over the browser's.
crates/beui-html-spike is the worked example: block-app's account picker
(crates/block-accounts-ui) prerendered by `:ssr` and hydrated by `:web`
(guides/running/web.md). crates/beui-demo-html runs beui-demo's pages on this
core.

To test on both cores:

- Behaviour that must match goes in a shared script in crates/beui-shared-tests:
  a screen and a `drive` script, run natively (once with late measurement) and
  in Chromium (guides/testing_a_gui.md, 7).
- What the HTML core draws is checked with browser paintings rendered from
  `render_to_string` on `linux_x86_64_html`: `./scripts/test
  //crates/beui-view:html-test` for primitives, `./scripts/test
  //crates/beui:html-test` for unstyled and styled components (tests in
  `crates/beui/src/html_paintings/tests/`), and the spike's `:html-test`. Build
  the page's `<style>` from `stylesheet()`: its `@font-face` rules name the
  bundled fonts at the page's root, which the browser rasterizer serves, so
  text and icons draw as they do natively. Paintings show layout, colour and
  text, not what needs the browser's layout (marks, carets).
- The spike's dark painting
  (`crates/beui-html-spike/snapshots/prerendered_accounts_page_dark.browser.paint`)
  and block-accounts-ui's
  `an_account_in_a_company_says_its_administrators_can_read_it.paint` show the
  same screen on the two cores; rasterize both and compare them after a change
  to the HTML core or styled. They differ only in text rendering: glyph
  advances and hinting, and the browser rounding a font's ascent and descent to
  the nearest pixel where FreeType rounds them outward, which moves a 20px
  title and some icons by a pixel.
- Native tests take the late path with `beui::testing::delay_measurement(true)`
  (`BEUI_DELAY_MEASUREMENT=1` for a driven program), so a component that only
  works when measurement is synchronous fails natively too.

## Component layers

Beui separates mechanism, behavior, and appearance. The dependency direction is
deliberate:

| Layer | Location and public path | Responsibility |
| --- | --- | --- |
| Base | declared in `crates/beui-view/src/components`, with a body per core in `src/native/` (over `beui-core`'s nodes in `crates/beui-core/src/base`) and `src/html/` (over `beui-core-html`); `beui::primitives`, which beui's own crates use and apps reach through `beui::styled` | Layout, painting, visibility, focus, pointer input, offset content, and text, implemented by each core (see [Cores](#cores)). |
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
  instead. The base layer is small on purpose — `Frame`, `List`, `Spacer`, `Layers`,
  `Grid`, `Text`, `Offset`, `VirtualList`, `Canvas`, `Drawing`, `Overlay`, `Interactive`,
  `Portal` and `Root` — and it stays small because most things are
  compositions of those. `unstyled::Picture` is one: a `Drawing` with a size, and
  `Window` (`beui_view::window`, re-exported as `beui::Window`) is another: an
  `Overlay` given a `window` spec. Back is a hook, `on_back`, not a node, and
  `Embed`, which only the native app composites, is in `beui::native`.
  `Selectable` is the one component each core builds its own way (the
  browser's selection on HTML) rather than a composition; text editing is not
  one of those: `TextArea` and `TextInput` are unstyled compositions of `Text`
  (`marks`, `glyphs`, `carets`, `hang`), `Interactive`, `measure_text` and
  `measure_line`, the same on every core.
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
is offered and keeping its shape, as an image does. A sized `Drawing` clips its
callback to the rectangle it gets, so a callback that draws in the units of its
`size` scales itself into that rectangle (`painter.transformed(offset, scale)`,
as the demo's Paths page does) rather than spilling past it.

Text is never one of those. A base `Text` takes concrete values and knows
nothing about the theme: a `font` (a `FontId`, 14 points proportional unless
given), a `color` and `underline`, and the props that shape the paragraph -
alignment, `overflow`, `line_height`, `line_padding`, `marks`, `glyphs` and
`carets`. `overflow` is an `Overflow`:

- `Wrap { at, max_lines }`, the default with `at: WrapAt::Words` and no
  `max_lines`, breaks lines to the width the text is given (`WrapAt` below).
  With `max_lines` it lays out no more than that many, and the last one ends
  in an ellipsis that stands for the rest.
- `Ellipsis` keeps a label to one line, cut to its width with an ellipsis. A
  newline or a `Break` still starts a new line, and each line is cut on its
  own.
- `Visible` lays each line out unbroken at its natural width and paints it
  whole, for text inside something that scrolls or clips for it: a text
  area with wrapping off, a terminal's screen rows, a map label.

There is no clip mode: text that has to be cut is `Visible` inside something
that clips. Its content is its children: string literals and
`Prop<String>`s in its own style, `Span`s - value children with no node, each
overriding its Text's `font` whole or any of its `size`, `family`, `bold` and
`italic`, and its `color`, `underline` or `strikethrough`, giving it a
`background` (a `SpanBackground`: a colour, a corner radius and an outset), and
marking a wrap opportunity with `break_after` - `Icon`s, `TextItem`s, whose child is laid
inline as if it were one character (U+FFFC in the text), and `Gap`s, room of a
given width that holds no bytes, so the indices on either side of one are the
same, and `Break`s, which end the visual line wrapped or not and likewise hold
no bytes: the index at a break starts the next line, the caret steps over it,
and a selection or copy across one carries no newline, so a soft-wrapped
terminal row drawn with `Overflow::Visible` copies as one line. A newline in a
span's text is a real line break that a copy keeps. A tab in a span's text is laid out as its own run that reaches the next
tab stop, every `TAB_COLUMNS` (4) spaces of its span's font from the start of
its line. An unset span prop takes its Text's. A Text whose only child is a
string or `Prop<String>` keeps it as its own content rather than as a span,
which costs no more than a node made with `Document::create_text`.

An `Icon` is a span that paints one glyph from the icon font and, like a
`Gap`, holds no bytes, so the caret steps over it, a selection never covers it
and a copy never carries it, and `<Text><Icon glyph=ICON_SAVE />"Save"</Text>`
reads and copies as "Save". `Document::text_icons` lists a text's icons. An
icon is its Text's font size times `scale` unless given a `size`, is centred
in its line rather than set on the baseline, and makes its line no taller than
text of the size it scales from would. It keeps `spacing_em` (a fraction of
its own size) from whatever sits beside it on its line, so the gap between an
icon and its label is layout rather than a space a selection could take, and
an icon alone keeps none; an icon whose glyph is empty takes no room at all.
Underline and strikethrough never cross one. Computed runs are a
`ForEach` of `Span`s over the caller's own tokens. The Text lays the runs out
and wraps them itself: `WrapAt::Words` breaks after whitespace, `-`, `/` or `\`,
or after a `break_after` span, and between letters when no such break fits;
`WrapAt::Anywhere` breaks between any two grapheme clusters, so each line holds
as many as fit (in a monospace font at a width of `cols` advances, exactly
`cols` characters; measure the advance as `cursor_pos(_, 1).x` of one
character, since a galley's `size()` is rounded up to a pixel). Each run is as
wide as its text's advance, so spans in a monospace font stay on its grid.
It lays text out with `line_padding` above and below each line, and aligns, truncates with an
ellipsis and spaces lines by `line_height` whatever its content. Offered an
unbounded width, wrapping text measures at its natural width, each paragraph
on one line; its `min_width` (see [Develop a base component](#develop-a-base-component)) is its longest
run that cannot be broken, a word and the space after it, so a row never
squeezes it into a column of letters. It caches each
span's shaping, so a span whose props change is the only one shaped again, and
line breaking resumes from the line that span is on; a colour or decoration
change, and any change to `marks`, `glyphs` or `carets`, only repaints. It paints each span's `background` behind the span's
range on every line the span is laid out on, so it follows wrapping, as one
shape where neighbouring spans have the same background (an inline code span
split by its styles). Over that it paints `marks` (selections, flat), then the
text, then `glyphs` (`TextGlyph`: a string in a font and colour, centred across
the laid-out box of a byte range on every line it is on and set on that line's
baseline, taking no room, the way a text area shows a whitespace marker over a
real space) and `carets` in front of the text, blinking ones included - byte
ranges over the joined content - and a `TextItem` given `at` is placed at that
index as if it were a caret instead of in the flow, which is how something
floats beside a position in the text. `measure_text(node)` answers where an index is and which index is
under a point from the node's last layout (see "Measurement" below), and its
`bounds()` is where its laid-out text reaches (its natural size, not the rect
it was given).
`unstyled::TextArea` is built from exactly this: a row per document line in a
`VirtualList`, each a `ForEach` of the row's spans, a gutter of `Text`s beside
it, and list markers, voids, embedded widgets and the caret's anchor as
`TextItem`s.

The galleys a `Drawing` paints come from `layout_text(text, font, layout)`,
which lays text out with the shown document's fonts and answers `None` until
the document has been shown once; the galleys it returns are cached, so asking
for the same word twice costs a hash lookup. A `Galley` is the native core's,
so unstyled and styled never name one: they measure through `measure_text`
and `measure_line` (see "Measurement" below). A `FontId` carries `bold` and `italic` alongside its size
and family; both are synthesised by FreeType rather than loaded as separate
faces.

Layout and raster are two scales, as pinch-zoom and Ctrl+/- are in a browser.
Text is laid out, and everything snaps to the pixel grid (`Document::pixel_grid`,
`Painter::pixel_grid`), at the layout scale: `Context::pixels_per_point`, the
platform's scale times the zoom factor, which a platform DPI change or Ctrl+/-
changes and which lays the document out again. The font pixel size
(`round(size * scale)`), line heights, baselines, advances and wrap points all
come from it, and galleys are cached by it. `PainterState::raster` is how much
finer than that a subtree is drawn (a hosted window's scale over the context's,
times each zoomed canvas's `Zoom::raster`), and only painting reads it: layout,
measurement and placement reuse ignore it, so changing it changes no size, wrap
or caret and lays nothing out, and a node painted under a new raster records
again. `Painter::galley` hands a galley to `Context::rastered` under a raster
other than 1, which asks the font backend (`FontBackend::raster`) for the same
layout with its glyphs at that many times the pixels: FreeType places each glyph
at its laid-out position times the raster (keeping a subpixel step) and
rasterises it unhinted from the outline at the laid-out size scaled by the
raster (`GlyphId::raster`, the factor's bits; `GlyphId::LAID_OUT` is 1), so the
outlines match the scaled advances. The result's `Galley::pixels_per_point` is
the layout scale times the raster. A backend without glyphs of its own (the
browser's) keeps the galley as it is.

`unstyled::Button` shows the split. It is a focusable `Interactive`,
and owns button semantics, disabled behavior, pointer and keyboard activation,
and accessibility: it takes the `label`, `glyph`, `role` or `Action` and builds
its own accessibility node from them. Its content closure receives a
`ButtonHandle` of reactive `hovered`, `active`, `focused` and `disabled` state
and the resolved `label`, `glyph`, `icon_only` and `tooltip`. `styled::Button` wraps it and
uses that handle to choose fills and paint a focus outline, so every visual
treatment sits on the same interaction behavior. A button that opens something
(`MenuButton`, `Popover`, `SplitButton`'s arrow, the date and colour pickers) is a
`PopupTrigger`: an `unstyled::Button` that toggles an `open` binding, marks itself
`expanded` and with the kind of popup it opens. Its `PopupTriggerHandle` carries
the `ButtonHandle` as `button` rather than copying its fields.

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
button), and neither a shrinking child nor a `Percent` one in a row gets less
than its own `Element::min_width` (wrapping text's longest word), `.align(..)` overrides the list's `align` for that child, and
`.gap(..)` replaces the list's `spacing` before it:
`@sizing={ItemSize::Percent(100.0).max(320.0)}`.

`Frame` combines optional sizing, an aspect ratio it centres
its box within, padding, fill and outline on one retained node,
each concern one prop. `width` and `height` take an `Extent`: an exact length
(`Extent::px`, or a plain `f32`), a share of the space offered when it is
bounded (`Extent::fraction`), and `.min(..)` and `.max(..)` bounds. A `max`
caps the room the frame takes, not its content: like CSS `max-width` on a
block, `Extent::at_most(320.0)` is as wide as the space it is offered, up to
320, however little its content needs (a dialog, a settings column). A box as
wide as its content up to a cap (a menu, a tooltip) is a list child sized
`@sizing={ItemSize::Intrinsic.max(320.0)}`, in a horizontal `List` if it has
no list around it; `ItemSize::Intrinsic.min(..)` gives it a floor. A memo of
`Option<f32>` hands an axis back to intrinsic measuring when it is `None`.
Nothing is hidden while it stays in the tree: what shows only some of the
time goes in a `Show`, or a `ShowKeepAlive` when it should keep its state
while it is gone, so it takes no room, no spacing, no focus and no place in
the accessibility tree. In a single-child slot like a `Frame`'s, put the
`Show` in a `Layers`, which fills the slot as the child would. `padding` takes
`Sides` (`Sides::all`, `symmetric`, `horizontal`, `vertical`, then `.left(..)`
and its siblings to override one side). `align` takes an `Align2` (a plain
`Align` sets both axes) and places a child smaller than the frame inside it
rather than stretching it over the frame, which is how a badge sits in a
corner. The primitive takes concrete values: `fill` a `Color32`, `radius`
`Corners`, and `outline` an `Option<Outline>`. The styled `Frame` apps use takes
design tokens instead (`styled::tokens`): `padding` a `Padding` of `Space`
steps (`Padding::all(Space::Md)`, `symmetric`, `horizontal`, `vertical`, then
`.left(..)` and its siblings; a plain `Space` pads every side), `width` and
`height` a `Length` (an `Extent` for fill, fraction and at-most, or a `Size`
for a fixed length), `fill` a `ColorRole` (a theme colour, or
`ColorRole::Custom` for one that comes from content, such as a user's tag
colour), `radius` a `Radius` (`Xs` to `Lg`, `Tab`, `Full`, or `Custom(f32)`
for a corner that scales with content, such as a card on a zoomed canvas)
with `corners` a `CornerSet` when only some corners round, and `border` an `Option<Border>`
(`Border::DEFAULT`, `Border::FOCUS`, `Border::new(role)` with `.width(..)` and
`.offset(..)` taking a `Stroke`: `Hairline`, `Focus`; `focus_border` shows one
only while the focus ring is visible). A `ColorRole::custom(..)` wraps a colour
or a signal of one.
`Space` steps are `None` 0, `Xs2` 2, `Xs` 4, `Sm` 6, `Md` 8, `Lg` 12, `Xl` 16,
`Xl2` 20 and `Xl3` 24; `Size` steps are `Track` 6, `Xs` 16, `Sm` 20, `Md` 24,
`Lg` 32, `Xl` 40, `Xl2` 48, `Xl3` 64, then the named `Row`, `Toolbar`,
`Field`, `TouchRow`, `Menu`, `Popover` and `Dialog`. Both resolve through
`use_density()`: under `Density::Comfortable` each `Space` from `Xs` up is one
step larger and a `Size::Row` is tall enough to tap. `Space::Custom` and
`Size::Custom` are for lengths that come from content (a measured indent, a
board cell, a canvas), never for a gap the scale has. `use_space` and
`use_size` resolve a token to the pixels an unstyled prop takes. A styled prop
never takes a raw number: `8.0` where a `Space`, `Padding`, `Size`, `Length`
or `Stroke` goes does not compile. The one way to write pixels into a styled
`Length` is `Extent::px`, so the source lint rejects `Extent::px` outside
beui's own crates; write a `Size` (`Size::Custom` for content). Styled `List`
and `Grid` take their `spacing`, `column_spacing` and `row_spacing` as `Space`,
and styled `Gap` its `width`; the base ones keep taking pixels. A base
`Spacer` is an empty box with no props, which fills the rest of a row given a
`Percent` `@sizing` or stands in for a child that is not there; a fixed gap
is the styled `Spacer`, whose `width` and `height` are `Space`s (a
`Space::Custom` for an indent computed from content).
A `FocusRing` takes the `radius` of what it rings and an `offset` `Stroke`,
and rounds its ring by the two together so the ring stays concentric.
A frame also carries the effects that need no node of their own, all of them
leaving the space it takes alone: `translate` lays its box and child out moved
by a vector (snapped to the pixel grid) and paints nothing while both are off
the screen, which is how a sheet slides in; `rotate` paints it turned clockwise
by that many radians about the middle of its box, clips inside it growing to the
upright bounds of what they clip so nothing turned is cut off; `opacity` paints
all of it, fill and border included, at that opacity (1 by default); and
`fade_edges` fades it out toward each edge over the widths it names. The opacity
and the edges are one mask (`Painter::faded`, `Group::Mask`): the renderer
multiplies every shape's coverage by it, so shapes inside it that overlap show
through one another, and a change damages the frame's rectangle. Pointer input
and the accessibility tree follow `translate` but see a rotated frame unturned,
so `rotate` is for pictures (a canvas's turned text), not for controls. The styled
`Frame` passes all four through. Frames paint no shadows: a floating surface
stands apart by its fill and border.
`Layers` lays every child over the same box, sized to the largest, later
children on top: content over a background, a badge over an icon. A child that
should not fill the box goes in a `Frame` that aligns it. Pointer input is the
same as anywhere else: it reaches the topmost catcher under the pointer and the
catchers around it, never one in a layer beneath.
`Grid` lines cells up in shared columns, filling them row by row: `columns`
is a `Track` per column (`Fixed`, `Intrinsic` - as wide as its widest cell -
or `Fraction`, a weight in what is left), with `column_spacing` and
`row_spacing`, and each row is as tall as its tallest cell. A cell that spans
several columns is written `<GridCell span=2>`. It is for forms and property
panels whose labels should share one width, not for a list of rows, which is
a `List` or a `VirtualList`.
A table of values to compare is `styled::Table`: column headings along the
top (each a `TableColumn`, which can carry a caption), a `TableRow` per row with its heading down the side, and a `TableCell`
per column, each with an optional caption under its value; every row shares the
column weights, so its cells line up, and it reads as a table to assistive tech:
the Table, Row, ColumnHeader, RowHeader and Cell nodes are `unstyled::Table`,
`TableRow`, `TableColumnHeader`, `TableRowHeader` and `TableCell`, each one
wrapping whatever its face lays out.
`Text` carries its own decoration too: `underline` is painted from the galley's
baseline, so switching it on never moves anything. `Portal` shows a subtree that belongs to
someone else: it takes a `NodeId`, lays it out and paints it where the portal
stands, and leaves it alone when the portal goes away, so a subtree can move
between places in the tree without being built again. Exactly one portal shows
a given node - claiming it takes it from the portal that had it - and the
subtree lives as long as the scope that built it. `Embed` reserves a rectangle
for something outside the document (`beui::native::Embed`, since only the
native app composites anything) — an editor the host composites behind the
surface — publishing the rectangle and the clip it was laid out in through the
`EmbedSlot` it was given (a `Copy` handle owned by the scope that made it) and cutting that rectangle out of the surface so what
is behind shows through. The placement also carries its window's raster over
the document's scale (`EmbedPlacement::raster`, 1 unless a host draws the
window finer or coarser), announced again when a `WindowReport::Rastered`
changes it, so whatever the host composites there can be drawn as sharp as
the window around it. `punch=false`
keeps the surface whole, for something the host draws over it instead.
`Offset` takes exactly one child, lays it out at its full length along a
`direction` and shows it from an offset; a run of rows goes in a `List` inside
it. It answers no input at all, so nothing scrolls by putting one in a view
(see [Scrolling](#scrolling)). With `fit` it measures as long as its child,
so a box sized by what it holds can still scroll once it is squeezed. Without
`fit` it measures nothing along its direction (it fills what it is given),
except when it is measured with no limit along it, as a `Percent` child is
inside a box sized by its content (a fitted sheet): then it measures as long as
its child, since nothing else could give it a length. Rows far
outside its viewport are culled like any other node out of sight. `Frame`'s
`rotate` paints its child turned about the middle of its box, which it keeps. Pointer
input and the accessibility tree still see the child unturned, so it is for pictures, such
as a preview of a turned screen, not for controls. Nothing swaps a box's width and
height for a quarter turn any more; turn a square box, as the display settings'
rotation tiles do. `VirtualList` is an ordinary box that stands for
one item per key, each of an estimated `item_size`, and builds only the ones its slice of
the viewport reaches (see [Long lists](#long-lists)).
`Scroll` takes a `direction`, so the same tag is a
column of rows or a strip of cards; a horizontal one answers Shift+wheel, a
sideways trackpad swipe, a touch drag and the left and right arrows.
A plain wheel is left to whatever is around it, the way a browser leaves a
horizontal strip alone, and a wheel only ever reaches the innermost scroll
under the pointer. The unstyled module contains
`Button`, `Pressable`, `Toggle`, `Choice`, `Slider`, `TextInput`, `TextArea`,
`Disclosure`, `Tree`, `SearchList`, `Select`, `CommandPalette`, `ContextMenu`, `MenuButton`, `Popover`, `Container`,
`PanZoom`, `PointerLock`, `Draggable`, `DropTarget`, `Tooltip`, `Floating`, `Scroll`, `Scrollbar`,
`Stack`, `Calendar`, `DateTimeField`, `TimeList`, `ColorArea`, `ColorWheel` and `Picture`.
Code outside beui uses `styled::Picture`, whose `radius` is a `Radius`, and
reaches the ones that draw nothing of their own (`Draggable`, `DropTarget`,
`Floating`, `PointerLock`) through `beui::styled`, which re-exports them. `TextArea` is the multiline one: it owns a
`text_editor_core::Core` through the `TextAreaState` its caller holds (a `Copy`
handle owned by the scope that made it; `with_core` and `with_core_mut` reach
the `Core`), lays the
document out with a gutter (line numbers and a border line, no fill: the area paints no
background, so it shows whatever is behind it), wrapping, collapsible sections and markdown
checkboxes, and lays out the inline and block `TextWidget`s the caller
names - `block` builds what goes under a block widget's line, and
`selected_widget` what floats under an inline one while it is selected - which
is how a block editor puts an embedded block inside the text and drives the
same document from a toolbar of its own. A `completer` names a trigger
character and a search: typing it after a space opens the menu
`completion_menu` draws under the caret, filtered by what follows it, with
arrows, Enter and Escape; `styled::TextArea` uses it for emoji (`:rocket`). The
menu is a passing layer with an `on_dismiss`, so Escape, back and a press
outside it all close it the same way, and the styled find bar takes `on_back`
while it is open, so back closes it as Escape does. The state republishes
what it shows whenever one of its own commands runs; code that changes the
document behind it - adopting an edit that arrived from someone else - calls
`sync()`, or `external_edit()` when the edit should also break the undo group,
since nothing polls the document for changes. It shows a `placeholder` while
the document is empty, masks every character under `password`, and leaves a
caret handle under a touch tap that drags the caret and opens its menu.
`disabled` takes the area out of focus, pointer and keys and reads as disabled;
`read_only` keeps it focusable and selectable (arrows, Ctrl+A, copy, find) but
refuses every edit (typing, IME, paste, cut, undo, checklist boxes, the
duplicate-line and replace actions, the text menu's Cut and Paste), and reads as
read-only, not disabled. `find=false` registers none of the find actions, so
Ctrl+F, Ctrl+H and Ctrl+G reach the surrounding app. `styled::TextArea` takes
the same `disabled`, `read_only`, `focused`, `line_numbers`, `prose`,
`on_content_size` and `find` (false also drops its find bar), a `size` for the
body text (a `TextSize`, `Custom` for a size that comes from content) and a
`color` (a `TextColor`): `Default` keeps the theme's syntax colours and muted
placeholder, and any other colour draws all the text in it with the
placeholder a faded copy, and a `padding` (a `Padding`, `Space::Custom` for
an inset that comes from content; the unstyled area takes `Sides`). It has no
font-size or scale prop: text drawn zoomed sits in a scaled space.
`single_line` is the same control laid out on one unwrapped line with no
gutter, scrolled sideways to keep the caret in view, where Enter submits and
Tab leaves; `frame` wraps the field in the caller's chrome inside the area's
own focus and pointer handling. `TextInput` is that single-line mode over a
plain-text buffer it owns, bound to a `value` it reads and writes, so a
fix to how text is edited lands in both. `multiline` makes it the multiline area
instead, with `line_numbers` off, where Enter starts a new line and the field's
chrome wraps the whole scrolling area rather than one line. `prose` (a `Prose`, on
`TextArea` and on `TextInputStyle`) lays the multiline area out as plain text
rather than a document: no whitespace markers and no document padding, with
the weight, horizontal alignment, padding around each line, wrapping, and where
the lines sit when they are shorter than the area that it names.
A `TextInput` face gets `clear` and `copy` in its `TextInputHandle`, so
`styled::TextInput`'s `clearable` and `copyable` buttons only draw them.
Nothing measures text off the tree: a `TextArea` reports the size its laid-out
rows reach through `on_content_size` (padding included, read from the rows'
`measure_text(..)` bounds and divided by the scale of a scaled space, so it is in the
area's own units), and a caller sizes a box from that. The infinite canvas grows a
text entity this way while it is edited, and draws each text and label as a
rich `styled::TextArea` over a `LiveText` in the entity's scaled item, the same area whether or not it is
being edited (disabled while it is not, `find=false`), so a checklist shows the styled
`Checkbox` at the size of its text (a `Checkbox`'s `size` is the `TextSize` it sits beside). Its export is the one place text is laid out away from the tree
(`TextAreaState::lay_out_prose`): the area hands each line's `Piece`s and
`RichOptions` (plain values in `beui-types`) to a layout function its caller
passes, which the canvas answers with its painter's `rich_layout`, so the
area itself names no core's layout. It gives every marker a square the height of
a line and the canvas paints it, and each quote's bar, as a pink box, since export does
not draw them yet. A `TextArea` takes its view size
from its own layout, not from where it lands on screen, so one inside a scaled
`CanvasItem` is not laid out again when the view pans or zooms; a
`VirtualList` reads the clip only while some of its rows lie outside it. A document whose language is
`TextLanguage::RichText` is rich text, structured the way Slate structures one: the document
knows nothing of bold or headings, a `Schema` (text-editor-core `schema.rs`) gives keys their
meaning, and the page renders the result. The bytes are the text alone, and every line is a
block. Formatting is a set of `Marks` beside the bytes, each setting one key to a `Value` or
clearing it, either over a stretch of text (`Scope::Inline`) or over whole lines
(`Scope::Block`, a block's props). Marks are anchored to the sequence's positions, the latest
wins where two overlap, and two copies that exchange marks in any order agree (the demo's Rich
text page sends them over its simulated network). Undo removes the marks an edit made
(`RichOp::Unformat`, which a copy sends on like a mark, and which a mark arriving after its
removal cannot bring back) rather than marking the old values over them, so text an undo brings
back never falls under a mark made for the edit, and a concurrent mark from another copy stays.
Redo makes those marks again under new ids. `Marks` keeps the resolved `Formats` and
updates them from each text splice and each new mark rather than resolving the document again,
so a document that holds marks passes every splice its sequence reports to `Marks::splice`
(and calls `invalidate` if it replaces the sequence wholesale). Each inline mark records whether it grows when
text is typed at its end (`Expand::After` for bold, `Expand::Never` for a link). `Core` owns only
generic operations: `EditorCommand::SetMark` styles the selection or what is typed next at a
caret, `SetBlock` sets a prop on the selected blocks or one at a position, `InsertVoid` puts an
atomic inline item (U+FFFC carrying its own marks, such as a mention) at the caret, and
`Transform` runs a function over `Transform`'s range-level operations as one undo step. Typed
text takes the marks before it that grow (or after it at a paragraph's start), and the schema
decides what Enter (given the line, its neighbours and whether it is empty), Backspace at a
block's start and Tab do to a block. No block traps the caret at the document's edge: Down at the
very end of a document whose last line has props, or Up at the very start of one whose first line
has, opens a plain line past it, and so does a click below the last line; a press there anchors a drag at that caret, so dragging up selects from the end. The schema also decides its tools (a name, a
shortcut, whether the selection has it, and the command it runs), each leaf's `SynHlStyle`, and
which blocks a highlighter decorates (a code block's language). `schema::basic` is a schema
with the usual marks, headings, lists, checklists, quotes and code blocks. `TextAreaState::with_schema`
makes an area for one, `TextAreaState::selected` is the selection's marks and block props,
and `styled::FormatBar` shows the tools it is given glyphs for. The area draws rich text with no
gutter or whitespace markers. In markdown and plain text, whitespace stays real text (a space
and a tab take their own advance in their own font, a `\r` none) and the area shows its markers
as the row `Text`'s `glyphs`: trailing whitespace always, any other only while selected, so
selecting never moves the text. A line's newline takes the width of an invisible `mm` in the line's font
after its text (the caret at the line's end sits before it), so a selection over it shows, and its
marker is drawn centred there while it is selected. How a document looks is three render hooks, the way Slate's are,
each keyed by the line description (`LineLook`) that markdown and rich text both give: `leaf`
(`RenderFn<TextLeaf, Span>`) turns a run of text, its resolved `SpanStyle` and its `SynHlStyle` into a
`Span`, so every run stays in the line's one `Text` and wraps and carries the caret across style
changes; the leaf draws an inline code span's background (`SynHlStyle::code`, which the markdown
highlighter sets for a code span and `schema::basic` for the code mark, never for a code block)
as the span's `background`, and `LeafSpan` is the span with a given background; `marker`
(`RenderFn<TextMarker>`) draws a list marker (bullet, number, checkbox with a `checked` binding
that toggles it) or a void (`MarkerKind`) as the content of a `TextItem`; and `line`
(`RenderFn<TextLine>`) wraps a line's text node in whatever goes around it (quote bars, a code
block's background, a list's indent), keeping the area's horizontal padding it is handed. The
engine says what is on a line, never how big: each marker is measured by its `Text` during
layout, and wrapped lines hang at the laid-out x after it. A markdown line's marker covers its
leading indentation and its `>` and list marker bytes, so they are drawn by the marker item (the
caret still steps through them) rather than shown as text. Both kinds of list look the same:
`MarkerColumn` puts a list marker in a column `MARKER_COLUMN_EM` of the text size wide, right
aligned so numbers line up on the dot (a wider marker widens only its own line), and
`TextLine::indent` indents a line by a fixed `LIST_INDENT_EM` per nesting level and
`QUOTE_INDENT_EM` per quote, whatever the source's spaces. A markdown line that continues a list
item (`LineLook::continues`) hides its indentation the same way and is indented to its item's
text. The
three hooks come in a required `TextAreaLook` beside the required `TextAreaColors`, `font_size`
and `padding`, with the area's `TextAreaGeometry`: every absolute size it draws or lays out with
(caret width, padding around a document line, room below the last line, the gutter, run chips,
inline widget pills, and block spacing and table cell padding, which are given for
`BODY_SIZE` text and scale with `font_size`). Unstyled keeps only proportions of the text size
(list and quote indents, margins, heading sizes). `TextAreaLook::plain(geometry)` draws text and
nothing else (no code backgrounds, bars or markers), which is what `TextInput` uses with
`TextInputStyle::geometry`. `styled::text_area_colors(&theme)`, `styled::text_area_geometry()` and
`styled::text_area_look()` build them for a theme; the styled look draws with `DocumentLeaf`,
`DocumentMarker` and `DocumentLine`, which use theme tokens, and a caller that draws voids handles `MarkerKind::Void`
and hands every other marker to `DocumentMarker`. Copying and pasting
carries the text without its marks. A menu is never asked for as a dropdown
or as a sheet: every menu opens as a dropdown when a mouse opened it and as the
same items in a `ModalSheet` when a finger did, so the two show the same
features. That choice is `unstyled::Anchored`'s, the one surface every anchored
popup sits on (`MenuPopup`, `Select`'s options, `Popover`, and through it the
date and colour pickers): an overlay under `anchor` at `placement`, or the same
content in a sheet. It looks at whether the press that opened it was a
finger's (`touched_by_finger`, `Document::touched_by_finger`: whether the last
press was a finger's, until a key is pressed; a pen points as precisely as a mouse), so no trigger
tracks touch itself; `sheet` (`SheetWhen::Touched` by
default, `Always` or `Never`) overrides that. Its `AnchoredStyle` takes both
looks with no default: `dropdown` wraps the content in a panel and `sheet`
renders an `AnchoredSheetHandle` (`open`, `on_close`, and `content`, a `Render` the sheet builds inside itself so focus lands within it), styled as a
fitted `ModalSheet` (`styled::popover_surface(max_width)` for a popover's).
Its content is a `RenderFn<AnchoredHandle>`, built once per look the first
time that look opens; the handle's `sheet` says which and `shown` holds while
that one is showing, so focus binds to it rather than to `open`. A
`MenuStyle` carries the menu's rows for both looks and the spacing between rows
(`MenuStyle::new(spacing, row, panel)`; the styled `menu_style()`), and an
unstyled one without a sheet stays a dropdown. Every component with a menu
requires its `MenuStyle`; a text field's context menu is
`TextInputStyle::menu: Option<MenuStyle>`, where `None` means no menu. An item's children cascade as a
submenu in a dropdown; in a sheet, or with `page`, they open in place of the menu as a page led by a back row
titled by the item (`<id>.back` for an item with a `row_test_id`); Left, Escape
and the back row return to the item, and the dropdown or sheet resizes to each
page. `then` says what choosing an item does: `MenuThen::Close` (the default),
`Back` to the page before (a location picked on a page), or `Stay`. `MenuCaption` is a
row of text nothing can choose, and `MenuGrid` lays its child items out as
tiles in `columns`, which the arrow keys walk in two dimensions and leave for
the rows above and below. A page of rows with a `depth` is a tree: `expanded`,
a `Binding<Option<bool>>` (`None` for a leaf), draws the chevron, Right and Left
and the chevron write `Some(open)` to it, and the caller shows the rows of open
branches (a `ForEach` over them), as with `Tree`. A row whose `checked` is `Some(true)` shows a check (with
`radio` it is read as one choice of a group), and
a page opens with the keyboard on that row. A row with an `action` takes its
label, glyph, enabled state, tick and first shortcut (drawn muted at the row's
end, or `shortcut` given) from it. `MenuNodes` builds the rows from a
description of actions, submenus and separators (`MenuNode`, see
guides/beui_keyboard.md, Actions and the command palette), each row's test id
`<test_prefix>.<id>`, so one description fills a context menu, the dock's More
menu and a plugin's frame menu. `lead` is a few muted words before
an item's icon on the same line, read with its label (`New in` before a folder's
name). The dropdown is as wide as its widest row, between a least width and a
greatest one past which labels end in an ellipsis, is as tall as its rows up to
the window's height, scrolls beyond that, and scrolls the row the keyboard is on
into view. `MenuPopup` is the same menu
opened by something other than a button, such as a model holding a request: it
shows while `open` holds, under `anchor` at `placement`, as a sheet when a
finger opened it or `sheet` holds, at least `min_width` wide, and reports `on_dismiss` for Escape or a press
outside. A host that opens one for a plugin's button (block-app's app menu)
anchors it to the button's rect, which the plugin sends from `node_anchor`; the
host routed that press, so the surface knows whether a finger made it. `MenuButton` is the button that
opens a menu under itself, which is what a toolbar reaches for where `Select`
would imply the choice sticks. Its face is text, an icon (`icon_only`) or a
picture (`image`, the tray's icons), in any `ButtonSize`, and it looks pressed
while its menu is open; `ContextMenu` is the same menu on a secondary press, and it
also takes an `open_at` point so a touch gesture can raise it where the finger
was. A text selection's menu
sets `selection` and stays beside the selection under a finger too. A finger
held still for the long-press delay (`Context::set_long_press_delay`, which a
test sets to zero rather than waiting) is a secondary press where it rests, so
every context menu opens on tap-and-hold; the press the finger began is
cancelled and lifting it is not a click. An `Interactive` hears that
cancellation, and a second finger landing, as `on_cancel`, which is where a
gesture in progress is dropped rather than committed. A finger dragged across a
`Interactive` is read as a scroll of whatever holds it that scrolls that way
(where nothing does, it stays the catcher's drag) unless the catcher sets
`touch_drags` (a `TouchDrags`, CSS's `touch-action` turned round):
`TouchDrags::All` (or `true`), which a canvas that draws or moves things under
the finger sets, or `TouchDrags::Along(direction)`, which keeps finger drags
along one direction only and leaves the other to the scroll around it, as a
dock tab in a scrolling tab bar does.
A pen is `Event::Pen`: a `PenPhase` (`Move` in range, whether or not the tip is down, `Down`
and `Up` of the tip, which is `Primary`, and of its barrel buttons, and `Gone` when it leaves
range) and a `Pen` with its pressure from 0 to 1, its tilt in degrees and whether it is the
eraser end. It is `PointerId::Pen` and behaves as the mouse does: it hovers, moves the pointer,
presses and drags without a slop or a long press and never scrolls what it is dragged across,
so every control works with it unchanged (`PointerId::hovers` is true for both). Its
`PointerEvent`s carry the `Pen`, and the `PointerPress` a catcher hears carries it too, as
`pen`, sampled when it was pressed and at each drag; a finger's `PointerEvent`s carry its
`force` where the platform reports one. The DRM
platform reads libinput's tablet tools, the web platform pointer events whose `pointerType` is
`pen`, Android a stylus or eraser `MotionEvent`, the winit platform winit's tablet-tool pointer
events (on Wayland and Windows; X11 and macOS report none), and drive's `pen` commands make one.
A finger that lands on no control is taken to the nearest `Interactive` that
takes presses within `TOUCH_REACH` of it, for the whole touch, so every control's
touch zone is bigger than it looks without anything growing; a direct hit always
wins, so a neighbour never takes a tap aimed at the control beside it. A
catcher that only watches presses over a whole region - a dock pane focusing
itself - sets `claims_touch=false`, so landing on it is not a direct hit and the
controls inside it keep their reach. A quick
tap with two or more fingers that did not move is a finger tap, which
`on_finger_tap` hears the way `on_shortcut` hears keys, unless a layer that
blocks everything is open; the editor frame's top
bar undoes on two and redoes on three. `Sheet` is the panel that rises from the
bottom of a narrow screen. It scrolls what it holds itself, so what goes in it
is not wrapped in a vertical `Scroll` (one inside would measure nothing tall): a
swipe anywhere on it raises it to its top stop before it scrolls the content,
and lowers it once the content is back at its start. It is never taller than
what it holds, whatever stop it rests at, and pulled past its top it stretches
like an overscroll. It slides up as it opens, easing out, and slides down as it
closes, easing in from however fast it was flicked; a `ModalSheet` stays up
while it leaves however it was closed, and fades its scrim with it. A sheet
that opens within a moment of another closing in the same document (a menu's
sheet giving way to the sheet of the dialog it opened, either order in the same
frame) takes the other's place instead: the closing one goes at once, and the
opening one starts at the height the other showed and eases to its own, so the
two read as one sheet changing its content (`Slide::Morphing`). Let go, it springs to the
stop nearest where the flick was heading, or closed below the lowest one, and
its content flings and bounces at its bottom end as a `Scroll` does. A fling
reaching the content's start bounces the whole sheet down and back without
changing its stop or dismissing it. Dragging the handle, or the backdrop above
a modal sheet, resizes an unscrolled sheet between its stops without scrolling
its content. When the content is scrolled, the same drag translates the whole
sheet without resizing it or changing its scroll position: release returns it
to fully open or dismisses it, with no intermediate stop. The
handle also works with a mouse, and going back closes the sheet. Styled sheets
fade the content at edges with more to scroll, using the same opacity mask as
styled scrollers; unstyled sheets take a concrete `fade` length.
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
`Button`, `Link`, `Checkbox`, `Select`, `TextInput`,
`NumberInput` - takes a `disabled` prop: it stops answering the pointer and the
keyboard, leaves the tab order, publishes itself as disabled to a screen
reader, and paints in muted colours, which is what a read-only editor binds
`editor.read_only()` to rather than leaving a live control that quietly throws
edits away. The re-exports in `unstyled.rs` and `styled.rs` are the authoritative
lists.

### Scrolling

Scrolling exists at all three layers, and app code wants the styled one.
`styled::Scroll` is a scroll with the project's
scrollbar already over its edge, so a panel that scrolls is one tag:

```rust
view! {
    <Scroll @sizing=ItemSize::Percent(100.0)>
        <Rows />
    </Scroll>
}
```

The focus ring is always the theme's accent, with no way to turn it off; like
`:focus-visible` it shows only when the keyboard brings the focus there, so a
click or a tap never shows it. Moving the focus scrolls the control into view with
its ring, but a ring drawn outside its control still needs room inside the scroll at the
content's edges, which is why the padding goes inside a scroll. A scroll that is one part of
a padded surface, its content lined up with what sits beside it, takes `bleed` instead
(`Bleed::POPOVER`, `Bleed::CARD`, `Bleed::DIALOG`, with `.top(false)` and its siblings to keep a side in): the
scroll and its scrollbar reach out into the surface's padding up to its border, and the content
is padded back in by as much, so it lines up as before and its rings have the padding to draw
in. The unstyled `Scroll` takes the same as `Sides`, which it builds from a `Frame` with that much
negative padding around it; a negative side of a `Frame`'s padding lays its child out that far
past the frame. `on_change` still reports the position for anything else that wants it.
With `fit` it measures as long as its content, so a box sized by what it holds
can still scroll once it is squeezed.
With `max_length` the scroll is as long as what it holds, up to that length, and
scrolls from there on, which is what a popup listing a few rows or many wants.
With `follow_end` (a log, a terminal) the scroll starts at its end and stays
there while what it holds grows, as long as it was at the end: scrolling away
leaves it where it was put, and scrolling back to within a point of the end
follows again. `ScrollApi::scroll_to_end()` (from the `api` prop) puts it at the
end on demand and, if it follows, follows again, which is what a terminal does
when you type. Both are the base `Offset`'s (its `follow_end` prop and
`Document::offset_to_end`): while it is pinned to the end it lays its content
out from the end it measured, skipping the anchor and any shift its content
reports. It tells the content so through `Document::pinned_to_end` while it
places it, and a `VirtualList` along that direction then builds its rows from
its last one upwards, measuring each, and lines the last up with its own end
rather than walking down from the row it drew from last: rows that measure
differently from the estimate change only the length above them, so the next
layout of the frame, at the end of the corrected length, finds the same rows and
settles. A
`Frame` with `max_height` around a scroll does not do this: a `Frame` with a
maximum fills the space it is offered up to it.

### Long lists

Layout already skips the work for nodes far from the view. Every node is
still measured, since sizes need them all, but `layout::layout` *culls* a node
whose rect ends more than `CULLING_MARGIN` outside the visible part of its
parent's painter: it keeps its rect, so `measure`, `reveal_node` and
`Document::node_rect` still find it, but nothing inside it is laid out, painted or
interacted with (`Document::is_culled` says which). This holds for the children
of any container, and the node holding the focus and its ancestors and
descendants are never culled. A `Canvas`, which lays out only the items its
view reaches, likewise always lays out the item holding the focus, so a field
panned out of sight or under an on-screen keyboard keeps the focus and keeps
asking for its input method instead of the keyboard closing and reopening as
the field leaves and returns. Each placement records its `Sight`, the range of
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
        <VirtualList keys={row_ids} item_size=ROW_HEIGHT>|id: ReadSignal<RowId>|
            <Row id />
        </VirtualList>
        <Footer />
    </Scroll>
}
```

`item_size` is an estimate, and a row that measures differently is laid out at
the size it measured. The list remembers what each row it has built actually
measured and reports the sum, counting each row it has not built at the average
of those it has, so its length is close to the truth from the first layout and
the last row can always be scrolled to. A row that is measured
above the viewport would move everything below it, so the list keeps drawing
from the row it drew from last frame and asks the scroll around it to move its
offset by the difference instead; the scroll applies that before it places
anything, so nothing on screen shifts. Outside a scroll a `VirtualList` builds
what fits in the box it is given, which is how one sized by a `Stack` builds a
screenful in one layout and a hundred pixels' worth in the next.

`overscan` builds that many pixels of rows beyond each edge of the window, so the
row a key press moves to already exists to be revealed. A list scrolled to its
very top shows a row inserted above the rest rather than holding the rows below
in place.

Rows belong to their keys, not to their positions. Scrolling reuses the rows
that stay in view, and keeps the ones that just left, as many as are in view
and only while they are within a window's length of it, so scrolling back
shows them without building them again; anything further away is disposed
with its effects, so a long jump or a list scrolled out of view keeps nothing
but the row holding the focus. That row stays a child of the list, laid out at
its own place off screen, until the focus leaves it, so the focus stays where it
was. Tab order runs over every key, built or not: Tab and Shift+Tab reaching a
run of rows the list has not built build them one at a time, from the end they
come in at, until one has a Tab stop (rows with none are released again, and
the search gives up past 4096 of them), and the focus then reveals that row as
it would any other. Elements opt into this through `Element::focus_children`,
`build_for_focus` and `release_for_focus`.
A row the list disposes is not dropped but recycled: the item builder is
handed a `ReadSignal` of its key rather than the key, and when a key comes into
view the list sets that signal on a row that left instead of building a new one.
So the builder must derive everything it shows from the key signal (a memo of
the label, a test id, the selection), read it untracked in event handlers, and
expect any state it keeps of its own to carry over to the next key. A row that
must start fresh for each key wraps its content in `<Dynamic value={key}>`,
which rebuilds on a key change and so costs what building used to. A row
holding focus is never handed another key, and the list keeps at most as many
spare rows as it has ever shown at once. The item builder runs once per row it
builds, and the key signal changes once per key it then shows. Changing `keys`
keeps every row whose key is still there: inserting a key anywhere shows only
its row, and only if it lands in view; removing one releases only its row; and
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

`Selectable` (`beui::primitives::Selectable`, with `SelectableState`,
`copy_selection` and `select_all` in `beui::selectable`; `styled::SelectableText`
over it) is each core's own read-only selection, not a composition: on the
HTML core it is the browser's selection, and on the native core it is
`beui-view/src/native/selectable.rs`, built on `beui_view::selection`. The
native one selects across the plain `Text`s
under it and across any `VirtualList` under it with the same component. Each
`VirtualList` registers its rows (`beui_view::selection::SelectableRows`: its
keys, which key each built row shows, and its optional `text` prop, a
`Callback<K, String>` used only for selection), and the selection walks the
region in tree order, taking a list as one item rather than its built rows.
A caret inside a list is a key, the index of the `Text` within that row and a
byte index, never a node, so a recycled row cannot carry a selection to
another key. A press maps the built `Text` under it to its row's key and
forgets the node; every built row paints which of its bytes the selection
covers, and a row built later (scrolled into view, or recycled for another key)
paints from the same selection as it is built. Select All goes from the first
key's start to the last key's end without building anything, and copying walks
the keys between the ends, reading built rows from their `Text`s and the rest
from `text`, one line each (a row's several `Text`s are joined by line breaks;
a list given no `text` copies its unbuilt rows as empty lines). A key removed
under the selection moves that end to the nearest row that remains, the start
forward and the end backward, and a row whose text shrank is clamped to it.
Dragging past the top or bottom of the nearest vertical scroll around the
region scrolls it while the pointer is held there (`repeat_drag`).
`selected_text` reads the document, so call it from a handler or inside
`with_reactive_scope`.

A list the layout stops reaching - scrolled out of view beside other content, or
inside a scroll that collapsed - is told so through `Element::unplaced`, which
the layout calls on every node it placed last pass and did not place this one.
The list releases its rows there rather than holding a screenful it cannot see.

`unstyled::Scroll` is the same arrangement without the appearance: it owns the
base offset, the input that drives it, the position it reports, and the layer
that lays the bar over the content's far edge, and it takes a `ScrollbarStyle`
saying what to put there.
That is the seam the styled layer fills, with the width of the strip the bar
covers and a builder that is handed a `ScrollHandle` of the live `position`,
the `direction`, a `scroll_to` that drives the offset the way the wheel and the
arrow keys do, the bar's `visibility`, and an `on_engage` to report the pointer
on the bar:

```rust
ScrollbarStyle::new(SCROLLBAR_WIDTH, |handle: ScrollHandle| {
    let ScrollHandle { position, direction, scroll_to, visibility, on_engage } = handle;
    view! {
        <Scrollbar
            position
            direction
            visibility
            on_scroll_to={move |offset: f32| scroll_to.call(offset)}
            on_engage={move |engaged: bool| on_engage.call(engaged)}
        />
    }
})
```

The bar is an overlay, as on macOS: it takes no space from the content, and it
shows only while it is wanted. `visibility` goes to 1 when the offset or the
overscroll changes and while the mouse is within the strip's width of the edge
or on the bar (`on_engage`, which `unstyled::Scrollbar` reports for its whole
rect and while it is held), stays there for `SCROLLBAR_LINGER` after the last
of those, and then falls to 0 over `SCROLLBAR_FADE_OUT` on a timer that stops
once it is hidden. `unstyled::Scrollbar` takes it as `shown` and turns down
every point while it is false or the content fits, so a press or a finger on a
hidden bar reaches the content beneath; a finger can drag the bar only while a
scroll has just shown it. The styled bar fades its colours by `visibility`,
paints only the thumb while the scroll moves, and adds the track while the bar
is engaged. A scroll keeps its content clear of the bar, if it needs to, by
padding inside it, which the edge-to-edge rule already asks for.

The bar answers the pointer itself. `unstyled::Scrollbar` owns that: it takes
the position, the direction and an `on_scroll_to`, hit-tests the press against
the thumb it would paint, drags the thumb with the pointer, pages by a viewport
towards a press on the track either side of it, and hands its content a
`ScrollbarHandle` of `hovered` and `dragging` so the styled bar can paint those
states. `thumb_start` and `thumb_length` are the same fractions both layers
work in, so what is painted and what is pressed cannot drift apart.
`ScrollbarThumb` paints the thumb at those fractions of its rect rather than
laying it out, so a scroll's new position reaches its bar without laying
anything out again; a bar of your own puts one inside its track. The styled
bar paints its track inset by `SCROLLBAR_INSET` from the scroll's outer edge and
from both ends, so it does not touch the separators around a panel; the inset at
the ends sits outside the unstyled bar, keeping those fractions measured over
the track that is painted. A press on
the bar rests whatever momentum a fling left, so the content stops where it is
put.

Without one the scroll shows no bar, which is what the unstyled layer does on
its own. A control that scrolls something of its own takes a `ScrollbarStyle`
and passes it down, the way `unstyled::Select` hands one to the scroll behind
its options, so the styled control decides the bar and the unstyled one never
names a colour. The bar paints nothing while its content fits, and since it
reserves no gutter, a scroll that grows past its viewport does not shift its
content.

`ScrollbarStyle::fading(length)` also fades the content out toward each edge
that has more content beyond it, over at most `length` points and only as far
as the content has scrolled, so a scroll whose rows happen to end exactly at
its edge still reads as scrollable. The styled layer's scrollbar style fades by
`theme::SCROLL_FADE`, so `styled::Scroll` and every styled control that scrolls
fade. `unstyled::Scroll` works the widths out from the position its offset
reports and wraps the offset in a base `Frame` with those `fade_edges`, which paints its child inside a
mask group (`Painter::faded`, `Group::Mask`): the wgpu renderer multiplies the
alpha of everything in that group by it in the shader. Custom `Drawing`s are not faded.

Under both sits the base `Offset`, which is named for what
it does rather than for what it is used for: it shows its one child along a
direction from an offset, with no bar, no theme, no fade and no input of its
own. A wheel, a touch drag and the arrow keys are `unstyled::Scroll`'s,
which puts its children in a `List` inside the offset, wraps that in a
focusable `Interactive` for the keys, the wheel and the drag, keeps the
momentum an unfinished fling carries, and drives the offset from all three. So reach for `Offset` when something needs its
content shifted under a viewport and nothing more, and for a `Scroll` whenever
something needs to scroll.

A `Scroll`'s momentum is one motion across the edge, not a fling with a band
laid over it. It keeps how far its content has gone past an end unbanded (the
fingers' distance in a drag, the fling's travel after a fling crosses the end),
and shows `rubber_band` of it, a curve that leaves the edge at the speed it is
pulled and stiffens from there. `rubber_band::bounce` moves that stretch: going
out it is an undamped spring, so a fling keeps its speed as it crosses the end
and slows smoothly, and from its furthest point a critically damped one brings
it back without passing the edge. A band let go while already heading back
hands the speed it still has to a fling into the content. The sheet's content
bounces the same way.

`Offset` never clamps: it shows its child at whatever offset it is given, past
either end included, and the band is just the part of that offset beyond them
(`ScrollPosition::overscroll`, with `clamped` the part within). Keeping the
offset in range is whatever drives it. When content shrinks under a `Scroll` or
an offset is set past an end while nothing is moving it, the `Scroll` puts it
back at the end in its report of that layout and holds the report back, so the
next layout pass of the same frame lays out and reports the settled position and
nothing past the end is ever painted.

That offset is anchored to a node, not measured from the top of the content:
the scroll remembers the node at the top of its viewport and where it started,
so a row further up growing or shrinking - a wrapping label, an image that
finished loading - leaves what is being read exactly where it is. It finds that
node by walking down from its child through the nodes that lay their children
out in place (`Element::passes_scroll_anchor`: `List`, and a `Frame` that is neither translated nor rotated),
taking the first child that reaches past the top of the viewport each time.

The offset places its child once per layout. What the content's own layout
decides about the anchor comes back to the offset as part of that layout's
result: a `VirtualList` whose rows above the view measured differently from
their estimate reports the difference with `Document::report_anchor_shift`,
and the nearest enclosing offset sums what its child reported with how far the
anchor drifted and any reveal it was asked for. When that sum is not zero the
offset moves by it and is laid out again in the frame's next layout. An offset
picks its anchor once a frame, from the layout the frame started from, which is
what was painted, and measures the drift against it in each of the frame's
layouts, moving by what it has not moved by yet; so content that changes in a
later layout of the frame, such as a `Container` that heard its new width,
keeps the anchor in place too, and an anchor picked from a layout the frame
replaced never steers it. Anything that moves the offset from outside its layout
(`set_offset_value`, a reveal) calls its `on_steer` callback, which a `Scroll`
uses to stop a fling or a spring the moment it is steered.

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
picking a day moves on to the time. Its `value` is a `Binding<Option<DateTime>>`: an
empty field shows placeholders, a field writes its binding only once every
segment is filled, and a field left half filled goes back to its value when the
focus leaves it. A `Date` field keeps the time of the value it
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
moves and written to `value` once, when it ends, the way `NumberInput`
writes a scrub - so an edit lands in the undo history once per gesture.
That state is `unstyled::ColorPickerState`, and `unstyled::HexText` keeps a hex
field in step with a color for both. `unstyled::ColorInput` holds the preview
and puts the popover and the hex field together; its `swatch`, `picker` and
`field` renders draw the parts and its children lay them out.
`styled::SwatchButton` is a single round swatch that picks one colour: a
button labelled with the colour's name (its tooltip too), ringed in the accent
while `selected` and in the border colour under the pointer, for a row of
preset colours that is not a whole picker. Its `radius` is `Full` by default;
a palette of square cells passes `Radius::Xs`.

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
a `Drawing` that a shader paints on the wgpu renderer, behind
beui-components-styled's `gpu` feature; without it (the HTML core) the same
faces are computed on the CPU into an `Image`.

`unstyled::Popover` is what both open: a trigger and an overlay, built the
first time it opens, that traps Tab, closes on Escape, a press outside or its
handle's `close`, and gives the focus back to the trigger when it closes. It
dims nothing, so its layer passes input through (`LayerPolicy::Pass`): the
pointer goes on hovering the document around it, and a press outside asks it
to close and then lands on whatever was pressed - except a press on its own
trigger, the overlay's `trigger`, which is left to the trigger so a second
click closes it rather than reopening it. Whether it is open is its `open`
binding, as with `MenuButton`'s: a caller that opens it, or does something when
it opens or closes, passes a signal rather than listening for a change. What
it holds decides where the focus lands when it opens, by binding a `focused`
prop to the handle's `shown`, as the calendar, the time list and the color area
all take. It is an `Anchored`, so a finger opens it as a sheet; its required
`surface` is the `AnchoredStyle`, so its content is only what goes inside the
panel (`styled::popover_surface`).

### Tooltips

`unstyled::Tooltip` shows something after the pointer has rested on its child
for a moment, and `styled::Tooltip` is the bubble version of it:

```rust
view! {
    <Tooltip label="Reveal the block being shown">
        <Text color=TextColor::Muted><Icon glyph=ICON_LOCATE_FIXED /></Text>
    </Tooltip>
}
```

An icon-only button needs no wrapper: `unstyled::Button` and `unstyled::Toggle`
take `icon_only`, and their handle's `tooltip` is the label (with the action's
shortcut) while the button is icon-only and nothing it opened is showing, and
empty otherwise. The styled faces wrap themselves in a `Tooltip` of that text,
so the `label` a button publishes to a screen reader is the text the bubble
shows, and a menu button's tooltip goes away while its menu is open. Wrap anything else whose meaning is not
on screen - a bare glyph in a row, a truncated cell - in a `Tooltip` of its
own.

The bubble lives in an inert `Overlay` (`LayerPolicy::Inert`). It paints
above what it hangs from, and unlike a menu or a dialog it takes no input at
all, so the document under it keeps answering the pointer, Escape still reaches
whatever it was going to reach, and clicking the control the tooltip describes
clicks the control. The dwell is a `create_timer`, started when the
pointer arrives and stopped when it leaves, so a tooltip nobody is hovering
costs no frames.

### Time: timers and animation

beui has no per-frame hook. Work runs because a signal it reads changed, and
anything that genuinely depends on time asks for it with `create_timer(work)`.
The timer belongs to the reactive scope that created it and is dropped with
it; the `Timer` handle is `Copy`, and calling it after that scope is gone does
nothing. `start(delay)` schedules `work` (keeping an earlier deadline if one is
already set), `restart(delay)` replaces the deadline, `stop()` cancels it, and
`running()` says whether one is pending. The document runs due timers inside
its reactive scope at the top of a frame and asks the context for a repaint
at the next deadline, so nothing is drawn while no timer is due. `work`
returns `Some(delay)` to run again - an animation returns
`Some(Duration::ZERO)` for the next frame - and `None` once it has settled,
which is what lets a scroll fling or an indeterminate progress stop costing frames. Start a
timer from the event or effect that begins the motion rather than leaving one
running.

UI motion uses `create_animation(step, finish)`, not a repeating timer. Its scope-owned,
copyable `Animation` handle has `start()`, `stop()`, `finish()` and `running()`; starting
an active animation keeps its clock, stopping cancels without completion, and finishing
applies its terminal state exactly once. The step receives `AnimationFrame` (`now`,
`elapsed` since start, and `delta` since the previous step) and returns whether another
frame is needed. Returning false finishes it. A callback may start another animation;
disposing the scope cancels without completing it. Publish values through signals.

Motion that never ends, such as an indeterminate progress bar, is not an animation. An
animation asks for the very next frame, and a driven app settles only once no frame asks
for another (guides/running/drive.md), so one that never finishes would keep `drive settle`
waiting for as long as the clock runs. Tick it from a timer that asks for its next step a
frame's time later (`Some(Duration::from_millis(16))`), as a caret's blink waits out its
interval: a frame asked for later does not keep an app from settling.

Use `tween(elapsed_seconds, duration_seconds, Easing)` for duration-based progress and
`Spring::advance` for a displacement and velocity relative to a target. Tweens catch up
to the frame clock after stalls; springs solve damped motion analytically rather than
clamping away elapsed time. Retarget a spring by changing its target while keeping its
current value and velocity. `Fling` supplies the shared inertia curve. Components keep
interaction decisions and semantic completion actions, not their own frame scheduling.

`Motion::Instant` finishes active animations and completes new ones without scheduling
animation frames; the default is `Animated`. It is one of the preferences (below). Each
document exposes `motion()` and the reactive `watch_motion()`. Instant mode disables inertia
and rubber-banding, keeps carets visible and indeterminate progress bars still, and leaves held gestures
following input normally. Functional timers (expiry, key repeat, media and clocks) still
run; keep visibility delays separate from the fade that follows them. Switching back does
not replay completed transitions; repeating indicators explicitly resume. Appearance and
motion are independent preferences, even for the e-paper theme.

### Preferences

`Preferences` (a `ThemeKind`, a `Motion` and a `CanvasTouch`) is what every document follows. Their types live
in `crates/ui-types`, which the plugin protocol shares, so a plugin's preferences are the
host's without conversion. A `Context` holds three `PreferenceLayer`s, each an `Option` per
preference, and resolves them top down, falling back to `Preferences::default()`:

- `PreferenceSource::Developer`: the inspector's Simulation tab. Never saved; its No override
  choices clear it.
- `PreferenceSource::App`: what the app chose, from its own settings.
- `PreferenceSource::System`: what the platform prefers. Platforms report it with
  `WindowReports::system_preferences`: winit from the window's theme and, on Linux, the
  desktop portal's `color-scheme` and `reduced-motion`; the web platform from
  `prefers-color-scheme` and `prefers-reduced-motion`; Android from night mode and a zero
  animator scale; the DRM platform says e-paper and instant motion on an e-paper panel; the
  plugin platform reports the host's resolved preferences, which reach a plugin in its
  acceptance and through the host interface's preferences-watch; plugin tests send them with
  `BeuiTest::set_preferences`.

Set a layer with `Context::set_preference_layer(source, layer)` and read them with
`preference_layers()`. Each document mirrors them at the start of a frame (`preferences()`, and
`watch_theme()`, `watch_motion()` and `watch_canvas_touch()` for one each);
`Document::watch_preference_layers()` is that, reactively, for settings that show where a value
comes from; components read these as `use_preferences()` and `use_preference_layers()`, and
write a layer through `use_preference_layer(source)`. An app that keeps its own choices passes
`RunOptions::saving_preferences(name)`, which loads and saves the app layer in the platform's
config folder (beui-demo does); one that does not still follows the system. block-app stores
them in its display settings instead (guides/the_new_block_stack.md).

`pixels_per_point()` is the document's scale as a signal, for layout that
depends on it.

### Overlays, and things that float

An overlay is laid out and painted above the rest of the document rather than
among it. Every open one is a layer, and the layers form a tree: a layer's
parent is the layer holding the node it hangs from, which is its anchor when it
is anchored to a node and otherwise the overlay node itself, and the document
root is the bottom layer. `Document::layers()` is that tree as one ordered
list, bottom to top, and paint, hit testing, focus, Escape, back and the key
dispatch all walk that same list. Siblings that block everything sit above
those that do not, and otherwise are ordered by when they were opened or raised
(`Document::raise_overlay`), so raising a window never lifts it over a dialog;
children sit above their parent and rise with it, and a `level` (`LayerLevel`) puts a layer and everything above it over
every layer of a lower level: `Top` for things that must float over every
window and dialog, such as a drag preview, a volume OSD or `styled::Toasts` (so
a toast stays readable and pressable while a dialog is open), and `Lock` for
the lock screen.
Opening a layer raises it and every layer it hangs in, so a menu opened in a
window that was covered comes up with its window; an inert layer rises alone,
so a tooltip shown over a covered window leaves the window where it was.
`Document::layers()` is worked out once for each change to the tree or the
layers and shared until the next, so asking for it again costs nothing.

An overlay is anchored to a node, a point, a rect in document coordinates, or a
rect pinned to a node (`OverlayAnchor::Pinned { node, rect }`): the rect is
taken in document coordinates and, the first time the overlay is placed, held
by its offset from the node's nearer edge on each axis, so it follows the node
when it moves or resizes and the layer still hangs from the node.
`Placement::Popover { gap, margin }` places content like a popover: centred on
the anchor and `gap` below it, flipping above where only that side has room and
shifting sideways to stay `margin` inside the screen (the vertical choice and
the shrinking when neither side fits are `BelowStart`'s). Given a `pointer`
frame in its content, the overlay moves the content so that frame, not the
content's padded edge, stands `gap` off the anchor, and given a `beak` reach it
draws a beak of that reach on the frame's top or bottom edge, whichever side it
opened on, pointing at the anchor's middle. The overlay chooses the side, so it
draws the beak, filled and outlined like the frame so the frame's border runs on
around it, and kept clear of the frame's rounded corners; nothing is drawn while
the anchor's middle is past them. The beak lies outside the frame's box without
taking room, so give it room in a padded frame around it: a plugin's host draws
what is outside an overlay's content under the editors it holds. The styled dock
gives its cards `POINTER_REACH`.

An `Overlay`'s children (or `content={move || ...}`) are built when it opens
and taken out of the tree when it closes, like a `Show`: a closed tooltip,
select, menu or prompt costs its overlay node and nothing else, and nothing in
it is focusable, hit or in the accessibility tree. `keep_alive=true` makes it a
`ShowKeepAlive` instead: built the first time it opens, then kept with its
state while closed (a dialog, a popover, a dock's window). Its children are
built more than once, so they are an `Fn`; a component holding a `Render` it
can call only once hands it over as `render.once()` to a `keep_alive` overlay,
which never calls it twice. State the rest of the component reads, such as
which rows a select's search hides, belongs outside the content, derived from
data rather than from nodes that may not have been built.

A close that animates is the overlay's business, not its owner's: the content
calls `use_overlay_presence()` and hands `linger_while` a memo that stays true
until its exit has finished, and the overlay stays open and keeps the content
until then, so its owner writes `open` false and sees nothing hidden in
between. `ModalSheet` slides away this way.

Each layer has one `policy` (`LayerPolicy`):

- `BlocksAll` (the default) - a menu, a select popup, a dialog. Its content
  takes the pointer, and a press anywhere else is shut out: it asks the layer
  to close and goes no further, and nothing beneath it is hovered.
- `Pass` - a window, a toast, a popover. Its content takes the pointer where it
  actually is, so a press on it does not land on what is underneath, and
  everything around it stays live.
- `BlocksParent` - modal to the node it hangs from alone. A press on that node
  is shut out and asks the layer to close, the focus cannot enter it (Tab skips
  it, and focusing something inside it focuses the layer's first focusable
  instead), and everything else stays live. A modal `DockDialog` is one.
- `Inert` - a tooltip, a drag preview. It takes no input at all, so the
  document underneath goes on answering the pointer and the keyboard as if it
  were not there.

An overlay that is not a window of its own is shown in the window it was
opened in, unless `shown_in` names another of the document's windows where
windows open: it is then laid out in that window's viewport, painted in it and
counts as in it (`Document::window_of`).

`traps_focus` (on unless a caller turns it off) keeps Tab inside the layer and
makes it a boundary for keys (see the keyboard guide): a menu, a dialog and a
popover trap; a window does not. A layer that blocks everything but traps
nothing, such as the touch selection's Copy and Paste menu, leaves the focus in
the text field beneath it.

Closing is asked, not done: `on_dismiss` is called with a `Dismiss` reason -
`Escape`, `Back` or `OutsidePress` - and its owner closes the overlay by
writing `open` false, or decides not to (a modal `DockDialog` asked by a press
on its parent flashes instead). An overlay with no `on_dismiss` cannot be
dismissed. There is one request for each reason, sent to one layer:

- Escape that nothing on the focus path took goes to the nearest dismissable
  layer holding the focus, or with nothing focused, the topmost trapping layer.
- Back - Android's back gesture, or the Back key or mouse button - goes to the
  topmost layer that has an enabled `on_back` handler in it or is dismissable,
  stopping at the first layer that blocks everything; the root's handlers come
  last. `WindowRequests::handles_back` says whether anything would take it.
- A press outside goes to every dismissable layer it is outside of, from the
  top down, until it lands in one or is shut out by one that blocks everything
  (and then to the chain of blocking layers that one is nested in, so a press
  outside a submenu closes the whole menu). A layer that blocks everything
  still lets the pointer reach the blocking layers it is nested in, so the
  rows of a menu stay hovered and pressable under its open submenu, and a
  press on one closes the submenu. Which layers the pointer can reach is one
  rule (`Layers::reachable`) that the hit test and the forwarding of input to
  a forwarding catcher share. A mouse asks on the press; a finger
  on the tap, so a scroll that starts outside a layer leaves it open.

Inside the layer it is sent to, a request goes to the most recently made
enabled `on_back` handler in that layer whose component is in the tree (not in hidden
`ShowKeepAlive` content or a closed overlay) first, and to `on_dismiss` only
when there is none, so Escape, back and a press outside all play the same slide-away.
An overlay closed because the one it is nested in closed is told so after the
fact, as `Dismiss::WithParent`. Its owner writing `open` false
(`Document::close_overlay`) runs nothing, so a prompt whose dismiss means
"cancel" needs no guard against its own Keep button.

Every layer gives the focus back when it closes, however it closes (its owner
writing `open` false, a parent closing, or its node being removed while open):
a layer remembers the node that had the focus when it opened, or the last one
focused outside it since, and refocuses that node if the focus is inside the
layer when it closes, or is nowhere and the layer traps focus, and the node is
still there to focus. A close that has moved the focus somewhere outside the
layer on purpose keeps it there, and so does a focus change that comes after
the close. Components rely on this rather than refocusing their triggers
themselves, so a select, the command palette and a dialog put the focus back on
what opened them, and a forwarding catcher (a plugin's region, a Wayland
window) gets the keyboard back.

What a layer remembers is a focus scope, and the document keeps one stack of
them (`focus_scope.rs`): an open layer pushes one and closing pops it.
Anything else that takes the focus for a while and gives it back pushes its
own with `Document::push_focus_scope(holder)` and pops it with
`pop_focus_scope(holder)`, which refocuses what had the focus only if the focus
is still inside `holder`; block-app's region does that when a plugin asks for
the keyboard.

Focus asked for beneath the topmost layer that traps it never lands there, so
nothing beneath hears it gained the focus. It is remembered until the next
input event: if the layer closes first, the focus goes there rather than back
to what the layer remembered, which is how a dialog's button can close the
dialog and focus something outside it in either order.

The focus is checked when something that could take it away changes - a layer
opening or closing, a frame hiding, a node being removed or moved - by walking
up from the focused node, never by searching the tree; a node whose parent
link is gone is outside the tree.

`on_back(enabled, handler)` is a hook, like `on_key`: called in a component, it
hangs a handler on that component's node (on the root outside one) for as long
as the scope that called it lives. It only routes: the handler hears every
phase of the back gesture, a back with no gesture before it (a key, Escape, a
press outside) as `BackGesture::Invoked` alone, and moves nothing. The motion lives above it. `unstyled::BackSlide` wraps a
page: held, the page follows the finger across a good share of the screen;
let go, it carries on off the edge before `on_back` runs and the next page
slides in behind it, and a cancelled gesture eases it back. Given `behind`, a
function that builds the page back goes to, it builds that page under the
moving one for as long as the gesture lasts, shaded and coming in from the
side under the `shade` colour (transparent unless given), so `on_back` swaps in what was already showing and nothing slides in
afterwards; the page that moves needs an opaque background, and a page that
must keep its state (a scroll position) is built once and handed to a `Portal`
in both places. The stacked dock shows the tab back goes to behind the tab it leaves
this way. A back with no gesture before it (a key, Escape, a press outside)
carries the page off from where it rests in the same slide-away. Given `open`, a back
only asks: `on_back` runs at once, the page eases back if it was held, and it slides away
whenever `open` falls, calling `on_gone` once it is off. `unstyled::Modal`, under
`styled::Dialog` and `Fullscreen`, works that way: Escape, back and a press on the scrim
reach `on_dismiss` at once, and the modal slides away and fades its scrim only once its
owner closes it, so an owner that asks first ("discard changes?") or a plugin that
answers later keeps it in place meanwhile. While it leaves it lingers inert, taking no
input. A `Sheet` sinks with the gesture and slides on down from where it was. On Android the
runner passes `handles_back` to the activity's `setBackHandled`, and when
nothing takes it the system's own back (to the home screen) plays instead.

A `Center` overlay is never larger than its window: content that measures taller
or wider is laid out in the window's size, so its bottom cannot fall off a short
window.

`unstyled::Modal` is every modal's outer part: an overlay with a scrim that fades
with the modal's presence, and dismissal (Escape, a press on the scrim, back) through
`on_dismiss`. `placement` is `ModalPlacement::Center` or `Fill` (the whole window). `motion` is
`ModalMotion::Slides` (the content goes in a `BackSlide`), `Still`, or `Reported`, where
the content reports its own presence through the `ModalHandle` it is given, as
`ModalSheet`'s sheet does. `styled::Modal` picks the scrim by `Scrim` (`Soft`, `Dim`,
`Strong`, `Opaque`), and `Fullscreen` is one that fills the window over `Scrim::Strong`.

A dialog's buttons are data, never buttons in its body: `actions` is a list of
`DialogAction`s, each a label, an `ActionRole` (`Primary`, `Destructive`,
`Secondary`, `Cancel`), and `disabled`, `busy` and `shown` (`DialogAction::primary(label,
on_click).disabled(blank)`, `DialogAction::cancel(label)`). The dialog places,
orders, draws, focuses and binds them (`unstyled::DialogBody`, painted by
`styled::DialogBody`), so every modal lays its buttons out the same way. Cancel runs
the dialog's `on_dismiss`, the same as Escape, back, a press on the scrim and the
close button. There are two kinds. A `DialogKind::Alert` is a decision (leave,
discard, delete): its actions are its only way out, its role is AlertDialog, and the
focus starts on its primary action, or on Cancel when that action is destructive. A
`DialogKind::Dialog` (the default) is content (settings, members, inspect): a title
row with a close button, and a footer only when it has actions. Enter that nothing in
the dialog takes runs the `Primary` action unless it is disabled or busy, so a field
needs no `on_submit` of its own; a `Destructive` one never runs on Enter. A wide
footer is right-aligned with `Secondary` actions (and a `Destructive` one beside a
`Primary` one) on the left, then Cancel, then the primary (macOS and GNOME's order),
and it wraps rather than stacking. A narrow or touched one stacks its buttons full width,
primary first. `busy` disables an action and shows progress in its button
(`Button`'s `busy`). With `id`, the buttons are `{id}.{name}`: `confirm` for the
primary or destructive action, `cancel`, and what `named` gives a secondary one;
the close button is `{id}.close` and the footer `{id}.actions`. A confirmation
inside a dialog is an alert of its own, opened over it; a control or row action
(a member's role, share's Add) stays in the content.

`styled::Dialog` is the two parts together: `DialogFrame`, the outer part, and
`DialogBody`, the inner part. `DialogFrame` (on `unstyled::Dialog`) is a centred
`DialogCard` in a `Modal`, or, for a `Dialog` kind when its window is
narrower than `SHEET_BELOW` or the press that opened it was a finger's
(`touched_by_finger()`, the same rule as `Anchored`'s), a fitted
`ModalSheet`; an alert is always a card. It builds its content once, when it first
opens, and moves it between the card and the sheet with a `Portal` if that changes
while it is open; it provides the `DialogLayout` (`narrow`, `sheet`, `bottom_inset`)
to what it holds. `DialogBody` is the title row, the content in a fitted `Scroll` with
`Bleed::dialog` that gives up length when the screen is short (or, with
`scrolls=false`, the content given the rest of the height to scroll itself, reaching
the card's sides with a `Scroll` or `Tree` whose `bleed` is `Bleed::DIALOG`), and the
pinned footer; it reads `narrow` and `bottom_inset` (how much of its bottom is covered,
which its padding keeps the footer above) from the `DialogLayout` it is in. Code that places
its own body uses the parts: block-app's keep-changes prompt is a `DialogCard` with a
`DialogBody` in a centred `Modal`, the dock's Rename card and the
block picker's stages are a `DialogBody` in a `DockDialog` (which provides a
`DialogLayout` that is narrow while the dock is stacked and the card a sheet), and a
plugin opened as a dialog draws a `DialogBody` that block-editor-beui hands a
`DialogLayout` from its frame's `dialog`. A sheet is fitted to the screen beui's on-screen
keyboard leaves, so its footer stays above it.

`DialogBody` handles Enter and its own close button and Cancel; Escape, back and a
press on the scrim reach whoever owns the outer part. An outer part that is not in
the same document provides a `DialogDismissal`, which the `DialogBody` inside
claims while it lives: `handled()` says whether one did and `dismiss()` runs its
`on_dismiss`, so a form with unsaved changes can ask before closing. block-editor-beui
provides one in a plugin dialog and forwards the host's dismissal, Escape and back
to it.

A layer at `LayerLevel::Lock` that blocks everything is a lock screen
(block-app's `LockScreen`): it is above every other layer, so anything opened while
it is up opens beneath it; it is never dismissable; the focus cannot leave it,
not even to nothing; `Document::locked` is true; and while it is, the
listeners above the tree (`on_global_key`, window and host actions) hear
nothing but media keys (`Key::is_media`), and a forwarding catcher beneath it
loses any press it had captured, as one beneath any layer that blocks
everything does.

`unstyled::Floating` pins its child to the top or the bottom edge of a node
named by `NodeRef`, taking no space in the layout around it:

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
for `capture_presses` instead. The press stops at that button, so the row it
sits in does not open as well.

A press made with modifiers held can be claimed from everything inside a
region, which is how a window manager takes Super+drag from the windows it
holds. An `Interactive` given `claims` (a list of `Claim`s, each modifiers and
an `Area`) takes every primary or secondary press in a claim's area while all
of its modifiers are held; a claim with no modifiers takes every press there.
The claim is searched for from the top of the tree
down, before `capture_presses`, so the outermost claimant wins; the press, its
drag and its release reach the claimant alone, none of its descendants, and no
forwarding catcher under it unless the claimant is that catcher.
`Document::press_claims` lists the rectangles of the claims that need
modifiers, laid out this frame, which a plugin reports to its host. A Wayland program's window is a
forwarding catcher like a plugin's region, so a press claimed over it never
reaches the program. `on_press_outside` hears every primary press outside the
`Interactive`'s visible rectangle, wherever it goes and whatever takes it, at
the same moment and from the same place a press outside asks layers to close
(a mouse's press, a finger's tap; one call, `Document::pressed_outside`, does
both, and a long press counts as a secondary press, which asks the layers but
not these), which is how the host hands an embedded editor's active
child back when someone clicks elsewhere. A secondary drag stays with the node that
took it while the pointer crosses a floating window above that node; only the
start of one is hidden from what a floating window covers.

A canvas a pen draws on is an `Interactive` with `drawing` (an `Area`: `true`
for all of it, or a `HitRegion` of it). While the `canvas_touch` preference is `CanvasTouch::Off`, a finger that
lands on one reaches nothing at all: no press, drag, long press, scroll or pinch,
for that node or any around it, until it lifts. A pen, a mouse and a touchpad
are untouched, and so is a finger on anything the drawing surface does not
hold, its floating chrome among them. It is a switch the person turns rather
than palm rejection, which guesses. `Document::drawing_rects` lists the
`drawing` rectangles, which a plugin reports to its host (`FrameReport::drawing`)
so the host keeps the finger from the plugin, and from its own pan and zoom, too.

`unstyled::Tree` makes the same split the other way round. A row is a tab stop
with the tree's keyboard and its `TreeItem` accessibility, and nothing more:
where a pointer has to land to select it is the face's business, because a
face that draws a chevron of its own outside the name wants pressing the
chevron, the indent beside it and the name to mean three different things. The
handle carries `select`, `toggle` and `hover` for the face to call from
wherever it decides they belong. The arrow keys, Home, End and typing move the
keyboard between rows without selecting them, and Enter or Space selects the
row it is on. `toggle` puts the keyboard on its row too. Given `on_open`, Enter
and a second click select the row and report `on_open` too, for a tree whose
rows are chosen as well as selected (the block to link).

`styled::Tree` is the face that split was made for, and the one app code
reaches for. Everything left of a row's highlight - the indent and the chevron
- is one button of its own, tooltipped and labelled for a screen reader, so a
finger anywhere left of the name expands the row without opening it, and
pressing the highlighted row beside it selects. The content builder gets a
`TreeRowFace` carrying the row's `key`, `item`, `selected`, `focused` and
`hovered`. Given `menu`, which answers a key with a `TreeMenu` (its items, each
acting through its own `on_click`) or `None`, every row with one shows
a "…" button at its end, on a desktop too, and the same menu opens from it, a
right click or a long press: a dropdown under a mouse and a sheet under a finger.
Row menus are `unstyled::Tree`'s: given `menu`, a `TreeMenus::new(style, menu)`
pairing the `MenuStyle` with the function, it wraps each row that has one in a `ContextMenu` and hands the face a
`TreeRowMenu` in the handle's `menu`, which `unstyled::TreeRowMenuButton` turns
into a menu button anchored to itself, so the dropdown hangs from the "…"
button.
Row height follows `use_density()`: `Density::Compact` rows for a desktop and
`Density::Comfortable` ones, tall enough to tap, where `provide_density` asks for
them, which a plugin's frame does when its host says the window is a phone's. It owns the drag gesture
too: a press that travels further than a few pixels reports `on_drag_start`
and the click it would otherwise have become is dropped, so a row can be
dragged out without selecting what it left behind. `row_test_id` names each
row for tests, as `<id>.row`, `<id>.chevron` and `<id>.more`, and `outline` gives a single
row an outline of its own, which is how a drop target says whether it will
take what is over it.

A row's name is edited in place through the controlled `editing` prop, the
key being renamed or `None`. While it names a row, the row's face gives way
to a field of the row's height holding `TreeItem::label`, focused with all of
it selected and scrolled into view, named `<id>.rename` for tests. Enter and
losing the focus, to a press elsewhere or to the window losing the keyboard,
report `on_rename` with the trimmed name; Escape, or a name left empty or
unchanged, reports `on_cancel_edit`; both hand the keyboard back to the row
after Enter or Escape, and the caller clears `editing` in either. F2 on a row
asks for an edit with `on_edit`, when one is given. Keys typed in the field
never reach the tree's own navigation. `unstyled::TreeRowEditor` is the field
a face places for a row whose handle reads `editing`, and owns all of that.

Choosing a row never expands, collapses or scrolls anything: only the chevron
and the arrow keys do. The styled tree owns its `Scroll` (`padding` goes inside
it), and follows the `selected` key wherever it is instead. Given `ancestors`,
which answers with the keys above one, outermost first, it marks the chevron of
the deepest row still shown above a selection hidden inside a collapsed one, and
while the selection is hidden or scrolled out of view it floats a button over
the top or bottom edge that reports `on_reveal` - the caller expands the
ancestors - and then scrolls the row into view once it is laid out. The file
tree and the beui inspector both work this way, and both hold the tree's
`TreeApi` through its `api` prop to find its rows (`row_node`) and its focus
(`focused`).

### Overlays stay in their window

A document knows nothing of the monitors it is shown on. Every overlay is laid
out within the window it was opened in (the main window's rect, or a hosted
window's own `(0, 0)`-cornered rect less any on-screen keyboard there):
`Center` and `Fill` take that whole area, one anchored to a node, a point or a
rect (`OverlayAnchor::Rect`, in that window's coordinates) is placed against it
and kept inside the window. Where an app shows one window per monitor (block-app
under `--session`, one per display), each window is a monitor, so a dialog opened
in it stays on it. `use_viewport_size()` is the size of the main window a
document is shown in (a `Dialog` reads it, or its hosted window's size, to become
a sheet on a narrow one). `styled::Toasts` takes any anchor this way, so the desktop plugin stacks its toasts in the
corner of a display above its bar. A toast goes after its `duration` unless it is `sticky` or
`danger`: an error stays until it is dismissed, so it cannot be missed. All of that is
`unstyled::Toasts`: the overlay at an `edge` that lets input through, the stack, each
toast's timer on the frame clock, and a live region (a `Log`, with each toast a `Status`,
or an assertive `Alert` when it is `danger`); the styled one passes its width, margin and
spacing and draws each card from a `ToastHandle` (the `Toast`, `dismiss`, `activate` and
`act`).
block-app's keep-changes prompt and lock screen are one card in a `Modal` that
fills (or centres in) its window; block-app puts one in each display's window.
The desktop plugin's `LevelOsd` is one card in an inert `Fill` overlay, shown in each
display's region.

### Docking and windows

`dock::styled::Docking` (crate `beui-dock`) is the workspace layout: panes split from one another, a tab
bar on each pane, and tabs that can be dragged between panes or out into
windows that float over the rest of the dock. `dock::unstyled::Docking` underneath it
owns the tree, the dragging and the keyboard, and paints nothing; the demo in
`crates/beui-demo` is laid out in one.

The children describe the layout the dock starts with and the tabs that exist;
a `DockingLayout<K>` holds where everything is now, and is what the caller saves
and restores:

```rust
let layout = DockingLayout::new();
view! {
    <Docking layout>
        <DockSplit id="root" direction=Direction::Horizontal fraction=0.25>
            <DockPane id="files">
                <DockTab id=Key::Files title="Files"><FilesPanel /></DockTab>
            </DockPane>
            <DockPane id="editors" empty={move || view! { <Nothing /> }}>
                <ForEach keys={open}>|key: Key|
                    <DockTab id=key title={title_of(key)} on_close={move || close(key)}>
                        <Panel key />
                    </DockTab>
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
`pinned` to keep its tabs in it), names its starting `active` tab, can start
with its tabs in a sidebar (`vertical`, `sidebar_width`), and can be `locked` to
keep its tabs in its area (below). `DockWindow` floats one
tree at a `rect`. A `see_through` tab shows something drawn under the dock, as the inspector's App tab shows the
app: neither its pane nor a stacked dock fills behind it, so the pane's chrome learns it from
`DockChromeHandle::see_through`, and it stays on the main surface, where nothing of the dock is
under it (`DockState::admits` refuses it a window). A tab's key is any `Clone + Eq + Hash` type; a key that is a
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

`DockingLayout` is a `Copy` handle owned by the scope that made it;
`OwnedDockingLayout::new()` makes one outside a scope (a host, or a test before
it builds) and derefs to it.

`DockingLayout::snapshot` is the `DockingSnapshot` to save, and `restore` puts
one back; restore it before the effects of the write that declares its tabs run (any
writes made together do, with no `flush()` between), since a tab not
declared when the dock reconciles is closed. `reset` goes back to the layout the
children describe. The handle also shows a tab (`show`) and reads which is
focused (`focused`, `shown` for a stacked dock). A tab that stops being declared
for a moment - a `Show` around it that flips, a `ForEach` rebuilt from new keys -
loses its place, so keep a tab's declaration in one stable place.

A tab's key names the tab, not what it shows, so a tab that shows something
else keeps its key, and with it its place, its focus and its history. Master
and detail views, a list that opens what it names into tabs, as the demo's
catalog and be-launcher's pull requests do, hold their tabs in a
`DockDocuments<K, T, I>`: slots of content `T` that it keys with
`new(layout, key, identity)`, `key` turning a `DockSlot` into the layout's key and
`identity` telling one document from another. `open(content, beside)` shows the
document if it is open; otherwise it puts it in the slot shown last, unless
`beside` (Ctrl or Cmd held) asks for a slot of its own. It answers
`DockOpened::Shown`, `Replaced` with what the slot held, or `Added`.
`refresh(content)` updates an open document in place, `shown` and `find` name
slots, and `close` empties one. `DockDocumentTabs` declares a tab for each slot,
handing the caller a `DockDocument` to make the `DockTab` from, and
`DockDocumentBody` inside it rebuilds its body only when the slot's document
changes, so new content for the same document keeps the body's state:

```rust
<DockDocumentTabs documents>|document: DockDocument<Tab, PullRequest, u64>|
    <DockTab id={document.key()} title={title_of(document.content())} on_close={move || document.close()}>
        <DockDocumentBody document>|pull_request: ReadSignal<PullRequest>|
            <Detail pull_request />
        </DockDocumentBody>
    </DockTab>
</DockDocumentTabs>
```

The `DockState` the layout holds is a plain value underneath:
`state()` reads it, and `find`, `all_tabs`, `focused_tab` and `surface_tabs`
answer from it.

Tiled, the styled `Docking` lays its panes edge to edge, divided by a one-point line
(`splitter_thickness`) that takes presses a few points either side of it (`splitter_reach`);
only floating windows and cards keep a border, rounded corners and a focus outline.
A dock narrower than `NARROW_WIDTH` stacks: it draws the same state as one screen, the focused tab
filling the dock, edge to edge, with no tab bars, splitters or windows, and everything else in
the state is kept, so widening it tiles the layout again. The dock picks the mode
from its own width (the unstyled `Docking` stacks below its `stack_below`, which the styled one sets to `NARROW_WIDTH`), and
`DockingLayout::mode()` reads it, which is how the workspace knows it is on a phone.
The dock draws the stacked screen's bar itself: the tab's icon and title, a
square counting the open tabs that can be closed, the one on show included, always shown
(so it reads 0 while only tabs like Files are open), which opens a sheet to show or close them
(a card for each tab with a close button, from the one shown last, and below
them a row for each tab without one, such as the workspace's Files and
Settings, in layout order), and a back button while `DockState::back` has somewhere to go, which
the back gesture also follows. A tab's icon comes from the optional `icon` function, an icon-font
glyph (empty for none) that the tab bars, the drag preview and the stacked bar
all draw. The bar is the dock's, not the caller's; what a
tab adds to it is only its menu, which the panel hands over with
`dock_menu(items)`, a `Memo<Vec<MenuNode>>`.
The menu is not tied to the stacked bar: a tiled pane offers the menu of the tab
it shows behind a More button just before its close button, at the end of its
tab bar or at the top of its sidebar, and so does a window. An action built with
`.pin(Pin::Icon)` or `.pin(Pin::Label)` leaves the More menu and sits in the bar
beside it. A tiled tab bar shows the icon pins, a divider, then the label pins as
labelled buttons (the workspace's undo and redo, then Share); a sidebar shows every
pin as an icon; the stacked bar shows only the icon pins and leaves the label pins
in its menu. An action built with `.pin(Pin::Title)` belongs to the tab's name: the
stacked bar draws the name as a button with a chevron after it that runs the
action (the workspace's Rename), and leaves it out of its menu, while tiled bars
keep it in theirs. Splitting a menu between the bar and More is
`unstyled::ActionBar`'s, which any toolbar can use: given the `menu`, whether
label pins fit (`labels`) and whether the caller shows the title pin itself
(`title`, then `unstyled::title_action` finds it), its handle holds the `icons`
and `labels` to pin and the `overflow` for the More menu. `styled::ActionBar`
draws it, with `ActionLabels::Shown`, `Icons` or `Overflow` for the label pins,
a `direction` and a button `size` (a pinned action built with `.checked(..)` is a toggle that looks pressed while it is on), and names its parts `<test_prefix>.pin.<id>`
and `<test_prefix>.menu`; the dock's bars are styled action bars. A tiled bar is 40 points with its rule, its tabs 30; its controls are
`ButtonSize::Toolbar` icon-only buttons (30 square) and the labelled pins compact
`Button`s of the same height, while a tab's own close button stays compact. The
stacked bar keeps regular-sized buttons. Pinning is how a tab puts its few most used controls in reach without
giving the dock a node; everything else stays in the menu, and the tab keeps the
rest of its page for its own toolbar. A menu is a list of
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

Every change of the focused tab is a `DockVisit` in `DockState::history`,
saved with the layout: the tab it came from, the tab it went to, and whether
that tab was `opened` by it, which is its first time shown since it joined the
dock. `back` is the last visit to the tab on show. Going back undoes it: a tab
the visit opened is closed through its `on_close` when it has one, and closing
the tab on show, in either mode, shows the tab its last visit came from;
otherwise `go_back` shows that tab and drops the visit and every one after it.
A tab that leaves the dock hands the visits that left it to the tab it was
reached from.

The dock's switcher walks `recent_tabs` across every pane and window, the way
Alt+Tab walks a desktop's windows. `DockingLayout::begin_switch(backwards)`
chooses the tab shown before the current one (or, backwards, the one shown
longest ago), `step_switch` moves the choice along and wraps, `commit_switch`
shows the chosen tab (activating it in its pane and raising its window) and
`cancel_switch` leaves everything as it was; `switching` and `switch_choice`
read it back, and `DockState` has the same calls and `switch()`. While a switch
lasts, the dock opens an overlay in the middle of the document listing the
tabs, each with its icon and title and the chosen one highlighted, drawn by the
`switch` render prop (`DockSwitchHandle`, a row per tab whose `pick` commits
that tab); nothing is shown until the switch is committed. A switch is not part
of the saved layout. Escape cancels it, with whatever modifiers are held and
wherever the focus is (a program's window included): the overlay holds no focus
for Escape to find it by, so the dock listens for Escape at the window while it
is open and asks the overlay to close (`Document::dismiss_overlay`), as a press
outside or back does. A dock in a plugin only hears a key a program has the
keyboard for when it intercepts the chord, so the desktop plugin also registers
Alt+Escape as an intercepting action that cancels. The dock binds no other key: whoever binds it decides the
keys, usually a chord that begins or steps the switch while a modifier is held,
and `held_modifiers()` letting go of that modifier to commit,
as the desktop plugin does with Alt+Tab and the demo with Alt+Q.

Each surface - the main one and one per window - lays its tree out over the
rectangle it was given, so panes and the bars between them are canvas items at
computed rectangles rather than nested boxes. `layout_surface` is that
calculation on its own, which is what the drop targets and the tests are
resolved against. A window is a floating overlay, so it paints above the dock
and takes the pointer where it actually is, and the document underneath it is
shut out rather than answering a press through it.

A dock window is a `Window` (see "Windows and window hosts") sized to its rect
plus the reach of its resize grips, which lie around it. Where nothing opens
windows it is that `Window`'s overlay and the dock moves, clamps, springs back
and resizes it itself. Where a host opens windows each dock window is a real
window (space's panels, or block-app's windows on the desktop for the desktop plugin's
dock, guides/desktop.md, Displays): a press on its bar calls `begin_move()` and a grip `begin_resize(edge)`
(as does a secondary drag with `drag_modifier`, from the nearest edge or
corner), so the host moves and resizes it. A size set from inside, such as
`DockTabControl::resize_window`, changes the rect and so the `Window`'s `size`,
which asks the host with `WindowRequest::Resize`; either way the dock writes the size of the host's
`Resized` reports into the window's rect (less the grips) and keeps each
`Placed` report's `HostPlacement` in memory, handing it back as
`WindowPlacement::Host` when the window opens again (after the dock was
stacked, say). A window opened with no placement kept (floated from a tab's
menu, or dropped on the main window where no pane takes it) opens `Over` the
window the dock is in, at the corner it would be drawn at there, grips
included, so a host can put it where it sits in the dock. Each window holds its own drop target, and drop targets are resolved
in the window the `DragPoint` names, in that window's coordinates
(`stacked_at` takes the window), so a tab dragged from one window onto
another's bar or pane lands there. A tab or pane dropped where no window is (a
`Draggable`'s `on_drop_outside`, below) floats into a new window opened
`AtPointer`; without a host that drop keeps its old meaning.

How the windows stack is the document's layer order alone (`Document::layers()`),
not anything in `DockState`: a press on a window, the focus moving into one and
showing a tab of the window that has the focus (`DockState::show`, so a host can bring
back a window something else covered) raise its layer (over a fullscreen tab too, unless
the focus is in that tab), and a drop
target is looked for in the window whose layer is on top at the pointer, never
in one covered there by a layer that is not the dock's (a menu, a dialog, the
fullscreen tab).

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
at its top. The grip's menu also closes every tab in the pane, offers the
menu of the tab the pane shows below its own rows, and turns on icons only (`set_icons_only`, or
`icons_only` on a `DockPane`): `DockTabHandle::icons_only` asks the tab face for
its icon alone, which the styled dock draws as a square with the title in a
tooltip (the title's first letter where a tab has no icon); a sidebar of
icons moves its More menu and close button below its tabs, in a column. A sidebar's edge
goes by the way the pointer moves as well as where it is
(`DockState::drag_sidebar`): dragged left past `MIN_SIDEBAR_WIDTH` it turns icons
only on at once, and while icons only, any move to the right turns it off at
`MIN_SIDEBAR_WIDTH`, growing past that only once the pointer passes that edge. The drag
carries on either way, and the sidebar slides to its new width (the dock's
`icon_sidebar_width`, or the width under the pointer), following the pointer
until it catches up. Windows resize from any of
their eight grips and are raised by whatever takes the focus inside them. The grips lie
wholly outside the window, in a ring `HANDLE_REACH` points wide (a window's overlay is that
much larger than the window on every side), with each corner's grip running `GRIP_CORNER`
points along both edges, and the bar between two panes takes presses only in the gap it
draws: neither ever reaches over a pane's or a window's content, which gets every press
right up to its edge (a program's window included). A
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

A `locked` pane makes an area: the pane and every pane split off it, including
the groups inside them (`area_of(leaf)` names a leaf's area, the locked pane's
`id`). Every tab declared in the pane, including tabs a `ForEach` adds later, is
locked to that area, as a pinned group's tabs are to it: it can move, split and
group anywhere inside the area, which grows with its splits and shrinks back as
they empty, but a drop outside it is refused and it cannot float. Tabs that are
not locked go anywhere, into an area too. `DockState::home(tab)` answers a
`DockHome`: `Group(id)` for a pinned group's tab, `Area(id)` for a locked one.
The tab menu's pin item reads "Unlock" and "Lock" for such a tab (Lock is
disabled until the tab is back in its area, and is never done for it), and
"Pop out into a window" is disabled while a tab is locked or pinned. Seeding,
reconciling and `restore` move any locked tab found outside its area back into
it. A stacked dock ignores locks and keeps them for when it tiles again.

A tab's panel is built the first time the tab is shown and belongs to the dock
rather than to the pane showing it: the pane holds a `Portal` pointed at it, so
the panel keeps its nodes, its scroll position, its caret and its state when
the tab is hidden behind another, dragged to another pane, or floated into a
window. A panel no pane is showing is laid out by nobody, so it costs nothing
and a screen reader does not read it. Closing the tab is what removes it.

Pressing a pane's or a window's own chrome - its bar, a tab, a grip - gives the
keyboard to the tab it shows rather than taking it: the bar is an `Interactive`
with `press_focus=false` (guides/beui_keyboard.md), and the dock (`beui-dock/src/unstyled/keyboard.rs`) puts the focus in the
panel, where it was when that tab last had it or else on the panel's first
focusable (`Document::focus_into`, which waits a layout for a panel still being
built). Picking a tab with the pointer does the same for the tab it shows, while
the arrow keys leave the focus in the tab bar. When a pane stops showing a tab
whose panel has the focus, the dock remembers where it was and gives the
keyboard to the tab shown in its place. Each time the dock gives a tab the
keyboard, `DockTabControl::keyboard` changes, so content whose keyboard lives
in another document, such as a program's window, can take it there.

`drag_modifier` (none unless asked for) makes the dock a window manager while
those modifiers are held. A primary drag anywhere in a floating window - its
content, its bar or its tabs - moves the whole window, as dragging its bar does;
a secondary drag resizes it from the edge or corner nearest the pointer (by
thirds of the window). On a docked pane a secondary drag moves the split nearest
the pointer, and a primary drag does nothing: it is swallowed rather than handed
to the content, since a press made with the modifier is never meant for it.
Each window and each docked pane claims those presses (a `Claim` with those modifiers), so
neither the tabs nor the content see them. The code is
`beui-dock/src/unstyled/modifier_drag.rs`; the desktop plugin sets Super, and the demo's Docking page
has a switch for Alt.

A `DockDialog`, declared beside the dock's other children, is a window attached
to a tab: `<DockDialog id parent title size modal on_close>content</DockDialog>`
hangs over the tab named by `parent` (or over another dialog, so dialogs of
dialogs stack), centred on the parent's panel at the `size` its content asks
for and kept on the parent's screen (`Placement::CenterOn`), in the
`dialog_chrome` render prop (styled: a bordered card with a title bar and a
close button, `dock.dialog.<tab>` naming its body). It is not part of the
`DockState`: it is never a tab, nothing drags it into the dock, a key declared
as both a dialog and a tab is only a dialog, and it is not saved with the
layout. It shows exactly while its parent tab does (`DockState::is_shown`),
so it hides when another tab takes the parent's place and returns with it,
and it rises with the window its parent is in, since its layer hangs from the
parent's panel and so is a child of that window's layer. A new dialog takes the focus. The
dock tracks which dialog has it beside the focused tab: pressing a dialog
gives it the focus, pressing the parent's pane or tab bar takes it back, and
when a dialog closes the focus returns to its parent (the overlay's focus
restore). The content's `use_dock_tab()` is a `DockTabControl` for the
dialog, whose `focused` follows that and whose `show` focuses the dialog.
`modal` makes the dialog's layer `BlocksParent` until it closes or stops being
modal: a press on the parent's panel is shut out and asks the dialog to close,
which it answers by focusing the innermost modal dialog and flashing it (the
handle's `attention`, and `DockTabControl::attention` for content in another
document, such as a program's window, to take the keyboard itself); the focus
cannot move into the panel (Tab skips it, and focusing it lands in the
dialog); and while the parent's tab is focused the focus goes to its innermost
modal dialog. Escape or back with the focus in a dialog closes it through
`on_close`. While a tab has a modal dialog hanging from it (or from one of its
dialogs), its label shows a dot (`DockTabHandle::waiting`, styled
`dock.tab.<tab>.waiting`): the tab is waiting on a choice, which matters most
once another tab is shown in its place and hides the dialog.

`presentation=DockDialogPresentation::Card` (rather than the default `Window`)
shows the same dialog as a card floating from its tab: always modal, it is
centred on the parent's panel a small gap under its tab bar
(`Placement::HangFrom`, Docking's `dialog_margin`) and keeps the `size` it asks
for even where the pane is narrower, reaching over the panes beside it; it is
only kept that margin inside the window, and shrinks only where the window is
too small (the body is `Extent::at_most(size)`): in a short window it shrinks
under its tab bar first, and rises over the bar, hiding the tab's waiting dot,
only when even its smallest size does not fit below it. Its scrim (Docking's
`dialog_scrim`) dims that panel alone: a `BlocksParent` overlay with a visible
scrim lays it over the node it blocks. A `BlocksParent` layer occludes the node
it blocks and its content (`Document::layer_covers`), dimmed or not, so the card
draws over other panes and takes the presses it covers there, a plugin's host
routes presses on the blocked panel to the dock rather than to an editor shown
in it, and the rest of those panes, the tab bar and other tabs stay live. A
press on the blocked panel calls the card's attention, or
closes it through `on_close` with `dismiss_on_outside_press` (any modal
`DockDialog` takes it; a press on another pane never closes one). It has no
title bar: its content draws its own header and close button (styled: the
dialog card's raised surface, `Radius::Lg` on every corner, the dialog outline
that shows focus and attention, kept `dialog::MARGIN`
from the tab bar and the window's edges). Its layer hangs from the panel, so it
follows the tab into another pane or a window.

Given an `anchor`, the rect in document coordinates of the control that asked
for it, a card is a popover instead: it hangs `Placement::Popover` from that rect
pinned to the parent's panel (`OverlayAnchor::Pinned`), Docking's `dialog_gap`
off it and `dialog_margin` inside the window, so it keeps its place on the tab
when the pane moves or resizes and follows the tab into another pane or window.
It is not dimmed: the panel is blocked all the same, so a press on it closes the
card or calls its attention as above. The chrome names the frame to point from
with the handle's `pointer` (styled: the card's frame, whose beak points at the
control's middle from the side the card opened on). Everything else - modal to
its tab, hidden with it, the waiting dot, the keyboard, the sheet in a stacked
dock - is the card's. In a stacked dock it is
`dialog_sheet` instead (styled: a `ModalSheet`), whose body fills the sheet's
width as every sheet's does rather than keeping the card's `size`, and the same
body moves between the two through `Portal`s, so nothing in it is rebuilt. A dialog's
body is built once when it is declared and kept while its tab is hidden,
whichever presentation it has.

One tab at a time can be fullscreen: `DockState::set_fullscreen` with a
`DockFullscreen` naming the tab and an area (a rect in document coordinates, or
`None` for the screen it opens on), `DockingLayout::enter_fullscreen` and
`leave_fullscreen` from outside, or the `DockTabControl` that `use_dock_tab()`
hands the tab's own content. The panel moves into a floating overlay over that
area, wrapped in Docking's `fullscreen` render prop (styled: the theme's
background), so it covers the tab bars, the other panes, the dock's windows
(even one the focus moves into) and anything else in the document there, and nothing under it hears the pointer, while the rest of the
document stays live. Leaving puts the panel back in its pane without rebuilding
it. The dock binds no key to leave: whoever enters fullscreen offers the way out,
as the demo's Escape does. Fullscreen is not part of the saved layout.
Showing or focusing the fullscreen tab (`DockingLayout::show`, the switcher, a press anywhere on
its content) raises the fullscreen overlay above the floating windows, its own pane's window
included, so a window brought in front of it and then left goes behind it again. The pane
it left draws Docking's `away` render prop (styled: a note that it is full screen and a Leave
full screen button), seen where the fullscreen area does not cover it, such as another screen.
A tab's content calls `DockTabControl::offer_fullscreen` to add Full screen / Leave full screen
to the tab's right-click menu; only content that has its own way out should.
`DockTabControl::show_notice(notice)`, a `Memo<Option<String>>` kept until the content's scope
ends, is what the content needs said about it right now: while it is `Some`, the tab, the
dialog's header and the stacked bar show it in place of the title, in the accent colour on an
accent-soft fill (`DockTabHandle::notice`, `DockDialogHandle::notice`, `DockStackHandle::notice`).
The demo's Docking page has a sample.

### Progress

`styled::Progress` is a progress bar over `unstyled::Progress`. `value` is
`Some(fraction)` for progress that is known and `None` (the default) for
progress that is not: then the accessibility node is busy and a segment
sweeps along the bar, read from the frame clock through the handle's `phase`.
It fills its width unless given one; `INLINE_PROGRESS_WIDTH` is the width for
one beside a label or a button. The sweep runs only while the bar is laid out
and motion is animated: an indeterminate progress behind a `Show` that is
false, in a tab nobody is looking at, or under reduced motion asks for no
frames and rests in the middle. It uses `component_placed` (`measure(node)` being `Some` for a node), which works for any
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
    >|handle: DragHandle|
        <CardFace card_id dragging={handle.dragging} />
    </Draggable>
    <DropTarget on_drop={move |(card, _): (CardId, DragPoint)| move_card(card, column)}>|handle: DropHandle|
        <ColumnFace column over={handle.over} />
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
that nest - a card inside the pile that holds it - set `capture_presses`, so a
press reaches the innermost one and not the one around it, the way
`unstyled::Button` wins a press; of overlapping ones only the topmost hears it.

While a drag is under way the `preview` is shown beside the pointer in an
inert overlay at `LayerLevel::Top`, so it paints above every window and takes
no input. Where windows open (`Context::opens_windows`), it is shown in the
window the pointer is over (the overlay's `shown_in`, the board's `DragPoint::window`),
so a drag carried from one window into another, a tab between space's panels
say, shows its preview under the pointer there. The pointer
it follows is the one the document saw, not the one the draggable's own
catcher saw: the board is fed the pointer at the start of every frame, even
where a floating overlay covers the source, so a drag carried over a floating
window keeps going. A finger drag waits for `TOUCH_DRAG_THRESHOLD` and for the
catcher to hold it rather than a scroll, so a Draggable in a scrolling list
takes finger drags only where it sets `touch_drags`, and
its preview sits above the finger instead of under it.

A `DropTarget` is registered by the rectangle its content was laid out at. The
target under the pointer that accepts the payload takes the drop - `accepts`
filters by value, and a target of another payload type never sees the drag at
all - and of the targets under the pointer the one painted last wins: the
innermost in the document tree, or the one in the higher layer, whatever order
they were built in, so a slot inside a column inside a sidebar can each take a
drop of its own. Only the part of a target that is on screen counts. Its `DropHandle` carries
`carrying`, true for every target that would accept what is being dragged, and
`over`, true for the one that would take it now; `on_over` reports the payload
and the pointer as it moves, which is how a target that works out where inside
itself the drop lands - the dock's tab bars and split zones, a timeline's
insertion point - draws its marker, and how a filmstrip reorders live. The drop
is handed over before the source hears `on_drag_change(false)`, so a source can
keep whatever it set up for the drag until the drop has used it.

A drag between editors is not this inside the plugin: a block dragged from
the file tree to another editor crosses plugins, so it goes through the host
with `Editor::drag` and `accept_drag` (guides/adding_a_plugin_editor.md). The
host carries it on its own document's board, though: a plugin's request to drag
a block puts it there with `Board::begin`, each plugin region is a `DropTarget`
for it, and the board drops whatever it carries when the primary button or the
finger is let go, wherever the press was, so a drag that no `Draggable` began
still ends; a cancelled touch ends it without a drop.

### Pan and zoom

`unstyled::PanZoom` turns wheel, trackpad and middle-button gestures over a
rectangle into camera movement. Its `view` is a `Binding<PanZoomView>` - the
world point shown at the middle of the viewport and a scale - which gestures
write; a caller that wants the camera passes its own signal, and one that does
not leaves the prop out:

```rust
let camera = create_signal(PanZoomView::IDENTITY);
view! {
    <PanZoom view={camera}>|handle: PanZoomHandle| {
        let PanZoomHandle { view, scale, .. } = handle;
        view! {
            <Canvas view>
                <CanvasItem x=0.0 y=0.0 width=100.0 height=50.0 />
            </Canvas>
        }
    }</PanZoom>
}
```

A scroll pans, Shift+scroll pans sideways, dragging with the middle button or
with two fingers pans, and Ctrl+scroll, a trackpad pinch or a two-finger pinch
zooms around the pointer or the point between the fingers. It is a tab stop as
well: the arrows pan it, `+` and `-` zoom around the middle of the viewport,
and `0` returns the scale to one. The handle carries the camera as a
`CanvasView` ready for a `<Canvas>`, the `scale` for interactive content that should grow
with the zoom (content that only draws is a `CanvasItem scales=true` instead,
guides/pan_and_zoom.md), whether a pan is in progress, and whether it has focus, which is
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
the runner hands each to `Platform::pick_file` with a `FilePickAnswer` that the
platform sends whenever the person is done - winit from a thread that runs the
native dialog, the web runner from an `<input type=file>`, Android from
`BeuiActivity`, and a plugin through the host it runs in; a platform without a
picker answers that it has none. The document delivers the answer at the start of a later frame,
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
        on_motion={move |motion: Vec2| camera.look(motion)}
        on_key={walk}
    >|handle: PointerLockHandle|
        <Scene />
    </PointerLock>
}
```

The lock is a `Binding<bool>`, like a `Toggle`'s `checked`: the control reads
`locked` and writes what it wants back to it. It lets go on
Escape, when it loses focus, and when it is removed, and keys it does not use
itself go on to `on_key`, which is what a control that also walks with the
keyboard binds.

Beui does not hold the pointer itself - it asks. `PointerLock` sets
`pointer_locked` on its `Interactive`, the document publishes whether any
`Interactive` in it asks as `WindowRequests::pointer_locked`, and whoever is showing the document does
it: the winit runner locks the cursor and hides it, and a plugin region passes
it to the host in its `WindowRequests` (guides/adding_a_plugin_editor.md). A
platform reads raw motion only while `Runner::window()`, the state it was
shown, holds the pointer, and while it does sends no absolute `PointerMoved`
(winit, web and drm alike), since a held pointer's position means nothing.
The winit platform lets the grab go itself the moment its window loses focus
and takes it back on focus only if the shown state still asks, since X11's
and macOS's grabs outlive focus. A browser drops its lock by itself, on blur
and on Escape (which the page never sees), and refuses one asked for without
a user gesture: the web platform (its `lock::Lock`) reports either, while the
shown state still asks, as `Input::PointerUnlocked`. The document answers it
as it answers losing the keyboard while it holds the pointer, by clearing
focus, so a `PointerLock` lets go and the next press takes it again; a space
host lets its grab go, as on blur. If the shown state still asks after that,
the next mouse press asks the browser again from inside its gesture and
reaches nothing, nor does its release.
What comes back is
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
        draw={Prop::dynamic(move || draw_gpu(drawing.get()))}
        @sizing=ItemSize::Percent(100.0)
    />
}
```

`Draw` has the two halves a gpu frame has. `prepare` is handed the device, the
queue and an encoder, and is where the passes of its own go - rendering into
its own textures, writing its own buffers. `paint` is handed the render pass
beui is painting the document into, and draws there. Both are handed a `DrawAt`
carrying the rectangle and the clip in physical pixels, the size of the whole
target, the scale, the target's texture format, which is what a pipeline
built on the first frame is built against, and `renderer`, a number that is the
same for every frame one renderer draws and different for any other. Key gpu
resources shared between drawings by `renderer` rather than by the device: wgpu
devices of two instances can compare equal. The color wheel
(beui-components-styled's `color_wheel/gpu.rs`) shares one pipeline that way. A `Draw` must leave the pass's
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
example. A crate that draws with the gpu does so behind
a feature that beui's `render` feature turns on, as beui-components-styled's
`gpu` does.

## Imports

App code imports `beui::reactive::*` for the framework and `beui::styled::*`
for components, and names anything else (`beui::unstyled::X`,
`beui::styled::theme::X`) on its own line. `beui::primitives` is for beui's own
crates, its tests and beui-demo's base pages; the source lint rejects it
anywhere else, since a primitive takes raw colours and pixels where a styled
component takes tokens.

## Use beui in a standalone app

The default `beui` feature is `window`, which includes the winit runner and the
wgpu renderer. A standalone app builds its document once and implements
`beui::App`:

```rust
use beui::reactive::*;
use beui::styled::*;
use beui::{App, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run(CounterApp::new())
}

#[component]
fn Counter() -> NodeId {
    let count = create_signal(0i64);
    let label = create_memo(move || count.get().to_string());

    view! {
        <List spacing=Space::Md>
            <Text style=TextStyle::DISPLAY>{label}</Text>
            <Button
                label="Decrease"
                variant=ButtonVariant::Secondary
                on_click={move || count.update(|value| *value -= 1)}
            />
            <Button
                label="Increase"
                variant=ButtonVariant::Primary
                on_click={move || count.update(|value| *value += 1)}
            />
        </List>
    }
}

struct CounterApp {
    document: Document,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            document: build(|| view! { <Root title="Counter"><Counter /></Root> }),
        }
    }
}

impl App for CounterApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
```

`build` takes a closure returning a `Root`, which is not a node: every document
built with it has exactly one `Root` at the top, holding one child, and a node
in its place does not compile. `primitives::Root` takes `clear_color` (what the
window is cleared to wherever nothing paints, so an app needs no frame filling
the window), an optional `title` and `system_bars` (`SystemBars::Light` or
`Dark`: what is behind Android's status and navigation bars, whose icons turn
dark over `Light`). `styled::Root` takes a `ColorRole` for `clear_color`,
defaulting to `Background`, and picks `system_bars` from the theme's kind unless
it is given. Every prop is reactive: the document hands its appearance
(`beui_core::window::Appearance`) to the context each time it is shown, it
travels in `WindowState`, and the runner clears with its colour, labels the
accessibility tree with its title and passes it to `Platform::show_window`.
winit retitles the window and the web platform retitles the browser tab and
colours the page's body to match; Android follows `system_bars`; DRM, headless
and plugins ignore the title. A plugin pane has no title of its own and the
plugin protocol carries none: `BeuiApp::View` returns the editor's own `Root`,
whose `clear_color` the plugin's surface is cleared to (the theme's background
unless the editor names another role), and block-editor-beui puts the frame's
chrome around that root's child. A preview, drawn over something of the
host's, is rooted in a transparent one; a plugin dialog (a creation dialog,
artifact settings) is rooted in `SurfaceRaised`, the card it fills. Holes are cut from that colour, never left by it: a child editor's `Embed`
punches its rectangle, and a painting with holes records the colour as its first
primitive so they cut it. Documents drawn over another one (the inspector, the
software keyboard) are built with `Document::new`, `enter` and `set_root` and
have no `Root`, so they leave the window's appearance alone.

Run the demo with:

```text
./scripts/buck run //crates/beui-demo:native
```

Beui's features:

- No features provides the document, components, layout, input model, and
  painting output. This is enough for headless logic tests.
- `render` adds the wgpu renderer without creating a window. Embedded hosts use
  this level.
- `window` adds the native runner and enables `render`; it is the default.
  On the desktop it is winit's. On Android it is beui's own
  (`crates/beui-platform-android`, with its Java in its `android` directory): the app's
  activity extends `com.be3.beui.BeuiActivity`, whose `BeuiView` is the
  surface, takes touches, keys and the soft keyboard's input connection, and
  hosts AccessKit. It has to be a view that input reaches through the view
  system: TalkBack's touch exploration arrives as hover events on it, which
  NativeActivity's input queue never delivers. The library defines
  `#[unsafe(no_mangle)] fn android_main(app: beui::AndroidApp)`, which beui
  calls on a thread of its own, and which calls `beui::run_with`.
  The activity draws edge to edge under transparent status and navigation
  bars: the app fills the safe area inside them, the bars show its clear color,
  and their icons turn dark when its `Root`'s `system_bars` is `Light`.

The soft keyboard takes its room out of the window rather than covering it: on
Android its inset joins the safe area, and on the web the pages' viewport meta
asks for `interactive-widget=resizes-content`. When the rectangle a document is
shown in changes size while the focus takes text, the document scrolls the
focused field into what is left, through every scroll it sits in.
- `web` enables `render` and adds the browser runner,
  `beui::run_web(element_id, options, layers, app)`, which draws with wgpu
  into the element if it is a canvas and into a canvas it adds otherwise.
  `crates/beui-web-demo` is the demo in a browser:
  `./scripts/buck run //crates/beui-web-demo:web` serves it on
  http://127.0.0.1:8070.

`beui::run_with` takes `RunOptions` (app id, starting size) where
`beui::run` takes only the app; both load wgpu, and
`beui::run_with_device` takes the `beui::OpenDevice` to open wgpu's device
with instead (block-app's opens its device itself, to import Wayland clients' buffers), and
`beui::run_on` runs on a platform of the caller's choosing, such as
`beui_platform_drm::Drm`, which drives the displays and input devices itself, or
`beui::Headless`, which draws offscreen with no window or display server. The rest of
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
  `Context::close_window`). `exiting` runs once on the way out, and the runner
  drops the app right after it, while the GPU device and the platform's displays
  are still there; no frame runs after it. Whatever the app keeps in a
  `thread_local!` must be taken out in `exiting`: the main thread's
  thread-locals are dropped only by `exit()` after `main` returns, in no set
  order, after the GPU and the rest of the runner are gone, and dropping GPU or
  reactive state there aborts or crashes the process (block-app's
  `Shell::exiting` takes down the Wayland compositor, the plugin runtimes and
  their worker threads, and the displays this way).
- `lifecycle(Lifecycle)` hears the app leave and come back to the screen, not
  the keyboard: `Background` when the window is occluded or minimised (winit),
  the activity pauses (Android `onPause`) or the tab is hidden or the page is
  left (`visibilitychange`, `pagehide`), and `Foreground` when it returns;
  `Focused` when the window gains focus; `NetworkChanged` when the browser goes
  `online` or Android's default network becomes available. The process may be
  killed without another frame after `Background`, so that is where unsaved work
  is put somewhere durable, without blocking (Android gives `onPause` little
  time). Platforms call `Runner::lifecycle`; the document's own focus still
  comes as `Event::Focus`.
- `busy` says whether work the app is waiting on, such as a server request or a
  plugin's frame, will still change what it shows. Only a driver reads it.
- `environment` is the variables a program needs to open its windows inside an app
  that hosts programs (block-app's Wayland compositor). Only a driver reads it,
  for `drive env` and `drive exec`.

From inside a frame the app can also ask the window for things through the
`Context`: `set_fullscreen`, `set_ime_area` for an input it draws itself,
`set_zoom_factor`, `close_window`, and `with_input` to show a document only the
input meant for it, which is how the inspector keeps the app's keys while its
own panel has the keyboard (guides/beui_keyboard.md, One input path).

A runner hands the document its events in batches, one per frame. A press that
follows typing waits for the next frame, so text typed before a click always
reaches the field it was typed into, however many events arrive between two
frames.

A driver is a layer, `beui_layer_drive::Drive` (guides/running/drive.md): the native run
functions stack it where `BEUI_LAYERS` names `drive` and serve its commands on
`BEUI_DRIVE_SOCKET`, pick their platform and renderer from `BEUI_PLATFORM` and
`BEUI_RENDERER` (`beui_core::runner::environment`), and block-app's web build stacks one
its `automate` export feeds (`beui::run_web` takes the layers). `Drive::new(inbox)` is one
drive layer with the app directly above it; `Drive::taps(inbox, under)` is a tap for each entry
of `under`, bottom first, sharing one session, each named by the layer just above it (`None` for
the app), for a stack with other layers between them (`drive,space,drive` is
`Drive::taps(inbox, &[Some("space"), None])`; `environment::drive_taps` works the names out of
`BEUI_LAYERS`). The drive layer keeps the accessibility tree (`Context::keep_accessibility`,
which a bare `Context` and a headless plugin do by default and other runners only for a
driver), takes the next command in the lowest tap's `input` and starts it in the `input` of the
tap it goes to: the session's and the platform's commands at the lowest, the app's reading and
input at the highest, and input under `--into=LAYER` at the tap under that layer. It turns input
commands into `Incoming`s already in points, which skip the platforms' intake
(`beui_core::input::typing` names the keys); a tap under another layer finds a named target in
the app and carries it down through `LayerFrame::above` (`Above::from_app`), and sends its input
in physical pixels (`Input::scaled`), as a platform does. It steps a stopped clock in `now`, and
answers in `framed` once a frame asks for no other and the app is not `busy`, reading the app's
`FrameOutput` (its actions, paintings, overflows) there. It acts on the window only through the
`PlatformServices` below its lowest tap, as the app does: `clipboard` copies and pastes through
them, `resize` asks `PlatformServices::resize` (which only a headless window answers), and
`state` reads the size, scale and fullscreen the window last reported. Its lowest tap answers
the app's file and save dialogs itself (`Layer::pick_file`, `save_file`), so `upload` and
`dismiss` answer them in any window, and it keeps the focus `focus` and `blur` gave. The headless platform's window is
`beui_platform_headless::Simulation` (`beui::Simulation`), a `PlatformServices` like any other:
it keeps a clipboard, grows to its screen for fullscreen, and reports its size, scale and
fullscreen through its `WindowReports`. beui's own tests drive one the same way, with a
renderer that draws nothing. `Renderer::capture` reads the presented frame back as a
`beui_core::renderer::Capture` for a screenshot.

A window nobody can see is `Runner::set_occluded`: its frames still run (layout,
paint, accessibility, the driver), but the runner hands each one's output to
`Renderer::note` instead of `prepare` and presents nothing. `note` keeps only
what the frame damaged, so the wgpu `Target` draws the union of every unseen
frame's damage into its retained frame the next time it does draw; a renderer
without `note` prepares as usual. The headless platform is always occluded, so it
draws only for a driver's `shot`.

`Context::seen` tells the app whether the frame it is running will be drawn. A
`shot` makes an occluded window seen until it is taken, and it is taken in the
first frame whose app says through `App::drawing` that it has nothing still to
draw. block-app answers with its plugins: each `DrawFrame` carries `seen`, an
unseen plugin runs its frames (layout, its description, children) without
encoding or presenting them, and keeps what they damaged, except its previews,
which another editor shows as pictures and which it paints either way; when the host is seen
again it asks each plugin that drew unseen for a frame, which paints all of it,
and `drawing` holds the shot until those frames have arrived.

The web runner draws into the canvas it is given with WebGPU, or WebGL where
the browser has no WebGPU. It reads the keyboard through a hidden text area,
so pasting and composition behave like any other input on the page. A press
on the canvas focuses that text area, and so does a text field taking beui's
focus any other way (the keyboard, a script, `drive click`), so the browser's
keys reach it. Like the
desktop runner, it only asks the browser for a frame when an event arrived,
something asked for a repaint, or a `Waker` was woken.

### Windows and window hosts

An app has one `Document`, one reactive graph and one focus, but may show
several top-level windows. A frame's `FrameOutput` holds one `WindowOutput` per
window in `windows`: its `id` (`beui_core::window::WindowId`;
`WindowId::MAIN` is the window the app starts in, `WindowId::next()` makes a
new one), its `size` in points, its display `layers`, its `WindowState`
(cursor, IME area, appearance), and its own `damage`, `moved` and `changed`.
`FrameOutput::main()` is the main window, `FrameOutput::window(id)` any other.
What happens once for the whole app (copies, pastes, file picks, closing,
actions, accessibility) stays on `FrameOutput`. A renderer, `painting` and
`quads` each take one `WindowOutput`; `painting::overflows` reads every window.

The app asks its host for windows with `Context::request_window`, which
collects `WindowRequest`s into `FrameOutput::window_requests`: `Open { window,
placement, size, role }` (a `WindowRole`: `Normal`, `Keyboard` for an
on-screen keyboard, which the host never focuses and places by the field it
types into, or `Popup` for a menu of the window it is placed over, which the
host never focuses either; the `Window` component's `role`), `BeginMove(window)`, `BeginResize { window, edge }`
(the host then moves or resizes it, as an OS does after winit's
`drag_window`), `Resize { window, size }` (the app wants another size; the
host answers with the size it gives) and `Close(window)`. A `WindowPlacement` is `AtPointer`,
`Over { window, at }` (its top-left corner at `at` in that window's points, where
the app would draw it inside that window), or `Host(HostPlacement)`: a value only the host
understands (a screen position, a 3D pose), which the app stores and hands
back unchanged. The host answers with `WindowReport`s pushed as
`Incoming::Window`: `Resized { window, size }` in points, `Placed { window,
placement }`, `Moved { window, origin }` (where the window's corner is, in points, in a
space all the host's windows share, as beui-platform-drm's desktop is), `Closed(window)` and `Focused { window, focused }` (the host's
window focus, as a window manager moves it on a click or to a window it
opens). The runner applies them before the frame
lays out, and `Context::window_size`, `Context::window_placement` and
`Context::window_origin` read them. `Context::window_hosting` says which windows a host opens, as a
`WindowHosting`: `None` (every platform on its own: a window is shown inside
the main one instead), `Any` (a layer opens every window, as space does) or `Placed`
(the platform opens only the windows it placed itself: a `Window` opened with
a `WindowPlacement::Host` the platform handed out through a handle in `Setup`,
as beui-platform-plugin's `Views` and its `views.placement(id)` do; every other
`Window` is an overlay inside the window it was opened in, so a plugin's
docked windows stay inside its region). A platform may move between them: the
plugin platform hosts every window while its host shows the windows of one of
its regions (guides/adding_a_plugin_editor.md), and a `Window` follows, opening
as a real window or an overlay from the frame the hosting changes. `Context::opens_windows` is true only
for `Any`. A platform with no main window of its own (beui-platform-plugin,
beui-platform-drm) sets `Context::set_shows_main(false)`: the document then paints neither the
root nor anything opened in the main window, so the root just holds the app's
`Window`s.

An app makes a window with the `Window` component (`beui::Window`,
`beui::styled`), an `Overlay` with a `window` spec:

```rust
view! {
    <Window
        id={window}
        size={vec2(320.0, 240.0)}
        at={pos2(40.0, 40.0)}
        placement={WindowPlacement::Host(saved_pose)}
        title={Some("Inspector".to_owned())}
        open={shown}
        on_report={move |report: WindowReport| remember(report)}
    >|handle: WindowHandle|
        <Panel handle />
    </Window>
}
```

Where a host opens windows it is one: its content is laid out from its own
corner, `(0, 0)`, at its size (the size the host last reported, else `size`)
and painted into its own `WindowOutput`. The document asks the host to `Open`
it, with `placement` and `size`, when it opens and to `Close` it when it
closes or leaves the tree. When `size` changes while it is open, to something
other than the size the host last reported, the document asks for it with
`Resize`; the window keeps the reported size until the host answers, so an
app that follows `Resized` reports in its `size` never fights the host; `on_report` hears the host's `Resized`, `Placed`
(the `HostPlacement` to store and hand back as `placement` next time),
`Rastered` and `Closed`, after which the window stays shut until the app closes and opens it
again. Elsewhere it is an `Overlay` at `at` (`Placement::At`,
`LayerPolicy::Pass`, no focus trap) holding its content at `size`, which is
how floating windows looked before there were windows, so an app looks the
same either way. `WindowHandle` (also `use_window()` inside the content)
carries the `id`, the shown `size` and `is_window()`, which says which of the
two it is (`real_window()` is the hosted window it is drawn in: its own, or the
one around the overlay it is). `begin_move()`, `begin_resize(edge)` and `close()` ask the host and
return whether it took the request: false means the window is an overlay and
the caller moves or resizes it itself. Overlays opened inside a window (menus,
tooltips) are laid out inside that window and paint into it.

Underneath, the window is an `OverlayNode` carrying a `WindowSpec`
(`Document::set_overlay_window`), and each `Layer` names its `window`: the
window's own layer and everything opened from inside it.
`Document::window_layers(id)` is one window's layers with the window's own at
index 0, which hit testing, press claims and forwarding use, so a press in
one window never reaches another's nodes at the same coordinates. Focus,
key routing and tab order stay one across every window, except that a host's
`Focused { window, focused: false }` takes the focus from a node inside that
window (remembered per window) and `focused: true` gives it back when nothing
else has it since. A window with nothing to give back first lets its
`on_report` hear the `Focused` and choose: a dock window the host focuses
(`window_focused` in `beui-dock/src/unstyled.rs`) focuses its pane and, unless the keyboard is
already inside it, hands it to the shown tab (`give_keyboard`, the node that
tab last had, else its first focusable), so a tab floated into a window the
host focuses on opening takes the keyboard with it, while the main window
keeps the remaining tab's focus for when it is focused again. If the keyboard
is still not inside the window after that, the document focuses its first
focusable in tab order (`focus_into`, which waits for the content if it is not
built yet), so a plain `Window` the host focuses on opening gets the keys too.
The main window gets no such fallback. `Document::window_of`
names the window a node is in, `window_rect` its rectangle and `open_windows`
the windows the host shows. Every window's rectangle starts at the origin, so
damage is told apart by window: the paint cache files what each root's nodes
damage under the window it paints into (`PaintCache::paint_into`,
`damage::WindowRegions`), and each window gets only its own, clipped to its
size (the whole window when it opens or resizes), so a window resizing or
animating leaves the others' paintings untouched. Scroll copies are filed the
same way: each `paint::Move` names its window, the document settles each
window's moves against that window's painting and damage alone, and each
`WindowOutput` carries its own `moved`, so scrolling one window copies in that
window and damages nothing in the others.
Each window is drawn at its own pixels per point: a host reports one with
`WindowReport::Rastered { window, pixels_per_point }` (the main window
included), and until it does a window uses the context's. It is the window's
raster, not its layout: every window is laid out at the context's scale. The
document lays out and paints each window's tree, and everything opened inside
it, with `Context::window_painter(window)`, whose `PainterState::raster` is that
window's pixels per point over the context's, so its text is rasterized for the
size it is drawn at without moving, as a zoomed canvas's is
(`Document::viewport_window` names the window being laid out, since the
overlays above a node are out of the arena then). A change of scale lays
nothing out: it repaints the window's tree at the new raster, and the window's
output is damaged whole. `WindowOutput::pixels_per_point` is the
scale the output was drawn for, which a host draws it at.
The cursor goes to the window the pointer is over, the IME area to the window
holding the focus, and the rest of `WindowState` stays on the main window,
except the appearance: a `Root` built inside a hosted window sets that
window's clear colour, title and system bars
(`Document::update_appearance(Some(window), ..)`), and a window without one
takes the main window's. The accessibility tree stays one tree, with each
window's nodes in its own coordinates; `FrameOutput::window_accessibility_tree`
is the part of it in one window (its nodes, rooted at the window, with the
focus when the focus is there), and `FrameOutput::window_overflows` names the
window of each overflow.

What a host is told about one window is asked of the document by window:
`overlay_rects(window)`, `floating_rects(window)` and `overlay_layers(window)`
cover the overlays opened in it, `drawing_rects(window)`,
`press_claims(window)` and `wants_every_sample(window)` the nodes in it, and
`measure_node(ctx, node, available)` measures a node with the painter of its
window. A key listener at `KeyAt::Window` and an action, registered inside a
`Window`, belong to the hosted window that holds it
(`Document::host_window_of`): they hear a key only while the key's window is
theirs (`Document::key_window`: the window of the focused node, else the one
the host focused last) or while no window is known, so two plugin regions each
keep their own Escape and shortcuts. `Context::request_action_in(window, id)`
runs that window's action of that id, and the actions the document describes
come once more per window, as an `ActionGroup` whose `window` names it. Where
the platform opens only the windows it placed (`Placed`), a host's `Focused {
window, focused: true }` moves the focus into that window, keeping the focus it
takes from another window for when that one is focused again and giving back
the one this window last had, and focuses nothing else; `focused: false`
leaves the focus where it is. A `Resized` report for the window holding the
focus reveals a focused text field the way a resized main window does.

Tests host windows with `testing::Stepped::hosting_windows` (block-ui-test's
`DocumentTest::hosting_windows`), whose host opens every window asked for at
its size and reports `Resized` and `Placed` back the next frame.
`frame_in(window, inputs)` sends input to a window, `take_window_requests`
reads what the app asked, `report_window` reports as a host would, `windows()`
lists the open windows and each one's latest `WindowOutput`, and
`DocumentTest::snapshot_window(name, id)` takes a window's painting.
`Stepped::hosted(context, app, window, layer, setup)` runs under a real
window-hosting layer instead, such as `beui_layer_space::Space`, stacked over
its drive layer, with the `Setup` it is given (space wants `GpuSetup` and the
`Context`), so a test can paint the scene that reaches the platform
(`Stepped::painting`, `drive paint in platform`), as be-wayland's space test does.

Pointer input names its window: `Incoming::Input` carries a `window`
(`Incoming::in_window`; every constructor starts with the main window), in
that window's own coordinates. The intake turns a change of window into
`Event::PointerWindow`, so every `PointerEvent` and `InputState::pointer`
carry the window too. A host names the window under the pointer even while a
press made in another window holds it, and does not send `PointerLeft` when
the pointer only crosses into another window: the node holding the press goes
on getting the drag, and its `PointerPress` names the window the pointer is
over and the position in that window (`local` is then measured from the
holder's rectangle in its own window, so it means little; a click needs the
release in the window the press began in). A forwarding catcher (a plugin
region, a Wayland window) is handed those events in its own window's
coordinates instead, when the host has reported both windows' `Moved` origins:
the position moves by the difference between the origins and the event names the
catcher's window, so a drag begun on one display goes on smoothly over the next.
Each pointer event a catcher is handed also names the node the pointer is over
(`Forwarded::Pointer`'s `over`, an `Over`: the node and the pointer's position
from that node's corner in the node's window), which is how a host hands a held
press on to the region it is over: `Replay::replay(input, zoom, aim)` asks `aim`
where each pointer event goes, `Aim::Own` (this catcher, from its corner),
`Aim::Other { node, at }` (replayed at `at`, with `Replayed::over` naming the node)
or `Aim::Away` (over nothing of its own: while a button is held, `PointerLeft` once,
then the catcher's own coordinates). The drag board looks for drop
targets only in the window the `DragPoint` names. A host signals that the pointer, holding a
button, is over no window by sending `PointerLeft` (moves and the release then
keep going to the window the press began in, outside its rect): the intake
turns that into `Event::PointerOutside`, which sets `Pointer::outside` until
the next press. While it is set and the pointer is outside its window's
reported size, the board (`Board::set_outside`, each frame) drops nothing:
the target the pointer left hears `over(None)` and the drop calls the
`Outside` callback given to `Board::begin` with the last point (inside or
out, so a drag that leaves on its first move still has one), which
`Draggable` passes on as `on_drop_outside`. Unless a host opens every window
(`Context::opens_windows`, `WindowHosting::Any`) the flag is ignored, so dragging out of a platform window still drops at the last
point inside. `ui_types::Input` has no window: it is the
plugin protocol's input too, and a plugin region is always one window.

### Layers

A `beui_core::runner::Layer` is middleware between a platform and the app's
runner. `Launch::layers` is the stack, the layer nearest the platform first;
the native run functions (`beui::run_with`, `run_on`) build it from
`BEUI_LAYERS` (`choose_platform` and `stack` in `crates/beui/src/window.rs`).
Input goes up the stack, from the platform through each layer to the app, and
the app's output and its calls on the platform go down it. Every hook has a
default that changes nothing:

- `attach(&Context)` once, as the runner is made: a layer that opens the app's
  windows sets `WindowHosting::Any` here (see above).
- `setup(&Setup)` after the renderers load and whenever the GPU is replaced;
  `setup.get::<beui::GpuSetup>()` is the wgpu device, queue and surface format,
  `setup.get::<Context>()` the app's context and `setup.waker` wakes the
  platform.
- `pace() -> Option<Instant>`, before everything else in a frame: a layer that
  paces frames (an XR headset) blocks for the next one and returns its
  predicted display time, which becomes the frame's time (unless the clock is
  stopped; never earlier than the last). While a layer paces, the runner turns
  the window's vsync off (`Renderer::set_vsync`) and keeps framing while the
  platform window has no size (minimised), at its last size and unseen.
- `now(Instant) -> Instant`, bottom first, before the frame: the frame's time.
  The runner reads the context's clock once (`Context::stop_clock` and
  `advance_clock` stop and step it), and the layers, the app and every later
  hook see the time the stack settled on (`LayerView::now`, `Context::now`).
- `input(&mut Vec<Incoming>, &mut LayerFrame)`, bottom first, with everything
  that arrived for the frame before the app sees it: a layer may rewrite, drop
  or add entries, name each pointer event's window in that window's
  coordinates, and push `WindowReport`s. Right after it the runner asks
  `keyboard_attached()`, top first: the first `Some` overrides whether the
  platform reports a keyboard attached (see The on-screen keyboard), `None`
  (the default) leaves it to the layers below and the platform.
- `output(&mut Shown, &FrameOutput, &mut LayerFrame)`, top first, after the
  app's frame. `Shown` is what the layers above left of the app's windows and
  window requests (the app's own at the top); a layer may read, replace or
  drop them. What reaches the platform is what the bottom layer left:
  `Runner::shown_window(id)` reads it, and the runner hands its main window to
  `show_window` and the renderers. The `FrameOutput` is the app's own, which
  `Runner::output()` keeps.
- `framed(&mut Frame, &FrameOutput, &mut LayerFrame)`, top first, once the
  frame is prepared: a layer may ask for the next frame sooner (`repaint`,
  `repaint_after`) or read whether one is due.
- `wants_sight()` keeps an occluded window's frames drawn, and
  `wants_capture()` asks the renderers for the pixels of the frame last
  presented, which `captured(Option<Capture>)` hands over.
- `up(window, at)` and `down(window, at)` map a point in points between the
  windows below the layer and the windows above it, `None` where it reaches no
  window. `LayerFrame::below` maps through the layers under one to the platform
  (`to_platform`, `from_platform`) and `LayerFrame::above` through those over
  it to the app (`to_app`, `from_app`).
- The services: `copy`, `copy_image`, `paste`, `pick_file`, `save_file`,
  `delivers_paste_shortcut`, `shows_keyboard`, `show_window`, `resize`,
  `set_fullscreen` and `publish_accessibility`, each handed the
  `PlatformServices` below it (`LayerFrame::below` is the same thing), which it
  forwards to by default, so a layer may answer a request itself or change it
  on the way down.

`LayerFrame` also carries the `LayerView` (the platform window's size in
physical pixels, the pixels per point and the frame's time), the `Context`, the
`App` (for `busy`, `drawing`, `environment`, `close_requested` and
`request_painting`), the time the frame started, and `close()`, which ends the
app after the frame as a closed window does. An `Rc<RefCell<L>>` of a layer is
a layer too, so a test or a bench keeps a handle on one it stacked
(`InSpace` in crates/beui/src/tests.rs). Two layers that share state through
such a handle can sit at different heights in one stack.

With a layer that opens windows, the main window's size is the one the layer
reports, not the platform window's, and the app gets no safe area. A wgpu
layer draws each window into a texture of its own and composites them through
a `Drawing` (`beui::drawing`, whose `Draw::prepare` has the device, queue and
an encoder) in a `display::Layer::Shape(Shape::Drawing { .. })` covering the window it
returns. beui-platform-drm under such a layer shows the main window on its
first display.

### Displays under beui-platform-drm

beui-platform-drm shows no main window and opens only the windows it placed
(`WindowHosting::Placed`): one per display, macOS-like. It hands the app a
`beui_platform_drm::Displays` handle through `Setup` (`setup.get::<Displays>()`):
`displays()` lists each connected display as a `Display` (its `id`, which stays
the same while the monitor is plugged in, its `name`, its `position` and `size`
in the desktop's points, and its own `scale`, pixels per point), `display(id)`
one of them, `watch(f)` hears every change (a display plugged in or out, a new
mode, rotation or scale) before the frame it reaches lays out, and
`placement(id)` is the `WindowPlacement` to open that display's `Window` with.
The app opens a `Window` per display at the display's `size`, so it lays out at
the right size in the frame that first shows it; the platform matches the
window's `Open` to the display in that same frame and draws its output there,
and reports the window's `Resized`, `Moved` (the display's position) and `Scaled`
(the display's scale; a window opened on a display at another scale than the
first is drawn crisp from its second frame). It reports no placement for any
window, so a dock never reopens one, and any other `Window` is an overlay in the
window it was opened in. A display with no window shows its clear colour.
The desktop puts the displays side by side, left to right, along their bottom
edges, each `size` its physical mode over its scale; the main window is the
first display's size at its scale, which is also the context's.
Pointer input goes to the window of the display under the pointer, in that
window's points (`Units::Points`), and a press focuses the window it lands in
(`WindowReport::Focused`); a press held in one window goes on as the pointer
crosses into another, by the rules above. Each display's scale is `BEUI_SCALE`
(1 when unset) unless `DisplayConfig::scales` names one for its monitor id.
`Displays::new` makes a handle with displays of a test's own, and
`Displays::set` changes them.

### Space: the VR simulator layer

`crates/beui-layer-space` is a `Layer` that shows an app's windows as
panels in a first-person 3D scene, a VR simulator that runs on any wgpu adapter.
`BEUI_LAYERS=space` stacks it over any native or headless (`:dev`) run. It opens every window
(`WindowHosting::Any`), rewrites the platform's input in `input`, composites the windows above it
into the one main window it leaves in `Shown`, asks for frames while it animates in `framed`, and
maps points between the platform window and its panels (`up` through the ray at a point, `down`
by projecting a panel's point). Its inherent `Space::input` and `Space::frame` take a `LayerView`
directly, which its tests call. Everything it does is in the layer, so no platform knows about
it, and all its 3D (`glam`) stays in the crate.

- Panels: `Space::panels` is a list of `Panel { window, pose, size,
  metres_per_point, bend }` (`Panel::new` starts flat at 1 / `POINTS_PER_METRE`)
  keyed by `WindowId`; a panel is `size * metres_per_point` metres
  (`Panel::metres`), and `bend` is the angle its width wraps round a vertical
  cylinder whose axis is in front of it, so its sides come toward the viewer
  (0 is flat, `MOST_BEND` half a cylinder). `Panel::world`, `offset` and
  `intersect` work on that surface, flat or bent, and on its extension past
  the window's edges; `frame_at(pos)` is the surface's point and turn at a
  window point and `anchor(pos, frame)` places a reshaped panel so that point
  stays put, which keeps a resized or scaled panel's fixed corner where it was
  on a bent panel too. On the first
  frame it opens the main window's panel at the platform window's size in
  points, at eye height straight ahead, far enough that it fills 80% of the
  view's width, and reports that size (`WindowReport::Resized`).
- Windows: space opens every window (`Context::opens_windows`), so every
  `Window` is a real window here. `Space::request` handles each of the frame's
  `WindowRequest`s. `Open` adds a panel at the size asked for and reports
  `Resized` and `Placed` the next frame: `Host` decodes the pose, scale and
  bend (`Panel::placed`; a placement another host wrote, or one it cannot
  read, opens at the pointer instead), `Over { window, at }` puts
  its top-left corner on that panel's point `at` (in its points), at that panel's scale, and
  `OVER_NEARER` (3 cm) in front of it, facing the same way, a further
  `OVER_NEARER` nearer for each panel already there, and `AtPointer` puts the
  point `OPEN_GRAB` (24, 16) of the window on the cursor's ray, `OPEN_NEARER`
  in front of the panel the ray hits or `OPEN_DISTANCE` (1 m) away when it hits
  none, turned to face the viewer as a carry leaves a panel (`Panel::face`, see Carry). A window that opens takes the
  focus (see Focus); a window asked for again keeps its panel and leaves the
  focus alone. A `WindowRole::Keyboard` window (`Space::keyboards`) ignores
  its placement and opens as `Over` would put it on the panel holding the
  focused field (the window whose output carries the IME area), centred under
  the field `KEYBOARD_GAP` (12) points below it, or with no field over the
  bottom of the focused panel; it never takes the focus, however it is opened,
  pressed or carried, so its keys type into the field, and a hand's grip
  carries it like any panel. A `WindowRole::Popup` window opens where its
  placement says and never takes the focus either, so the window it belongs
  to keeps it (a Wayland popup's grab ends when its toplevel loses the
  keyboard). `Resize` sets the panel's size,
  keeping its top-left corner, and reports `Resized` the next frame. `Close`
  removes the panel and its texture; the main window's panel never closes.
- Grab: while its window has focus the host holds the pointer
  (`Space::grabbed`; it asks for `pointer_locked` all that time), so the mouse
  sends only `PointerMotion`. Losing focus (`Input::Focus(false)`), the
  platform dropping the lock (`Input::PointerUnlocked`: a browser on Escape)
  or holding Escape for `hold::LET_GO_AFTER` (0.6 s, on the frame clock) lets
  it go, and then the first press into the window grabs it again and reaches
  nothing, nor does its release. Escape's press reaches the app at once, since
  apps use a tap of it; space asks for a frame at the deadline
  (`repaint_after`), and once the hold lets go its repeats stop reaching the
  app but its release still does, so the app's key state stays whole. While a window the
  cursor is on asks for pointer lock itself, motion goes to it instead and
  the cursor stays put.
- Pointer: the cursor is a point in the world (`Space::cursor`), and every
  press, move and hover goes where the ray from the eye through the cursor as
  drawn (`Space::cursor_ray`, `cursor_screen`) meets the nearest panel's front
  (`Space::pick`): the app gets it as that window's, in its points (on a bent
  panel, the point of the curved surface the ray meets). A press's own
  position is ignored. An absolute position, a `PointerMoved` in pixels or in a
  window's points (a driver's, mapped onto that window's panel), puts the
  cursor where its ray hits; `PointerMotion` swings its ray about the eye,
  gravity-aligned whatever the head's pitch and roll (`Head::swing`): motion
  across turns it about the vertical, keeping its height, and motion up raises
  it toward the sky along its line to the zenith, short of
  `head::MOST_ELEVATION`, both at a fixed angle per pixel
  (`Head::pixels_per_radian`, so near the middle of an unrolled view a pixel
  of motion is a pixel on screen), the same in the simulator and under a
  headset. Either saves it `CURSOR_LIFT` (1%) of the distance in front of what
  it hits, or on a miss at the last hit's distance (`OPEN_DISTANCE` before any
  hit). Turning the head and walking leave it at its point, so it slides
  across the view, and nothing holds it inside the view: motion or a turn can
  take it anywhere around the viewer, it still points (and clicks) where it
  is, and turning back or moving the mouse back finds it. While a panel is
  carried the cursor stays on its grabbed point. Moving onto another
  panel just names the new window, held button or not (no `PointerLeft`), so
  a drag goes on across windows. Moving off every panel sends the last window
  `PointerLeft`; while a button is held, moves and the release then go to the
  window the press began in, at the point the ray meets that panel's plane,
  outside its rectangle. That is how an app tells a drop on nothing (the dock
  floats a tab dropped there into a window opened `AtPointer`). Presses that
  miss reach nothing.
- Head: holding Ctrl and Shift (either side) steers. Ctrl or Shift alone
  still reaches the app; when the other joins it the app sees both keys let go and its modifiers cleared. While steering
  `PointerMotion` turns the head (yaw about the vertical, and
  pitch clamped short of straight up or down), W, A, S and D, by their
  place, walk along the floor relative to where the head faces, and Q and E
  roll it left and right (`Head::roll`, `ROLL_PER_SECOND`), all timed from
  `LayerView::now`. `Head::rotation` is yaw, then pitch, then roll about the
  view direction; the view, cursor rays and `scale` all go through it, but
  the cursor's motion does not (see Pointer).
  Mouse steering with the head rolled first turns the motion by the roll
  into the level frame, so moving the mouse along the screen turns the view
  along the screen while yaw stays about the vertical and the roll stays put. No key pressed while steering reaches the app, nor its
  release, and clicks reach it without Ctrl or Shift, so an app in space never
  sees a Ctrl+Shift chord. Keys, modifiers and `PointerMotion` sent by `drive`
  go through the same rule (a driver's motion is in points, scaled to pixels
  first), so `drive --into=space keydown ctrl shift w` walks and `drive --into=space motion
  200 0` while Ctrl and Shift are held turns the head.
- Views: what the scene is drawn through is a list of `head::View { position,
  rotation, fov, size }`, a pose with an off-axis `head::Fov` (built from the
  tangents of its left, right, up and down edges, `Fov::tangents`, or from
  OpenXR's angles, `Fov::from_angles`; `Fov::projection` is right-handed with
  depth 0 to 1 between `NEAR` and `FAR`) and its target's size in pixels.
  `Space::views` is the pose source's: the simulator's one view, the head over
  the platform window (`Head::view`, a symmetric `FOV_Y`), or the tracked
  `head::Views { display_time, views }` an XR host hands `Space::set_views`
  each frame (`None` goes back to the simulator), in world space, whose floor
  is y = 0. Tracked views move `Space::head` to their centre (mean position,
  averaged rotation, `Head::posed`), so everything that reads the head (cursor
  rays, carry, chrome, the window, which mirrors that centre eye at
  `FOV_Y`) works off the centre eye. `Space::draw(encoder, targets)` draws
  each `ViewTarget { view, texture, format }` (a 2D texture view of that
  view's size, such as an XR swapchain image) through its own view: a 4x
  multisampled pass per view resolved into the texture, after painting any
  window that changed; the platform window is still drawn by the `Drawing`
  `Space::frame` returns.
- Headsets: `BEUI_LAYERS=xr`, in space's place, shows space on
  an OpenXR headset through `crates/beui-layer-xr`, and waits for one when
  there is none; `BEUI_LAYERS=space` is the simulator. `beui::window` makes
  a `Link` (`Link::new(app name)`: `RunOptions::app_id`, else the program's
  name, for the runtime's dashboard), which tries once at once and then, on
  a thread of its own, every `RETRY` (2 s): load the runtime and make an
  instance (`Runtime::open`), then ask it for a headset (`xrGetSystem`),
  keeping the instance while the answer is `FORM_FACTOR_UNAVAILABLE`, so
  SteamVR started later or a headset plugged in later are both found without
  starting SteamVR again. `Link::status()` is `Found`, `Missing(why)` or
  `Failed(why)` (the runtime could not make or share the GPU; it then stops
  looking), and each change wakes the loop through the `Waker` the layer
  hands it in `setup`. The `Link` is the renderers' `OpenGpu`
  (`Link::open_device`): it opens the device through the `Xr` it found,
  else a plain one, and reports itself `OpenGpu::stale` once it has found
  an `Xr` the device was not opened through. A renderer whose `OpenDevice`
  is stale reopens its device as after a lost GPU (`Renderer::recover`,
  `Gpu::reopen`, which asks the `OpenGpu` for a fresh instance), and the
  runner hands the app (`App::renderer_replaced`) and the layers the new
  device; `XrSpace::setup` then takes a `Headset` for `Link::opened()`.
  While there is no headset, or its session is lost or not yet running,
  `XrSpace` hands space a `notice::Notice` ("Waiting for a headset" and
  why) that space draws over the mirror window (`Space::set_notice`), and
  space is the simulator meanwhile: its head at eye height looking at the
  main panel, the mouse and keyboard on the panels, no pacing and no
  polling. `Headset::lost()` says how a session ended: `Lost::Session`
  (`LOSS_PENDING`, the headset unplugged) is opened again on the same device
  once `xrGetSystem` finds the headset, tried every `RETRY`
  (`Headset::reopen`); `Lost::Instance` (the runtime quit: an instance
  loss event, `ERROR_INSTANCE_LOST`, or `xrGetSystem` failing otherwise)
  drops the headset, its session and instance (the device holds its `Xr`
  only weakly, since the Khronos loader allows one instance at a time) and
  sends the `Link` looking again (`Link::lost`), which reopens the device
  through the next instance; `Lost::Ended` (`EXITING`, or swapchains or a
  session that could not be made) leaves the simulator for the rest of the
  run, so a runtime that closes the app is not answered by opening it
  again. The mirror keeps the simulator's head where the headset left it.
  The runtime is loaded with the
  `openxr` crate's `loaded` feature: the system's `openxr_loader` if there is
  one, else the active runtime's own library, found as the Khronos loader
  finds it (`XR_RUNTIME_JSON`, then `active_runtime.json` under the XDG
  config directories on Linux, the `ActiveRuntime` registry value on
  Windows) and negotiated with directly (`runtime.rs`), since Windows has no
  system loader and SteamVR ships none. It builds on Linux and Windows; on
  other platforms the `Link` fails at once and space stays the simulator.
  The runtime picks the GPU, so the `Link` is an `OpenGpu`
  (`beui_renderer_wgpu::present`): `OpenDevice { extensions,
  gpu }` says how a renderer opens its device, the Vulkan device extensions
  it wants where the adapter has them (be-dmabuf's `DEVICE_EXTENSIONS`, which
  block-app passes on Linux, and be-win32-capture's on Windows, enabled through
  `open_vulkan_device`) and, when set, the
  `OpenGpu` that makes the instance and may make the adapter and device
  itself (`None` leaves them to wgpu). `Xr` makes
  the VkInstance and VkDevice through `XR_KHR_vulkan_enable2` with the
  extensions wgpu wants (`desired_extensions`, `required_device_extensions`)
  and those, and hands them to wgpu (`Instance::from_raw`, `expose_adapter`,
  `device_from_raw`), so the window, the panels, dmabufs and the headset
  share one device; it then starts the session. `XrSpace` wraps `Space` as
  the layer. Frames are paced by the headset: `Layer::pace`, which the
  runner calls before each frame, blocks in `xrWaitFrame` and returns the
  predicted display time, which becomes `LayerView::now` and the frame clock
  (unless the clock is stopped; never earlier than the last), and while a
  layer paces the runner turns the window's vsync off
  (`Renderer::set_vsync`, `AutoNoVsync`) and the layer asks for the next
  frame at once. Input locates the eyes in a `LOCAL_FLOOR` space (`STAGE`,
  else `LOCAL` lowered by eye height) and hands them to `Space::set_views`;
  just before drawing, the frame locates them again for the same display
  time (`Headset::relocate`, which the projection layer then names) and
  hands those over instead, then draws them into each eye's swapchain image (imported with
  `create_texture_from_hal`, one swapchain per eye, sRGB where offered),
  submits, releases the images and ends the frame with a projection layer.
  Panels whose order to the eye is plain are also shown as OpenXR
  composition layers, sampled once by the runtime's compositor, so text stays
  sharp: `Space::layers(&layers::Allowance { most, cylinders, largest })`
  picks them (`layers::choose`): panels facing the eye, not held by a
  carry, resize or gesture, no bigger than the runtime's swapchains, bent
  ones only where `XR_KHR_composition_layer_cylinder` is enabled, those that
  look biggest first, up to the runtime's layer count less the projection
  layer (and `MOST_LAYERS`, 8, for memory), and none whose order to another
  chosen one is unclear: two that can overlap on screen must lie wholly
  behind or in front of each other's planes, else the second keeps to the
  eye buffer. Each `PanelLayer` (pose, metres, bend, texture pixels and
  format, and a `version` the compositor bumps on each repaint) becomes a
  `beui_layer_xr::Quad`; `Headset::submit` keeps a swapchain per layered window (one no longer
  layered is dropped three frames later, once the GPU is done with it),
  acquires an image only when its version changed (an idle panel costs no
  copy), and `Space::draw`'s `LayerTarget`s have the compositor copy the
  panel's texture into it (`fill`) and draw each layered panel's front as a
  hole, cleared to transparent with depth, so nearer panels, chrome, beams
  and the cursor drawn in the eye buffer still cover it. The layers go
  under the projection layer, farthest first, as quads or cylinders (posed
  at the cylinder's axis, `central_angle` = bend), and the projection layer
  blends by its alpha (`BLEND_TEXTURE_SOURCE_ALPHA`). The mirror window
  draws every panel itself. `Headset::layered` counts the last frame's
  layers.
  A frame begun and not submitted is ended empty at the next `pace`. While
  the session is idle the layer polls the runtime every `IDLE_POLL` (100 ms).
  The mouse and keyboard work exactly as in the simulator: the mirror window
  keeps the keyboard focus and the pointer, and the cursor's screen is the
  mirror, the centre eye at `FOV_Y`, which lies inside a headset's field, so
  the marker of a cursor out of view (see Cursor) never sits in the periphery. Tracked views are in the
  tracking space, and `Space::body` (`head::Body { position, yaw }`) places
  that space in the world (`Views::placed`, and each hand's poses through
  `Body::place`); while views are tracked,
  Ctrl+Shift steering moves the body instead of the head: mouse motion
  across turns it about the eyes (motion up and down, and Q and E, do
  nothing), and W, A, S and D walk it the way the head faces. The body stays
  while the headset is away. The runner keeps framing a paced layer while the
  platform window has no size (minimised), at the window's last size and
  unseen, so the headset's frames go on. Controllers come from an action set (`controllers.rs`) attached when
  the session starts: per hand, the aim and grip poses, trigger and squeeze,
  bound for the simple controller, Index, Touch, Vive and Windows Mixed
  Reality profiles, read at the predicted display time in the eyes' space
  and pushed into the frame's input as `Incoming::Controller` (see Lasers),
  with `Lost` once when a hand's aim stops being tracked.
  `beui-layer-xr`'s tests run Monado's service with its null compositor on
  lavapipe (`//buck/sysroot:amd64-xr`) and have it show frames, the main
  panel as a quad layer; one starts the `Link` before the service, then
  stops and starts the service again, reopening the device as the runner
  would (`Gpu::stale`, `Gpu::reopen`, `XrSpace::setup`). They share
  `XDG_RUNTIME_DIR`, so each holds the tests' `MONADO` lock.
- Carry: on `WindowRequest::BeginMove` (for any window) the panel keeps its pose relative to the
  head, with the grabbed point on the cursor's ray at a fixed distance, so
  moving the mouse or steering carries it. As the mouse swings the ray around
  the viewer the panel turns with it, by the ray's change of yaw about the
  head's up, the up the facing below uses (`Carry::aim`); steering turns it with the head instead. From
  the moment it is grabbed it also turns, over `TURN_SECONDS` of
  `LayerView::now` (smoothstep, `carry::Turn`), from that carried rotation
  to facing the viewer: a look-at basis whose normal through its centre
  points at the eye and whose up is the head's up (`Head::up`, `Head::rotation`
  times Y) made perpendicular to that normal, so it shows upright to the view
  anywhere on screen however the head is rolled or pitched (level edges when
  the head is level), with the grabbed point kept on the ray (`carry::facing`,
  closed form: the eye's direction in the panel's frame is known from the
  grab offset, and the one turn left about the line to the eye is the one
  that keeps the panel's right perpendicular to the head's up), and then
  keeps facing them, so letting go turns it no further.
  The carry keeps the grabbed point at `Carry::distance` from the eye (where
  it was picked up, changed by scroll), whatever it passes in front of or
  behind: panels are drawn and picked by depth alone, so a panel carried
  across a nearer one goes behind it. Scroll changes the distance by
  `STEP_PER_LINE` (0.95) per line, about 5%; a touchpad's pixels move it by
  their share of a line (`SCROLL_LINE` points), so it glides in proportion,
  between `carry::NEAREST` and `FARTHEST`. It does not reach the app, which
  sees no pointer moves meanwhile.
  An app's `BeginMove` or `BeginResize` starts a carry or resize only while
  a press it was given is held (any window's, since one app's drag can open
  and carry another window; for a laser, while its trigger holds that
  window), as Wayland ties a move to its press: a plugin's request reaches
  block-app a frame or two after the press it answers, and one that arrives
  after that click was let go is dropped rather than leaving the window
  stuck to the cursor until the next click. Letting go of the button that
  was down when it began (or, for a carry begun with none, as
  `Space::begin_carry` can, the next click) ends it: the app gets that
  release in the window that took the press, where it grabbed when that is
  the carried one, the turn to face the viewer
  finishes at once (`Carry::finish`) and the panel is placed once more,
  so a carry let go mid-turn lands where it would have after the turn, and the host reports `WindowReport::Placed` with the panel's placement,
  `space X Y Z QX QY QZ QW S B`: its position, rotation, `metres_per_point`
  and `bend` (`Panel::placement`, read back by `Panel::placed`). The app keeps
  it in memory (the dock does) and hands it back when it opens the window
  again. Nothing keeps a placement across restarts yet (plans/vr.md).
- Resize: on `WindowRequest::BeginResize { window, edge }`, or a drag on a
  side of a panel's chrome (`resize::Resize`)
  the cursor's ray meets the panel's surface as it was, and that edge or corner
  follows it while the opposite one stays put, no smaller than
  `resize::SMALLEST`. Each new size is reported as `Resized` at once, so the
  app lays out at it the same frame; the app sees no moves meanwhile, and the
  carry's rule for letting go ends it, with a `Placed` for the panel's new
  centre.
- Sharpness (`scale`): each frame `Space` gives every panel the most pixels per
  point any of its views shows it at (`scale::wanted`): per view, the frustum-clipped panel's nearest depth
  (its outline sampled along the top and bottom edges, so a bent panel's
  nearer sides count), as the view's pixels per point there (so a panel scaled
  up by its chrome gets more; a view's pixels per unit of tangent,
  `Fov::pixels_per_tangent`, is the larger of across and down), times `SUPERSAMPLE` (1.25, so the main
  panel's first 80% view is drawn at exactly the platform's scale), rounded
  up to a step of `STEPS_PER_DOUBLING` (4) per doubling, between
  `LEAST_SCALE` (0.5) and `MOST_SCALE` (4) and no larger than
  `LARGEST_TEXTURE` (4096) texels a side. It keeps a scale until the panel
  needs more, or `KEPT_STEPS` (2) steps less, and a panel out of sight keeps
  its own, so walking about redraws a window only now and then. A new scale
  waits until the panel has wanted it for `SETTLE` (250 ms of
  `LayerView::now`, the host asking for a frame when the wait is over), and a
  panel being carried, resized or reshaped by its chrome keeps the one it has
  until the gesture ends, so a scale never changes every frame. A change is
  reported as `WindowReport::Rastered` (`Space::scale` reads it), which is
  only the window's raster: the app repaints the window at it without laying
  anything out, so its text stays where it was and only gets sharper or
  softer. A new window takes its scale at once. An `Embed` in the window
  hears the new raster (`EmbedPlacement::raster`), which is how block-app
  sends a plugin region in it its raster beside its scale (the protocol's
  `raster`, `RegionPlacement::raster`), and beui-platform-plugin reports it
  to the region's window as its `WindowReport::Rastered`, so plugin content
  in space is as sharp as the window and is not laid out again.
- Drawing: each window's display is drawn by its own
  `beui_renderer_wgpu::Renderer` into a mipmapped texture at its output's
  `pixels_per_point`, only when that window's output changed, repainting only
  its damage after copying its `moved` scroll inside the texture. A window that
  changes size gets a new texture but keeps its renderer, whose pipelines and
  glyph atlases cost far more to build than a resize step's whole frame. It is sampled with
  trilinear and anisotropic filtering and a level-of-detail bias of -0.5
  (`SHARPEN` in space.wgsl), so a texture up to 1.4 times the panel's size
  on screen is read from its sharpest level;
  then a 4x multisampled pass draws the backdrop, the floor grid and the
  panels. The backdrop (`floor.rs`, `background_fragment`) fills the view with
  no depth and is coloured by each pixel's direction alone: `SKY` above the
  horizon, `GROUND` below it, blended by `floor::ground_share` (the floor's
  `floor::fade` at the distance a ray from `EYE_HEIGHT` would meet it), so
  the floor is part of the sky, not a surface. Only the grid lines are 3D, on
  the plane y = 0 around the eye, faded with distance as the floor colour is,
  and blended over the backdrop without depth test or write before any panel,
  so a panel under the floor is drawn over them and stays visible and
  pickable (it is not hidden by anything but nearer panels). The panels are
  depth-tested (their backs grey; nearer panels hide farther ones by depth, a
  carried one too), and a
  `Drawing` copies it texel for texel into the platform window. A frame is composed again only
  when the head, a panel, the cursor or a window's output changed. A panel is
  a strip of `render::SEGMENTS` columns bent in the vertex shader. The
  `Drawing` also paints (`picture.rs`, `Paints`): the backdrop as a 16x16
  mesh of the screen with each vertex's direction's colour, the grid lines
  within 20 m as thin quads (fading out by 20 m, where the GPU's go on to
  `floor::EXTENT`), then each panel's and its chrome's paintings placed onto
  the projected surface cell by cell (`picture::scene`), which is what the
  crate's painting tests compare. Cells are ordered by depth per pixel, as
  the GPU's depth test does: each cell is a flat quad, and two cells
  overlapping on screen are in a fixed order wherever the difference of
  their planes' depths along the eye's rays keeps one sign over their
  overlap (checked at its corners, since it changes sign only along a
  straight line). A cell with nothing to draw takes no part. When every
  pair is decided, the cells are drawn in an order that keeps all of them
  (far centres first where nothing decides); where panels pass through
  each other or the pairs cycle, the screen is split into quarters on the
  pixel grid until each is decided or one pixel, which takes the nearest
  at its centre. A tile drops everything under the last opaque cell
  covering it all, so an intersection costs a few primitives per pixel
  along its line, and a scene with none paints as one tile.
- Cursor: while space holds the pointer (Grab) or an app locks it
  (`Space::cursor_shown`), the host asks the platform for `CursorIcon::None`
  and draws its own in the scene, last and over everything, as a quad facing
  the head with its hotspot on the cursor's point (`Space::cursor_at`). Once
  the grab lets go it draws none and asks the platform's own cursor for the
  icon it would have drawn, since the platform then moves the pointer. When
  the cursor's point is out of the view (off the screen or behind the eye),
  it draws instead a marker inside the edge (`Space::cursor_away`,
  `cursor::Look::Away`): `beui_core::icons::ICON_MOUSE` with an
  `ICON_ARROW_UP` turned toward the cursor, on the line from the view's
  middle toward where the cursor projects, `cursor::AWAY_INSET` (36) points
  inside the edge, just in front of what it hits there; the arrow's angle
  is kept in `AWAY_STEPS` (90) steps a turn so the sprite repaints only when
  it moves a step. It goes only when the pointer leaves the platform window. Its size grows with
  distance (`Space::cursor_scale`, metres per point) so it covers as many
  pixels as it has points at any depth. It shows the icon that the window
  under it, holding the press, or being carried or resized asks for
  (`Space::cursor_icon`), a resize, grab or pointing hand over chrome, an
  arrow over nothing. `cursor::Sprite` paints that
  icon with `beui_core::drawn_cursor::paint`, the same drawing the inspector's
  mouse simulation shows on a touchscreen, through a `Context` of its own with
  only the icon font, and the compositor draws it into a texture like a window
  only when its look (icon or marker) or the scale changes, under a
  `WindowId` of its own, and the paintings draw its last output over
  everything (`render::Frame::sprite`). A new
  icon repaints into the same texture with the same renderer; building a
  renderer for each icon cost about 30 ms a switch.
- Pointers: the mouse and each controller's laser are pointers of their own
  (`space::Pointer`: `Mouse` or `Hand(Hand)`), and each holds its own carry,
  resize, chrome gesture, hovered and captured window (`space::Held`:
  `Space::mouse`, and `held` on each `Space::lasers` entry), so both hands and
  the mouse can each carry, resize, reshape or press at once. A
  `WindowRequest::BeginMove` or `BeginResize` goes to the laser whose touch is
  down in that window, else to the mouse. Two pointers never hold one panel,
  but both hands' grips together scale and turn it (below).
- Lasers: controllers reach space as `Incoming::Controller`
  (`beui_core::runner::Controller`, per `Hand::Left` or `Right`): `Pose { aim,
  grip }` (`HandPose`, a position and an `xyzw` orientation whose -Z is where
  it points, in the world of the views, floor at y = 0), `Trigger` and
  `Squeeze` (0 to 1; a press at `laser::PRESS` (0.6), a release at
  `laser::LET_GO` (0.4)), `Lost` (no longer tracked: its touch is cancelled
  and what it held let go), and `Toward { window, at }`, which a driver uses
  to aim a hand from where it is (`Laser::resting` beside the head before it
  has a pose) at a window's point. An XR host sends a hand's pose, trigger and
  squeeze each frame. A laser is a ray from its aim pose (`laser::Laser`),
  smoothed by a One-Euro filter (`laser::OneEuro`, `MIN_CUTOFF`, `BETA`,
  `DERIVATIVE_CUTOFF`, timed from `LayerView::now`, the last pose fed again
  each frame so a hand held still settles, with frames asked for until it
  has), except that a change of more than `laser::JUMP` (0.15 rad or m) is
  taken at once. It acts as a finger, never a hovering mouse: nothing reaches
  the app until the trigger presses, which sends `Input::Touch` (a
  `TouchId` with `laser::DEVICE`, finger 0 for the left hand and 1 for the
  right) `Start` where the ray hits the nearest panel, focusing it (a press on
  nothing focuses none), then `Move` as the ray turns and `End` on release,
  all to the window it began in (off its panel, where the ray meets the
  panel's surface extended), so beui's touch handling applies: a drag
  scrolls, a long press opens the context menu, and two lasers on one panel
  are two fingers that pinch. The press freezes the ray it was aimed along
  (`Laser::frozen`) until the laser turns more than `laser::FREEZE` (0.02 rad)
  from it, so the jolt of pulling the trigger moves nothing and a tap lands
  where it was aimed. A trigger press on chrome works its part as a mouse
  press does (the move button carries along the laser). Squeezing the grip
  with the laser on a panel or its chrome carries the panel along the laser
  (`Carry::along`: the grabbed point stays on the laser's ray at the distance
  it was grabbed, `Carry::follow` each frame), with the mouse carry's turn to
  face the viewer, until the grip lets go; a carry a
  laser's touch began (a title bar's `BeginMove`) ends with the trigger
  instead, and the app gets the `End` where it grabbed. Squeezing the other
  grip on the panel one hand is carrying holds it with both (`Space::both`,
  `Both`): it then moves and turns with the line between the two grip
  positions (`Quat::from_rotation_arc` of where it began to where it is) and
  scales about its centre by how far apart they are (`metres_per_point`,
  between `chrome::SMALLEST_SCALE` and `LARGEST_SCALE` of where it began),
  until either grip lets go, which reports `Placed` and leaves neither hand
  holding it. Each laser is drawn (`Space::beams`, `render::Beam`), depth
  tested after the chrome, as a strip `BEAM_WIDTH` (4 mm) wide from the hand to
  where it hits a panel or its chrome, or `laser::REACH` (5 m) on a miss,
  faint at the hand, blue, and white while pressed, with a dot `DOT` times
  its distance from the eye across where it hits; the paintings draw them
  over the panels.
- Keyboard: while any laser has a ray, `Space::keyboard_attached` (its
  `Layer::keyboard_attached`) says no keyboard is attached, so tapping a
  field opens beui's on-screen keyboard as a panel (above); with only the
  mouse and keyboard it says nothing and no panel opens.
- Focus: like a desktop window manager, focus follows clicks, not the cursor
  (`Space::focused`, the main window's panel at first). A press on a panel's
  content or its chrome focuses that panel, as does starting to carry it and
  opening it (`WindowRequest::Open`), so a window popped out of the dock gets
  the keys at once; a press that hits no panel and no chrome focuses none.
  Windows reopened at their saved placements when the app starts are opened
  like any other, in the order the app asks, so the last one opened has the
  focus, as a desktop session restoring its windows leaves it with the last
  one mapped. A change is reported as
  `WindowReport::Focused` (the old window `false`, the new one `true`), so the
  app's document lets go of, and later restores, the keyboard focus inside
  that window (a window just opened gets its first focusable, or the dock's
  tab, see Windows and window hosts). While no panel is focused, key presses and text reach no
  window. Nothing is raised: panels have no stacking order in 3D, only depth.
  Closing the focused panel focuses the main one.
- Chrome: the host draws and handles controls of its own around every panel
  (`chrome.rs`): an L bracket just outside each corner, the window's title
  (its `WindowState` appearance title, which the `Window` component's
  `title` sets) in a pill centred above the top edge, or in its place,
  accent text on an accent pill, the window's `notice` while it has one (the
  `Window` component's `notice`, which the dock sets from its tab's
  `DockTabControl::show_notice`; `chrome::Title`), cut short with an
  ellipsis to fit between the top brackets, and under the bottom edge
  a pill with a move button (`ICON_MOVE`), a rotate button (`ICON_ROTATE_3D`),
  a roll button (`ICON_ROTATE_CW_SQUARE`) and a curvature button
  (`ICON_SPLINE`). They keep one size to the eye wherever the panel is, as
  the cursor does: their lengths (a bracket's arm `ARM` 22.5 mm and
  `THICKNESS` 3.5 mm, `GAP` 6 mm out from the window; the pill `BAR` 20 mm
  tall, `BAR_GAP` 8 mm below, with `BUTTON` 16 mm buttons) are metres at `REFERENCE` (0.6 m) times
  `Fit::scale`, the eye's distance to the panel's centre over `REFERENCE`
  (`chrome::fitted`), which `Space` sets on each panel (`Panel::chrome`)
  every input and frame and changes only once it is off by more than `KEPT`
  (5%), so a head that moves a little never paints the chrome again.
  `Fit::title` is the title's width at that reference, measured with the
  app's `Context` when the title changes. They are faint (`FAINT`) except on the focused panel, which
  keeps them lit wherever the cursor goes until another press moves the focus.
  `chrome::layout` places the parts and `chrome::part_at` finds one at a
  window point outside the window's rectangle. Their hit areas leave no gap
  next to the window for a ray to fall through to a panel behind: each
  bracket's arms take the band from the window's edge out to the bracket
  (and `SLOP` beyond), the sides between the brackets take that band too,
  and the buttons, the slider and the title take everything between them
  and the window's edge, edge to edge with each other. Picking is by depth alone: the cursor's ray takes
  the chrome on a panel's extended surface when it is nearer than any panel
  (`chrome::pick`), focused or not, so the chrome of a window popped out in
  front of another takes the click over the content behind it. A driver's point outside its window picks too. Input
  on chrome never reaches the app (the window under it last gets
  `PointerLeft`), and the cursor shows what a part does. While an app holds a
  button, chrome is ignored. Dragging a side between the brackets
  (`Part::Resize`, lit while hovered or held) resizes the window from that
  edge as a `BeginResize` would (`Gesture::Resize` while it lasts), the main
  window too: its size in space is its logical size, reported as `Resized`,
  while the platform window keeps its own size and stays the mirror.
  Dragging the title carries the panel as the move button does.
  Dragging a bracket scales the panel
  (`Gesture::Scale`): `metres_per_point` grows or shrinks with how far the
  cursor goes along the diagonal from the opposite corner, which stays put,
  between `SMALLEST_SCALE` and `LARGEST_SCALE` of where it began, so the
  window keeps its points, gets no `Resized` and keeps its aspect. Pressing
  the move button starts the carry a title bar's `BeginMove` starts
  (`Space::begin_carry`, `Gesture::Move` while it lasts, for the cursor and
  the held look), ended by letting go. Dragging the rotate button turns the
  panel about its centre as seen from the eye (`Gesture::Turn`,
  `chrome::Sight`, `chrome::turned`), like a trackball: when the drag begins
  it takes the sight line from the eye to the panel's centre, the head's up
  made perpendicular to it (the sight's up) and their cross product (the
  sight's right, along the screen's right). The aim's change (the cursor's ray; see below) from
  where the drag began is measured on the screen: its direction is the
  change of the ray's point on the head's image plane, its size the angle
  between the rays, times `TURN_GAIN` (2). Its horizontal part turns the
  panel about the sight's up and its vertical part tilts it about the
  sight's right, tilt first, so the edge nearer the eye follows the drag on
  screen, a drag along the screen's axes turns about just one of them, and
  a panel's edge level to the view stays level, wherever the panel is on
  screen and however the head is rolled or pitched. For a panel straight ahead of a
  level head and facing it, that is yaw about the vertical and pitch about the panel's own
  horizontal axis. With the mouse the cursor stays on the button meanwhile:
  `PointerMotion` swings the gesture's own aim (`Gesture::Turn::aim`, as it
  would swing the cursor) and the turn follows that aim, while the cursor is
  put back on the button's point (`grab`) as the panel turns, so the mouse
  turns the panel without the cursor wandering off, and lets go on the button.
  An absolute position (the platform's own pointer, not grabbed, or a driver)
  aims it directly. A laser's turn follows its ray. The panel never rolls
  however the cursor circles. Dragging the roll button rolls it about its
  normal through its centre (`Gesture::Roll`, `chrome::roll_angle`,
  `chrome::rolled`) by the change in the cursor's angle around the centre on
  the panel's surface as it was, so the button, below the centre, stays under
  the cursor like a dial's knob, with the head rolled too. Scale, turn, roll and slider drags report
  `Placed` when let go. A carry later turns the panel back to face the viewer
  upright to the view (`carry::facing`), so it overrides rotation and roll. The curvature button
  opens a slider in the pill (`Chrome::slider`), and clicking or dragging
  along it sets `bend` from 0 to `MOST_BEND` about the panel's centre. Scale,
  roll and slider drags map the ray onto the panel as it was when they began.
  Each panel's chrome is a display list (`chrome::output`, icons laid out
  with the app's `Context`, which `Setup` provides) drawn into a texture of
  its own only when its `Look` changes, at as many pixels per metre as its
  window's scale gives the window (`chrome::pixels_per_point`, no more than
  `LARGEST_TEXTURE` texels a side) with its icons rasterised at that
  (`Context::rastered`), so they sharpen with the window instead of
  stretching, on the extended surface `LIFT` in
  front of the panel, after every panel.

### The on-screen keyboard

beui has an on-screen keyboard of its own, modelled on Android's Unexpected
Keyboard, for a screen with no keyboard attached (`block-app --session` on a
tablet) and for anything a person wants to type with a pointer. Each key types
its middle when tapped and what its corners show when dragged toward them; a
few keys also take the four straight directions (the arrows on Compose, the
cursor slider on Space, word selection on Backspace). Circling up, right and
down types the key's capital. Shift, Ctrl, Alt, Meta, Fn, superscript and
subscript latch for one key when tapped, lock on a long press, and apply to
every key pressed while they are held; Shift's corner is caps lock. Fn and the
two scripts change the labels as well as what the keys type. Dragging left from
Backspace selects a word per step (Ctrl+Shift+Left) and deletes the selection
when let go; dragging along Space moves the caret a letter per step. Held keys
repeat. Compose combines the next keys through X11's compose sequences. While
it composes, each key shows what it would do next (`compose::preview`, which
`KeyCapHandle::previews` holds per slot): a letter that finishes a sequence
shows its result in its place, one that continues a sequence stays as it is, and
one that leads nowhere fades, as do keys that take no part (Esc, the modifiers,
the layers and Compose itself stay); a key with nothing left on it fades whole.

It is three pieces:

- `beui::keyboard::unstyled` (crate `beui-keyboard`): the layouts as Rust data
  (`layout.rs`: `QWERTY`, `GREEK`, `NUMPAD`, built with `KeyDef`'s const
  builders; there is no user configuration), the Fn, superscript and subscript
  tables (`tables.rs`), the compose table (`compose.rs`, generated from X.org's
  `en_US.UTF-8/Compose`), and `Engine`, the pure gesture and modifier state
  machine its tests drive (`engine/tests`). The `SoftwareKeyboard` component
  lays the keys out in rows of `Percent` weights inside one forwarding
  `Interactive` with `press_focus=false`, so every finger reaches the engine as
  its own pointer and pressing a key never moves the focus. It hands each key's
  appearance to a `cap` render function and the emoji layer to `emoji`.
- `beui::keyboard::styled::SoftwareKeyboard`: the keys painted with theme tokens under a
  separator along its top, a bar with the close button, and the emoji panel (`emojis` grouped as the
  `emojis` crate groups them, leaving out what the fonts cannot draw).
  `install_keyboard` puts a `KeyboardWindow` on a document as its
  `KeyboardHost`, which `beui::reactive::build` does for every document.
- `beui_core::software_keyboard`: whether it shows and where. A context shows
  one only once `Context::enable_keyboard` has been called, which the runner
  does on its first frame unless its `Platform::shows_keyboard` says the
  platform has an on-screen keyboard of its own (Android, the web, a plugin's
  host). `Document::show` (and the inspector, around
  the app it shows) calls `show_with_keyboard`, which takes the keyboard's
  height off the bottom of the window the focused field is in (the host's
  last focused window when nothing is focused, else the first open one when the
  main window is not shown), the way the
  mouse simulation's strip is taken: that window's content and overlays are laid out
  above it, and the keyboard document paints into that window
  (`Context::draw_into`) and hears the pointer in that window's coordinates
  (`InputState::seen_from`). The keyboard covers a share of that window's
  height, `DEFAULT_SHARE` until `Context::set_keyboard_share` says otherwise
  (at most `MAX_SHARE`; block-app sets it from the display settings' on-screen
  keyboard height), and its keys stretch to fill it. A platform's own keyboard
  ignores it.
  The keyboard is a document of its own, shown after
  the app with the pointer only while a press began on it or the pointer is
  over it, and with no keys. Where windows open (`Context::opens_windows`, a
  host such as space) it takes nothing off the screen: `KeyboardHost::show`
  gets a `WindowId` the context keeps for it, and `KeyboardWindow` shows the
  keys in a `Window` of that id with `WindowRole::Keyboard`
  (`keyboard::WINDOW_SIZE`), which the core closes with
  `WindowRequest::Close` when the keyboard closes. The pointer counts as on it
  while its window is the one the pointer is in. The keyboard's window reports go
  only to the document that holds that window (`Context::take_window_reports`
  takes those a filter keeps), so the keyboard's document hears its own
  window's `Rastered`.

There is one way to ask for an on-screen keyboard whatever shows it:
`toggle_keyboard()` (or `Context::toggle_keyboard`) counts up
`WindowRequests::keyboard_toggles`. Where beui's keyboard is enabled it opens it
until it is closed; otherwise the platform answers the change in
`Platform::show_window`: Android shows or hides its keyboard, the web focuses
or blurs its hidden textarea, and a plugin passes it to its host as
`WindowRequests::keyboard_toggles`, where block-app toggles its own. The
desktop bar's keyboard button, the inspector's Sim tab button and the mouse
simulation's keyboard button all call it. While the platform reports that no keyboard is attached
(`WindowReports::keyboard_attached`, which only `beui-platform-drm` sends, from
libinput devices that have an A key; every other platform counts as attached,
and a layer may override it, as space does while controllers are active),
it opens whenever the focused field's `ImeArea::keyboard` turns `Shown` or its
`asks` changes, and closes when the focus leaves for something that types
nothing, except where it is a window of its own (`Context::keyboard_in_a_window`
where windows open, which only space sets): there the person put it somewhere,
so it stays open until its close button or a toggle closes it. Its close button (`close_keyboard()`) closes it until a field asks
again, so tapping a field brings it back. Its keys reach the app as a keyboard
would: the engine makes `Input::Key`s with evdev codes and real modifier
presses (`beui_core::input::typing`, which `drive type` and `drive key`
use too) and `Input::Text` for what no key types, `Context::type_events` puts
them in `FrameOutput::typed`, and the runner pushes them as the next frame's
input, so a beui field, a plugin and a Wayland program all take them
the same way. Paste as plain text asks for a `PasteRequest::Typed`, whose
answer the runner types out key by key. The Inputs page of beui-demo shows it in place, and the
inspector's Sim tab has a button that opens it over the inspected app.

### The inspector

Ctrl+Shift+I in a standalone beui window opens the node, component,
accessibility, and performance inspector. Its document is a `dock::styled::Docking` that fills
the window, and the inspected app is one of its tabs: an App tab, in a pane split off the left of
the tools (Beui, Comp, A11y, Perf and Sim) with the tools 340 points wide, whose body is an
`Embed` the app's document is laid out in. So the tools can be dragged beside the app, under it,
beside one another or into floating windows, and the splitter sizes the app. A window narrower
than `NARROW_WIDTH` stacks the dock, so the app and the tools take turns behind the stacked
bar, picking shows the App tab, and a pick returns to the tool shown last.

The app is drawn under the inspector: `InspectorTools` shows the inspector's document first, so the
App tab's rectangle is this frame's, sets its painted layers aside (`Context::set_aside`), shows the
app in that rectangle and the highlights over it, then puts the inspector's layers back on top
(`Context::put_back`), which keeps its menus and windows over the app. The App tab is therefore
`see_through` (see Docking and windows), and the pointer is kept from the app while it is over one of
the inspector's overlays or a press that began on one is held. Each tree tool keeps
its own rows, and the inspector collects rows only for the tree tools on show. The tree tools pin a
Pick toggle (`inspector.pick`, an icon pin, checked while picking) to the dock's bar, and every tool
pins Close (`inspector.close`), so the inspector has no header of its own.
Its Comp tab lists the
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
keyboard button calls `toggle_keyboard()`, so it opens whatever on-screen
keyboard the platform has (see The on-screen keyboard); fingers that land on
beui's keyboard pass through to it as touches rather than moving the cursor.

The strip takes its room out of the window rather than
covering it: `Document::show` asks the simulation how tall it is, trims that off
the bottom, and lays the inspector out in what is left,
the way a phone's keyboard pushes a page up, and beui's keyboard opens above
it. Nothing is drawn over content that is still live, so the bar is opaque.

### Responsive design mode

The Sim tab's "Responsive design mode", or Ctrl+Shift+M, lays the document out
in a screen of a chosen width and height in points, the way a browser's
responsive design mode does. A toolbar above the app picks a device preset,
types the width and height, rotates the screen and sets its zoom: Fit shrinks
it to the App tab but never grows it, and a percentage draws
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

### Back gesture

The Sim tab's Back gesture slider plays Android's back gesture into the
document, so a `BackSlide` can be tried without a phone: dragging it starts the
gesture and follows it, reaching the end goes back, and letting go before then
cancels. It calls `Document::back` directly, the way the Android runner's events
do, so it reaches the document even while the inspector holds the keyboard.

### Filters

The Sim tab's Filters section puts a blur, a contrast reduction, and the
colour vision simulations over the shown rectangle. They are independent of the
screen reader simulation and of each other, and they need no cooperation from
the document: they are a post-processing pass in `Renderer`, so whatever a
document paints - a plugin's beui pane included - is filtered the same way.

`Context::apply_filter` is what turns them on. It takes a `Filter` (the region
in points, a blur radius, a contrast multiplier, and a `ColorVision`) and wraps
everything painted so far - since the innermost `Context::clipped` or
`Context::scaled` it is called in - in a filter group (`Group::Filter`), so
what was painted before the call goes through the filter and what is painted
after it lands on top of the result untouched. Filters are groups like any
other, so a second call filters the first one's result again rather than
replacing it. The wgpu renderer draws each filter group into a scene texture of
its own, starting transparent, filters it and lays it over what lies beneath; a
filter at the very bottom of the frame holds the background too, so it filters
the whole picture. The inspector calls it once a frame,
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
the filter's own region, once for each filter. The scene textures and the blur chains are retained the
way the frame is, and every pass is scissored to the widened rectangle: what
lies outside it was left correct by the frame before, because the reach bounds
what a change can touch at every level. Each damaged rectangle is drawn through
the whole chain as a pass of its own. A filter that changed - a new radius, or
one switched on or off - damages the region it covers and nothing else: the
`Context` reports the regions of the filters that differ from the frame before,
and the renderer adds those of the filters it prepared differently. A scroll is
copied unless what it moves overlaps a filter's region.

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
  resolving. A test id is one word. It is a property of the node like its
  accessibility: the node's accessibility node carries it as AccessKit's
  `author_id` (several ids joined by spaces), and a named node with no role or
  text of its own still gets one, a `GenericContainer`. So test ids reach
  `FrameOutput::test_id_rect`, `drive` and a plugin's description by the same
  incremental path as the rest of the tree, and only where the tree shows the
  node: not under a closed overlay. `Document::find_test_id`
  finds the node itself.
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
slot asks for that takes one, so `<List><Text>"hi"</Text></List>` needs no
ceremony. Writing
`@sizing` on the child of a slot that does not size its children is a compile
error rather than an attribute that quietly does nothing.

A `Vec` of children does not fill a slot. Children are written out as tags, and
however many of them a collection asks for comes from a `ForEach` over its keys
inside the same `view!`, so the sizing, the keying and the reconciliation all
stay in one place:

```rust
<List spacing=4.0>
    <Text style=TextStyle::HEADING>"Cards"</Text>
    <ForEach keys={card_ids()}>|id: Uuid|
        <CardRow id />
    </ForEach>
</List>
```

A slot holds its children in runs rather than one flat list, so a child can
stand for none, one or many of them and change how many as it goes: that is how
a fragment written among siblings takes the places between them, and how
`Show`, `Dynamic`, `Keyed` and `ForEach` fill a parent they do not own. `List`,
`Stack`, `Scroll` and `Canvas` all keep their children that way.

Use `Frame`'s `width` and `height` props to constrain a component's own size,
`Extent::at_most(..)` for a box that fills the room it is given but stops at a limit, and
`@sizing` to describe how it participates among siblings in a `List`. Say a
width once: a `Frame` nested inside one that carries the width
is laid out within it, so the outer box is the only place the number belongs.
`Container`, `narrower_than` and `shorter_than` provide container-responsive
state; `unstyled::Stack` and `styled::Stack` switch between a row and a column
without rebuilding their children.
`unstyled::FirstFit` is "this, or that when it does not fit", built on
`measure`: it holds its children built and shows one through a `Portal`. It
asks for the width its first child last measured at, up to what it is offered,
through a zero-height `Frame` beside the child it shows, and that frame's
measured width is its room. Every child but the last is laid out in a
horizontal `List` that gives it its natural width or the room, whichever is
wider, so the child is shown across the room and its own measured width tells
whether it still fits; the last is shown as it is. It moves on when the child
it shows no longer fits (the room narrowed or the child grew), and goes back
to an earlier child when the room reaches the width that child last measured
at, so in a row with `.shrink()` it narrows and widens with the row. It learns
a hidden child's width only by trying it, so one that shrinks while hidden
comes back when the room next changes. Each try takes a layout, all before the
frame paints. The others are held by no portal, so they paint nothing and take
no input or Tab stop. That is how `styled::Tabs` and
`styled::SegmentedControl` become a `Select` of the same options, sharing one
`selected`, when their row of options does not fit: from the room they are
given, not from a breakpoint.

### One layout per frame

A frame runs in fixed phases: input, then layout, then paint. Input is
dispatched against the rects the previous frame painted, which is what the
reader was looking at when they clicked. Events are dispatched once, in the
order they arrived, pointer and keyboard interleaved, and the effects an
event's handlers woke run after that event and before the next one, so a key
typed just after a click reaches whatever the click focused.

Effects never flush inside the dispatch of one event or inside the layout walk,
with one exception: a node being laid out may build its own subtree there, and
the effects that build owns run on the spot. `Document::build_within(node, ..)`
does it: it lends the walk's own `&mut Document` to the build, so the build's
`with_document` calls reach the document through that one reference, and it
stops anything the build invalidates from marking the node or its ancestors,
which are in the middle of being laid out. The build flushes with
`ScopeContext::flush_owned`, which runs only the effects owned by that scope
and the scopes under it; a write the build makes to a signal outside its
subtree wakes that signal's effects once the walk has finished, which is what
keeps a build confined to its node.

Layout is a function of the tree and the constraints it is offered, so a new
window size lays the tree out again from its root and every
node offered the space it had keeps its measurements, placement and paint; only
a new scale, new fonts or a new context lay out everything. One node builds
during it:

- `VirtualList` shows each row its window reaches that it does not have yet,
  recycling a released row or building one,
  as the walk reaches it, measures it and carries on, so one walk fills the
  window. A list laid out at the length it measured follows that length as
  its rows measure, rather than stopping at the estimate it was given, and
  rows that measure shorter at the end of the list pull in rows above the
  window to take their place. When its length changed it asks with
  `Document::invalidate_after_layout` for the next layout to measure it again.

### Measurement

Where a node was laid out is asked one way, the same on every core
(`beui-view/src/measure.rs`, re-exported by `beui::reactive`):

```rust
pub fn measure(node: NodeId) -> ReadSignal<Option<Measured>>;
pub fn measure_text(node: NodeId) -> ReadSignal<Option<TextGeometry>>;

pub struct Measured {
    pub rect: Rect,
    pub size: Vec2,
}
```

Both are `None` until the node is first laid out, and again once it is
unplaced. `Measured::rect` is in the window's coordinates, the space
`PointerPress::pos` is in, for hit tests and placing things over the node;
`Measured::size` is the size the node was laid out at in its own units, before
any zoom, for content that builds from its own size, so a `Container` in a
zoomed `CanvasItem scales=true` answers the same at every zoom. Each node has one signal per function, made on the first call and
dropped with the node, so a handler may call `measure(node).get_untracked()`
as often as it likes. `TextGeometry` is a snapshot of a `Text`'s last layout:
`bounds()`, `caret_rect(index, width)`, `index_at(pos)`, `inline_at(pos)`,
`lines()` (each a `LineBox` of its byte range, its rect and its baseline's y,
in the window), and `scale()`, the scale of the space it sits in. Offsets are
UTF-8 bytes of the text's content, and every core counts them the same way:
a string or `Span` holds its bytes; a `glyphs` entry is drawn over the bytes
it names and counts as them (a core may draw it in their place, but takes no
room for it); an `Icon`, `Gap` or `Break` holds none; an inline `TextItem`
stands for its `text` (U+FFFC, three bytes, when it has none), which is how a
text area's list marker covers the `- ` it draws, and a `TextItem` given `at`
holds none. The test `text_geometry_counts_utf8_bytes_and_only_the_bytes_inline_children_stand_for`
pins this down. `measure_text` stays `None` for a node that is not a `Text`.

Text that is not in the tree is measured with `measure_line(text, font)`: the
width of `text` set on one line in `font` (rounded up to a pixel, as a `Text`
measures it) and the font's line height, as a `Vec2`, or `None` before the
document has fonts. It answers at once on both cores (the HTML core from the
browser's font metrics), so a component can size what it builds from text it
has not built yet: a text area's line heights, its gutter's digit width, how
far a quote's `> ` hangs into the margin and the shared widths of a markdown
table's columns, most of whose rows a `VirtualList` never builds. Read
`use_font_revision()` (and `pixels_per_point()`) in the memo that calls it, so
it measures again when fonts load or the scale changes.

Callers must cope with `None` and with an answer that arrives a frame late:
that is the contract the HTML core, which measures after the browser lays
out, can keep. The native core answers in the same frame: `Document::report_laid_out`
writes every signal that changed after each layout of `update_layout`'s loop,
so an effect that reacts by changing the tree is laid out again before the
frame paints, and a scroll moving a measured node only runs the effects that
read it. `beui::testing::delay_measurement(true)` (or
`BEUI_DELAY_MEASUREMENT=1` for a driven program) makes the native core publish
measurements at the start of the next frame instead and ask for that frame, so
headless tests take the late path. The beui tests named
`..._when_measurement_comes_late` run key components both ways through
`prompt_and_late` and check they settle alike.

A component that builds from its own size measures its root after building it
and writes a signal its content reads:

```rust
let width = create_signal(0.0);
let columns = view! { <List direction={..}>...</List> };
let measured = measure(columns);
create_effect(move || {
    if let Some(measured) = measured.get() {
        width.set(measured.size.x);
    }
});
columns
```

Keeping the last answer when it turns `None` keeps a hidden panel from
rebuilding for a size of zero. Content that is placed by the measured size
rather than only choosing a variant (the dock's panes, laid out from its
tree's size) is built once the first answer is in, not at a guessed size first:
a layout at a size nobody sees still moves scroll anchors and reports scroll
positions. Content that changes the layout from its own
size takes one more layout per level of nesting, all in the same frame (a
frame lays out at most eight times); give such a component its room from its
parent on the axis it reads, since a size it decides itself is what it would
then read. A component that only needs where on itself it was pressed reads
the `local` position pointer events carry instead. `unstyled::Container` is
this pattern with a context: `narrower_than` and `shorter_than` answer for the
nearest one.

The rule these rest on is the one container queries settle on: a component
may query the space it was given, but must not be the thing that decides that
space on the axis it queries.

Everything else layout decides is told to the tree after the walk: a `Scroll`'s
position, `component_placed`, `measure` and `measure_text` for watching other
nodes, and `Document::on_laid_out`, the callback block-app waits for its plugins
in. A node with something to report queues it with
`Document::report_after_layout`, and one whose own size changed while it was
laid out with `Document::invalidate_after_layout`. The reports run together in
one entry; whatever they changed is laid out again, which only walks the
nodes that changed and their ancestors, until nothing changes, so a frame is
never painted from a layout the tree no longer has. A frame lays out at most
eight times: when the reports after the seventh still change something, the
eighth layout holds its own reports back for the next frame, the frame is
painted from it and asks for another, and debug builds log what was still
changing. Tests fail there instead: beui-core's own, and every test run through
`block-ui-test`'s `DocumentTest` or `BeuiTest`, which turn on
`beui::fail_unsettled_layout`. An open overlay whose anchor moved is placed
again after the walk, but only one the walk placed: an overlay under a culled
ancestor is not placed at all, so its anchor moving is nothing to report. Each of those extra layouts is a cost the frame pays, so keep what a
report drives to painting where you can: a scrollbar's thumb is painted where
the position says rather than laid out, which keeps scrolling to one layout a
frame. Reach for a report only for what depends on how other nodes ended up,
such as `TreeReveal`'s button appearing once the selected row has scrolled
out of view.

Pointer callbacks are handed positions in the coordinates of the node that
hears them as well as the document's: `PointerPress`, `ScrollGesture` and
`ZoomGesture` carry `local` and `size` beside `pos`, and a drop target's
`DragPoint` is in its own coordinates. A component that only needs to know
where on itself it was pressed reads those rather than its rect. `pos`, rects
(`measure`) and text geometry (`measure_text`) are in
the window's coordinates, through every space's scale as well as its
translation; `local`, `size` and a `HitRegion` are in the node's own
layout units, so a node inside a zoomed `CanvasItem scales=true` hears a press
the same as it would unzoomed (guides/pan_and_zoom.md). A length of the node's
own (a padding, a radius) compared with window positions is multiplied by
the space's scale first (`TextGeometry::scale()` for a text).

`Document::on_interacted` runs a callback once the frame's input has been
dispatched and before anything is laid out, for an app that wants work started
by that input under way while the document lays out.

Revealing something in a scroll is resolved inside the scroll's own layout:
`reveal_node` and `reveal_focus` leave a request on each enclosing `Offset`,
which lays its content out, finds the target (or its nearest placed ancestor)
and moves itself until the target is in view. A `Canvas` cannot move itself,
since its view belongs to whoever hands it one, so it takes the same request
and answers through its `on_reveal` callback: after each layout at a new view
it measures the target (or, when that does not fit, the focused text's caret)
against its own rect less the floating layers over it (a dock, a zoom pill),
and calls `on_reveal(delta)` with the least pan that brings it into view. It
keeps measuring at each new view the owner pans to, for up to a second and
only while the distance shrinks, so the owner can ease there a step at a time
(the infinite canvas pans 35% of each delta, or all of it under reduced motion)
and a view that also moved for another reason (an editor following its
content after a resize) is measured as it is, never panned twice. Besides
`reveal_node` and a resize while text has the focus, a canvas reveals text as
the focus enters it, so text placed at a canvas's edge is moved into view; a
scroll does not, as a long text area clicked partway down would jump to its
top. A node's parent is the node whose `children` lists it; the arena learns it
when that node changes, not from the layout walk.

### Update a document from outside its events

A signal may be written from anywhere on the UI thread: an event handler, the
host's frame code before or after `show`, a callback the host makes between
frames, a future the app polls. Write it directly:

```rust
value.set(next);
```

The effects that belong to a document wait for its next frame (see Zones in
the [reactive guide](reactive.md#zones)), and the write wakes the window, so
nothing needs to ask for a frame. Keep the signals the host writes next to its
`Document`. Do not rebuild the document to display new data.

`with_reactive_scope(document, f)` installs a document and enters its scope
while `f` runs. Use it for work that needs the document itself outside an
event, such as calling `with_document`, running a callback the user wrote as
if it were a click, or creating nodes, signals or effects in the document's
scope; a signal write never needs it.

## Use beui in a block editor plugin

A beui editor is a component. It implements
`block_editor_beui::BeuiApp`, whose `View` and the other views the host asks for
are trait components (see Components in traits and impls), and uses
`block_editor_beui::beui_plugin!`.
The type it names holds no state: the
framework builds the view once and keeps it. A plugin module is one beui app,
with one `Document` and one `Context`: every region the host shows of every
instance (a frame, a preview, artifact settings) is one of its windows, a
`Window` block-editor-beui opens from beui-platform-plugin's `Views` handle
with `views.placement(id)` and the region's size, and its content is the
region's view, so an editor only ever writes `View(editor)`.

```rust
#[component]
pub fn Counter(editor: block_editor_beui::Editor) -> NodeId {
    let counter = editor.block_content::<CounterContent>();
    let count = counter.field(ObjectId::ROOT, CounterModel::COUNT);
    let increment = move || counter.operate(CounterModel::add(1));
    view! { ... }
}

pub struct CounterApp;

#[component]
impl block_editor_beui::BeuiApp for CounterApp {
    #[component]
    fn View(editor: block_editor_beui::Editor) -> Root {
        view! {
            <Root>
                <Counter editor={editor} />
            </Root>
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
once at the top of the frame, and
`read` and `revision` track, so a memo or effect that reads them runs again
when the content changes. See the [reactive guide](reactive.md#blocks).

Nothing in an editor polls. A request the host answers (a file or block
pick, a fetch) is an `Asked` future, and `editor.then(asked, f)` runs `f`
with the answer once it arrives, unless the scope that asked has gone
(guides/adding_a_plugin_editor.md, Asking the host). What the host sends
unasked is recorded by the framework and delivered at the top of the next
frame as signals on the `Editor`: `drag()` and `files()` carry a drag or a file drop over the editor;
`audio()`, `history(block)`, `histories()`, `artifacts()`, `focused_block()`,
`web_view_events()` and `peers::<P>()` follow the host's state;
`block_types()` tracks the catalog; `placed()` is the content rect as a
signal. Work on another thread takes its `Waker` from `editor.woken()`,
whose signal ticks once the work has woken the editor, and an effect that
reads that signal collects the result.

A plugin's frame can open windows (a dock's floating window, the desktop plugin's).
Outside space and the desktop shell a `Window` is an overlay inside its
region's window. Where the host shows windows as its own (the screen request's
`windows`, `Region::windows`; guides/adding_a_plugin_editor.md), each becomes a
`EditorRegion::Window(w)` region. block-app keeps every such window in
`compositor/desktop.rs` (`Floated`): on the desktop shell it draws them on the
displays (`DesktopWindows`), and under space, where it says `windows` for every
frame region (`host::opens_windows()`), each is a `Panel`: a beui `Window` of
its own holding a `PluginRegion` of the window region (`PanelWindows` in
`compositor/region.rs`), opened where the plugin asked with its role, moved
and resized by space on `BeginMove`/`BeginResize`, and its `Placed` answered as
the window region's `placement`; closing the panel ends the region, which
closes the app's window. block-editor-beui places the children laid out in a
window on that window's region (`Editor::end_frame` places only the children
in the window it is given).

The counter editor under `crates/editors/counter` is the reference integration.
The [plugin editor guide](adding_a_plugin_editor.md) covers the manifest,
creation flow, host connection, and current beui plugin capability limits.

A template the manifest marks `"dialog": true` is made through `BeuiApp::Creation` instead, one
more component that the framework builds into the frame's window
and shows in the picker's creation dialog. It says what the dialog makes with
`creation.on_create(...)` and answers `creation.set_ready(true)` once it has been
filled in. Asking the host works there too, through `creation.then(...)`.

## Develop an unstyled component

An unstyled component owns semantics and interaction, not appearance, and has
no appearance defaults: colours, sizes, spacing, paddings, font sizes, scrims and
the renderers for anything it must draw to work (rows, triggers, a menu's
`MenuStyle`, a field's `TextInputStyle`, a text area's `TextAreaColors`, a
sheet's `stops`) are required props the styled layer fills from theme tokens.
Defaults that draw nothing stay: zero spacing, no scrollbar, no focus ring, a
transparent shade, a pass-through panel, an empty optional decoration. So do
behaviour defaults (the drag threshold, zoom limits, a slider's range, the
tooltip delay). Put it in
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
6. Return the root base node directly so component state and node modifiers
   attach to the node callers receive.

A catcher shows the `cursor` it is given while it is hovered or held, and
one given none leaves the cursor to the catchers around it, so a catcher
that only listens - for a secondary press, a wheel - does not undo the
I-beam of the text field it wraps.

Beside presses, clicks and hover, a catcher hears raw input: `on_pointer`
gets a `PointerInput` (the pointer, its button, `Down`, `Move`, `Up` or
`Cancel`, `pos`, `local`, `size` and `scale`, whether it is `inside` the
catcher, the modifiers, the time, and `settles`), and `on_wheel` a
`WheelInput` (`Scroll(ScrollGesture)` or `Zoom(ZoomGesture)`); each answers
whether it took the input. Gestures are recognised over those in
`unstyled::gestures`, not in the core: `scroll_wheel(axis, ..)` takes wheel
turns along an axis, `scroll_drag(axis, ..)` a settled finger drag as
`DragGesture`s, `autoscroll(..)` a middle-click autoscroll, `secondary_drag(..)`
a right-button drag or a two-finger hold as `SecondaryDrag`s, and
`pan_zoom(on_pan, on_zoom)` a middle drag, a two-finger pinch and pan (summed
over the frame through `after_input`, which runs a callback once the frame's
pointer input is dispatched) and a touchpad zoom. `first_of([..])` puts several
on one catcher, asking each in turn until one takes the input. So a vertical
wheel over a horizontal strip passes through to whatever is around it, and a
drag reaches the innermost catcher that scrolls that way. `unstyled::Scroll` is
`scroll_wheel`, `scroll_drag` and `autoscroll`; `PanZoom` is `pan_zoom`.

Where a catcher hears the pointer is declared, never asked at the moment of a
press: `hit_area`, `capture_presses`, `gesture_area`, `drawing` and each
`Claim` of `claims` take an `Area`: `Nowhere`, `Whole`, `Within(HitRegion)`
(rectangles in the node's own layout units, built with `HitRegion::rect`,
`with` and `without`, and allowed past the node's rectangle) or
`CaretHandles`. A component derives them as memos of its state (a `bool`
converts to `Whole` or `Nowhere`), so a core whose hit testing runs before
any handler, the browser's, knows them ahead of time.

Pointer input takes one path. Each mouse button and each finger is a pointer
of its own, and the document sees their downs, moves and ups in the order they
happened (`InputState::pointer_events`), so a press, a move and a release that
share a frame are three steps. A down is hit tested once (`interact::hit`):
the layers of `Document::layers()` are taken from the top, by the rules of
their policies, and in the first one the pointer lands on, the hit is the
topmost `Interactive` under it together with every `Interactive` around it,
its path. The same walk collects the layers the press is outside of, which it
then asks to close. Any other node is transparent, and so is an `Interactive`
at a point outside its `hit_area`, along with the catchers around it up to one
that captures or forwards; a passing layer whose content turns a point down
lets it through to what is beneath.

The pointer keeps what its down reached until it is let go. A modifier claim
(`claims`) is looked for first, from the top of the tree down and past
`hit_area`, and a claimed press reaches its claimant alone. Otherwise the press
goes up the path from the topmost catcher and stops at the first whose
`capture_presses` area holds it. Every catcher it reached that
handles presses arms: it hears `on_press`, `on_drag` wherever the pointer goes
(once in the frame of the press too), and `on_click` when it is let go over the
catcher, or for a finger, let go without having been dragged. The press then
focuses the innermost focusable catcher it reached, or clears the focus;
a claimed or captured press that reached no focusable catcher leaves the focus
where it was. A
secondary press reaches every catcher on the path that takes one. A middle
press is offered to each `on_pointer` on the path from the innermost out,
stopping at a catcher that middle-clicks or forwards; the first to answer
`true` takes the pointer and hears the rest of it alone. Any other pointer's
down, moves, release or cancel go to every `on_pointer` on its path (to the
claimant alone for a claimed press) until it lifts, unless one answers `true`
to the down, which takes it the same way.

A finger drag is settled when the finger leaves the tap slop, along the
larger of its two movements. That move goes along the path from the innermost
catcher with `settles` set to the axis: a catcher that holds drags that way
(`touch_drags`) wins it outright, else the first
`on_pointer` that answers `true` does, and every armed catcher that does not
hold that drag hears `on_cancel`. A claimed finger drag stays with its
claimant. A second finger, a long press and a lost window focus cancel the
press the same way, and the winner of the drag hears `Cancel` though the finger
stays down. A second finger landing while the first is held still is a
two-finger hold: the `on_pointer`s under it hear it as a secondary button,
which a move of the first finger cancels.

A catcher with `grabs_pointer` set hears every pointer event of the document
first, hovering moves and wheel turns included, and one it answers `true` goes
no further. Its `cursor` wins while it grabs, and a key press other than a
modifier, or `Event::PointersCancelled`, sends it `Cancel` (an Escape that does
is taken). `gestures::autoscroll` grabs while it scrolls.

Hover is the path under the mouse, worked out again every frame, so a catcher
hears `on_hover_change` when it joins or leaves the path whether the pointer
moved or the layout did. The cursor is the innermost armed catcher's, else the
innermost hovered one's. A wheel turn goes to the innermost catcher on the path
under where the pointer was when it turned whose `on_wheel` takes it, unless
a forwarding catcher comes first (one with `forwards_wheel=false`, such as the
on-screen keyboard, forwards only its presses and lets the wheel pass to what is
under it), and stays latched to it for
`WHEEL_LATCH_TIMEOUT` while the pointer holds still.

Parts a catcher draws outside its own rectangle, such as the touch selection
handles of `unstyled::TextArea`, take their presses with a claim on
`Area::CaretHandles`: the handles that the `Text` inside the catcher paints on
its carets, within reach of a finger below each caret, while the caret is in
the text's visible rectangle. Paint
order is the tree's order, so paint such parts from a node that comes after
whatever they must cover, or from a `CanvasItem` with `clip=false`, which a
canvas paints without cutting it to its own rectangle. Those handles are carets
with a handle, which `Text` paints after its text; a single-line area paints
them from an unclipped item after the text's. Where a handle sits is the same
on every core: `handle_center(caret, handle, scale)`, `HANDLE_RADIUS` and
`toolbar_anchor` (where a selection's toolbar opens clear of its handles) are
values in `beui-types`, which a component hit-tests with and a core paints from.

Do not put theme colors, fixed visual spacing, typography choices, or decorative
shapes in this layer. A new skin should be able to use the unstyled control
without undoing visual decisions.

State that belongs to the component's own handlers should be captured directly.
When callers, tests or host integration need to read it, the component exposes a
typed handle: a `Copy` struct named after it (`ToggleApi`, `SelectApi`), usually
wrapping its private state, whose methods answer what may be asked
(`checked()`, `selected()`, `option_button(index)`). The component takes it as
an ordinary prop, `#[prop(default = ComponentRef::new())] api: ComponentRef<SelectApi>`,
and calls `api.fill(SelectApi(state))` while it builds. A caller that wants the
handle passes its own `ComponentRef` (`<Select api=select ...>`, then
`select.get().selected()`), so the type checks that it fits; a styled component
that wraps an unstyled one takes the unstyled one's handle and passes it on.
`fill` also records the handle against the component's node, so a test holding
only a `NodeId` (from a test id) reads it as `document.api::<SelectApi>(node)`.
A method that needs to know where nodes were laid out takes
`tree: &impl Inspect`, which is what a test holds of a built tree (`api`,
`node_rect` and `is_overlay_open`, which `Document` implements). Never look
another component's state up by its node from inside the tree; take its
`ComponentRef` as a prop, as `TreeReveal` takes its tree's.

The styled and unstyled crates never name `Document`, `with_document` or
`NodeOf` outside their tests, so their components can be built on another
backend; `fix-rust-source` reports it when they do. They reach the tree they
were built into through `beui::reactive` (`beui-view/src/nodes.rs`). What
concerns one node is a method of the `TreeNode` trait on `NodeId`
(`panel.focus_is_within()`, `overlay.raise_overlay()`, `offset.drive_offset(..)`,
`text.text()`), which answers `None`, `false` or nothing for a node of
another kind or one that is gone. Where nodes were laid out is `measure(node)`
and `measure_text(node)` (see "Measurement"). The rest are functions: `focused_node`,
`set_focus`, `stacked_at`, `tree_local` for a value kept once per tree,
`use_preferences()` and `use_preference_layers()` as signals,
`use_preference_layer(source)` as a `Binding` that writes that layer, and
`use_font_revision()`, which changes whenever text may measure differently.
When a component needs something the document has and these do not offer, add
it there rather than reaching for the document.

State the user changes is a `Binding<T>`
prop that the component reads and writes, with no copy of it kept in sync; see
`unstyled::Toggle` for the pattern.

## Develop a styled component

Put a styled component in `crates/beui-components-styled/src/<name>.rs`, declare it in
`lib.rs`, and re-export its public API there. An interactive styled component
wraps the matching unstyled component, passes its `label` on for the unstyled
one to put in the accessibility tree, and renders the unstyled handle with base
visual primitives:

```rust
#[component]
pub fn Checkbox(label: Prop<String>, checked: Binding<bool>) -> NodeId {
    view! {
        <Toggle checked label>|handle: ToggleHandle|
            <CheckboxFace handle />
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
`ThemeStore` — one `Signal` per token. Bind a token straight to a prop with
`theme.accent`, or read tokens inside a memo that also reads interaction
state, so the control repaints when either changes. Because each token is its
own signal, a component wakes only for the colors it actually uses. Spacing,
sizes, strokes and radii are the tokens in `styled::tokens` and `Radius`;
write a component's private consts in them. Add a field to `Theme`,
with a value in every built-in theme, when a color is part of the theme rather
than unique to one component.

Styled controls must visibly expose keyboard focus, and must keep labels and
accessible roles stable when visual state changes. Use glyphs from `beui::icons`
in a `styled::Icon` inside a `styled::Text`; do not use Unicode characters as
ad-hoc icons. The [keyboard guide](beui_keyboard.md) records the expected
behavior for each control family.

### Buttons and rows

There is one `Button`. It takes a `variant` (`Secondary` unless given), a
`ButtonSize` (`Small`, `Compact`, `Regular`, or `Toolbar`, 30 points high and
square when icon-only), and `icon_only`, which shows only the glyph and turns the
label into the tooltip. `MenuButton` and `Popover` take the same `variant`,
`size` and `icon_only` and paint the same `ButtonFace`; `MenuButton` also takes an
`image` for a picture face, a `leading` node drawn before the label in place
of a glyph (a database choice's coloured mark, a status dot), and
`placeholder=true` while its label stands in for an unset value ("Set
status"), which mutes the label; it adds a
chevron to a labelled face unless `arrow=false`. A custom face over `unstyled::Button` reads `handle.icon_only` and
wraps itself in `<Tooltip label={handle.tooltip}>`.

A pressable row paints `RowFace`: a leading picture or icon (`image`, `glyph`, or
`leading=true` to keep the slot empty so a column of rows lines up), a `label`
and `detail`, and a `trailing` `RowTrailing` (`Chevron`, `Check` or `Radio`), or
`children` in place of the label and detail. Its `RowSize` is `Compact` for
lists, `Regular` for settings (whose label wraps), `Large` for a picker with
32-point pictures and `Touch` for 52-point rows in a sheet. A selected row
fills with `AccentSoft`, and with `AccentSoftHover` while the pointer is
over it, so it stays visibly selected under the pointer. `ListRow` (selection
and activation), `ActionRow` (an action, `Touch`), `SettingsLink` (a page of
settings, with a chevron), the menu sheet's rows and the launcher's rows are all
built on it.

A pick among pictures is a row of `TileButton`s: a square tile (`size`, `Xl` by
default) around any child, filled `AccentSoft` and ringed in the accent when
`selected`, with `outlined` for a hairline border at rest and `radio=true` for
radio semantics (`description` reads after the label). The rename card's icons
and the display settings' rotation picker use it. A colour pick is a row of
`SwatchButton`s.

`Banner` is an inline bar across an editor that says what failed (an import,
a load) until the user dismisses it: its `message` is an `Option<String>`, and
it shows nothing while that is `None`. Its test ids are `{id}` and
`{id}.dismiss`. `unstyled::Banner` gives it the alert role, an assertive live
region labelled with the message.

`ToolPill` is the raised, bordered, fully rounded bar a floating set of tools
sits in (a canvas's zoom controls, block-editor-beui's `BottomDock`): its
children in a row with arrow-key focus, scrolling sideways when the bar is
offered less than it needs. Placing it (a `Floating`, a margin) is the caller's.

### Text

Styled text is written in tokens, never pixels or colors. `styled::Text`,
`styled::Span` and `styled::Icon` take the same children as the base ones and
resolve their tokens into a base `Text`'s `font` and `color`:

- `size`, a `TextSize` on a scale from `Xs` to `Xl5`; `TextSize::px` is the
  point size, for the rare layout that must measure against it.
- `weight` (`Weight::Normal`, `Weight::Bold`), `family` (`Family::Sans`,
  `Family::Mono`) and `italic`.
- `color`, a `TextColor` role (`Default`, `Muted`, `Faint`, `Accent`,
  `AccentHover`, `AccentActive`, `OnAccent`, `OnDanger`, `Danger`, `Warning`,
  `Success`) resolved through the nearest
  theme, so the text follows a theme change. `TextColor::Custom(color)` is for
  colors that are data rather than design - a user's label color, a map
  marker, a terminal's ANSI palette - and a memo that picks a color by state
  returns a `TextColor`.
- `style`, a `TextStyle` preset: `BODY`, `CAPTION` (small and muted),
  `HEADING`, `TITLE`, `DISPLAY`, `CODE` (small and monospace) and `PARAGRAPH`
  (muted). A prop given beside a preset wins over it.
- `overflow`, as on the base `Text`: text wraps unless told otherwise, so a
  paragraph needs nothing, and a one-line label (a button's, a row's, a
  title in a bar) takes `overflow=Overflow::Ellipsis`.

A styled `Icon` is sized from the text it sits in, `ICON_SCALE` times its font
size, unless given a `size` token, and keeps `ICON_SPACING_EM` from its
neighbours, so an icon beside a label is one `Text`, not a row of two:

```rust
view! {
    <Text style=TextStyle::CAPTION>
        <Icon glyph=ICON_CLOCK />
        "Edited five minutes ago"
    </Text>
}
```

A lone icon is a `Text` holding only an `Icon`; give it `align=TextAlign::Center`
when it is placed in a box bigger than itself.

The icons are Lucide's, drawn from its font
(`beui-fonts/src/assets/icons/lucide.ttf`, committed unmodified from
the `lucide-static` npm package). They live in `crates/lucide`, which
`beui_core::icons` re-exports and which knows nothing of beui, so the block
catalog (`block-ui`) holds them too. An icon is a `Glyph`, its codepoint in the
font; only the list can make one, so a `glyph` or `icon` prop, which takes a
`Prop<Option<Glyph>>` (`glyph=ICON_SAVE`, or `None` for no icon), cannot be
handed text by mistake. The plugin protocol, which knows nothing of the font,
carries codepoints as text; `Glyph::from_codepoint` reads one back, and
`Glyph::as_str` is the text a painter lays out. The list,
`crates/lucide/src/list.rs`, is one line per icon: its `ICON_*` constant, its
Lucide name, its codepoint, and its tags from `tags.json` followed by the older
names (aliases) it shares a codepoint with. The `icons!` macro in that crate
turns it into the constants, `from_codepoint` and `ALL`, a `NamedIcon` per icon
in name order; `named` looks a name up in it, which is how anything saved holds
an icon, since Lucide renumbers an icon now and then, and an icon picker lists
and searches it (the workspace's appearance card). Search the list's tags for
what an icon is for ("duplicate" finds `ICON_COPY_PLUS`) rather than guessing
from names. Don't edit the list by hand: to update Lucide, unpack the package
(`npm pack lucide-static@VERSION`, then `tar xzf`) and run
`./scripts/buck run //crates/local-tools:bin -- icons PACKAGE` from inside the
repository, which rewrites the list from `icon-nodes.json`,
`font/codepoints.json` and `tags.json` and copies in the font. The list's
`@yeskeepfile` comment stops `./scripts/verify` deleting its header; a comment
holding that marker keeps every comment in its file, as `@yeskeep` keeps the
one comment line holding it. Text
is Inter Regular (`Inter-Regular.ttf` from Inter's release, unmodified), with
Hack for monospace and Noto Emoji as the fallback. crates/beui-fonts holds all
four: their bytes, which beui-font-freetype loads and the browser rasterizer
serves, and their file and family names (`FACES`), from which the HTML core
writes its `@font-face` rules; BUCK exports each file for web bundles. Bold
is synthesized from the regular face on both cores.

`TextStyleProvider` gives every styled text created within it a style its own
props override, the way a disabled section mutes everything in it. It is
context, so it reaches what is created inside it, which is not always what is
laid out inside it: text a render callback builds for an overlay takes the
provider around where the callback was written. A `Span` or `Icon` written
inside a `Text` takes what it does not set from that `Text` instead.

Raw pixel sizes and `Color32`s belong to base `Text`, for text that is drawn
rather than designed: labels on a zoomable canvas, a terminal, a game board.

### Colours

A colour prop on a primitive (`Frame` `fill`, `Text` and `Span` `color`, `Outline`,
`SpanStyle`, `TextMark`, `TextGlyph`, `SpanBackground`, `TextCaret`, `Overlay`
`scrim`, `Root` `clear_color`) is a `beui::Color` (beui-types), not a `Color32`. A
`Color` is a literal `Color32`, or a reference to a `ColorVar`, plus a small algebra the cores can both carry: `scale_alpha(f)` and
`mix(other, share)` (two unmixed colours). A `Color32` converts into a `Color`,
and a `Color32`, `Memo<Color32>` or `Signal<Color32>` passes for a `Prop<Color>`,
so code with literal colours is unchanged.

A `ColorVar` is declared, never named by a string: `beui::color_var!(pub SURFACE,
TEXT)` declares constants whose identity is where they are declared (module path
and item name), so a variable is reached only by importing it and two crates
cannot collide. Its CSS name (`css_name()`, `--v-` and a hash of that path) is
the same in every build, so a page prerendered on the server hydrates to the same
names; `name()` keeps the readable name for the inspector and `describe()`
(`var(SURFACE)`).

Whoever references a variable defines it, with a value per `ColorScheme`
(`Light`, `Dark`): `define_colors(definitions)` (`beui::reactive`) takes a
`Prop<ColorDefinitions>` (a map from `ColorVar` to `SchemeColors { light, dark }`)
for as long as the calling scope lives; later definitions of a name win. The core
and unstyled know no colour names. Native resolves a reference when it paints, for
the scheme of the document's theme preference (`ColorScheme::from(ThemeKind)`:
`Light` and `EPaper` are light), and repaints everything when the scheme or a value it
already held changes. The HTML core writes `var(--NAME)` (and `color-mix()` for the
algebra) and puts the definitions in a `<style>` inside `.beui-root`: the light
values on `.beui-root`, the dark ones under `@media (prefers-color-scheme: dark)`,
and both again under `.beui-root[data-scheme=light|dark]`, which the core sets
when the app chose a theme (`beui::html::choose_theme`). So a page prerendered by
`render_to_string` already shows the browser's scheme before any script runs, and
hydration computes the same references and changes no colour. So code must not
choose a colour by reading the theme's kind or the scheme: the server and the
browser can disagree. Choose a reference whose definitions differ per scheme
instead. Code that needs the
`Color32` itself (a `Drawing`'s painter, image tints) calls
`resolve_color(color)`, which tracks the definitions and the scheme;
`color_scheme()` reads the scheme.

`styled` is the one place theme colours live. `styled::Theme` holds the colour
tokens as `Color`s. The built-in themes are `Theme::DARK`,
`Theme::OLED` (dark on pure black, so OLED screens leave most pixels off),
`Theme::LIGHT`, and `Theme::EPAPER`, one for each `ThemeKind`, with literal colours;
`Theme::of(kind)` picks one, and a theme's `kind` says which it is, for code that
must tell them apart without comparing colors. Styled components that no provider
covers use the document's theme, whose tokens are references, one variable for each token
and each syntax colour, defined with the light
values of `Theme::LIGHT` (`EPAPER` when that theme is chosen) and the dark values of
`Theme::DARK` (`OLED` when chosen), so the scheme picks between them on either
core. `Theme::referenced` and `Theme::definitions` build those. What dims the app
under an overlay is a theme colour too: `scrim` under a dialog, sheet or dock
dialog, `scrim_soft` under the command palette and launcher, which leave the app
readable, and `scrim_strong` under `Fullscreen`.

Colours of a caller's own with a value for each built-in theme (the choice colours
in block-editor-beui, for example) follow the theme the same way:
`define_themed_colors(definitions)` takes a `fn(light: ThemeKind, dark: ThemeKind) ->
ColorDefinitions` and defines it once per tree for the kinds the scheme would pick,
and `theme.themed(var, |kind| literal)` (on the `ThemeStore` from `use_theme()`)
gives the reference under the document's theme and the literal for the provided
theme's kind under a `ThemeProvider` (the `referenced` field of `ThemeTokens` says which).
`Theme::of(document.preferences().theme)` reads the literal theme, for example to
pick an app's clear color.

`ThemeProvider` overrides the theme for the subtree written between its tags and
follows the `Prop<Theme>` it is given; a built-in theme given this way is
literal, so it looks the same in either scheme:

```rust
view! {
    <ThemeProvider theme={theme_signal}>
        <Settings />
    </ThemeProvider>
}
```

## Develop a base component

Each primitive is declared once, in `crates/beui-view/src/components/<name>.rs`,
whose `#[component]` calls `crate::core::<name>(..)`; each core has a body:
`src/native/<name>.rs` over `beui-core`'s `Document` (below), and
`src/html/<name>.rs` over `beui-core-html`, which builds one element per
primitive and turns props into inline CSS. Hooks follow the same pattern:
`src/reactive.rs` names what both cores provide, from each core's
`reactive` module. On the HTML core a container's own behaviour lives in a
`beui-core-html` module its body hands the elements to: `scroll` (`Offset`:
an `overflow: hidden` element moved through `scrollTop`/`scrollLeft`, never a
transform, so the browser's own scrolls - find in page, `scrollIntoView`,
focus - are adopted as the offset; it anchors on the first element under the
offset and corrects by its drift, snaps to the end, and reveals), `rows`
(`VirtualList`'s window and row measurement; rows are flow between two
`data-beui-spacer` elements that anchoring skips), `canvas` (an inner element
translated to the view's origin, with `--beui-scale` scaling item positions and
`scales` items), `overlay` (a `<dialog>`: `showModal()` for `BlocksAll`, else
`popover="manual"`; CSS anchor positioning, measured with `resolve_rect` where
unsupported; scrim on `::backdrop`; Escape, outside presses and focus traps),
`portal`, and `text` (`Text`: a `div` with `white-space: pre-wrap` and a
default `line-height` of 1.15, as native's; it keeps the order of its children
and the bytes each stands for, so `glyphs` cut a run into a `span.beui-glyph`
whose `::before` draws the glyph over bytes that keep their room and copy as
themselves; `marks` are CSS Custom Highlights, one per colour, or positioned
boxes where `CSS.highlights` is missing; `carets` (blink, flag, handles) are
positioned boxes in an `aria-hidden` overlay at `caret_rect`; `hang` is a
negative `text-indent` with a matching `padding-left`, measured from where the
`hang` byte sits on the first line, so it holds only for a Text whose first
paragraph is the one that hangs; `Break` is `::after { content: "\A" }`, which
no browser copies). Marks, carets and hang need the browser's layout, so a
painting from `render_to_string` shows glyphs but not them; the shared scripts
(`editing`, `text_input`) exercise them. The HTML core's `TextGeometry` is not a
snapshot: its queries read the DOM when called (`caretPositionFromPoint`,
`Range` rects), mapping UTF-8 bytes to each text node's UTF-16 offsets, and it
compares equal until the Text's bounds or content change. Each registers a pass with `frame::on_layout`, which the frame
loop runs (up to `LAYOUTS_PER_FRAME`) before paint; `ResizeObserver`s call
`frame::relayout()` so a resize is corrected in the same frame. Shared modules
with a small core-specific part (`actions`, `animation`, `nodes`'s `TreeNode`,
`file_picker`, `file_saver`, `drag`, `window`) keep their logic in
`src/<module>.rs` and call `crate::core::<module>::*` for the rest. A primitive
the HTML core has no body for yet is an outlined pink stand-in there. unstyled
and styled build on both cores; under HTML, `Selectable` is the browser's
selection, a window is its overlay, `on_back` never fires and file pick and
save go through `<input type=file>` and a download link. How to test a body on both cores is under [Cores](#cores).

`run` and `hydrate` read the browser's preferences before they build, so
hydration starts from the scheme the prerendered page shows (Colours, above).
Browser paintings render in Chromium's light scheme unless the page chose a
theme.

Base nodes are the only layer that should normally mutate a `Document` directly.
Put the node in `crates/beui-core/src/base/<name>.rs` and register the module in
`base.rs`, and its component in `crates/beui-view/src/components/<name>.rs`. A
base implementation has three parts:

- A node struct containing its retained state and child ids.
- An `Element` implementation for measurement, layout, painting, interaction,
  child traversal, and inspector metadata. Its `remove_child` lets go of a
  child whose scope went while the node still held it, so a node holding one
  child keeps it in an `Option`; a `ChildHost` hands it to `remove_hosted`.
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
update the node's own retained state, and may build its own subtree through
`Document::build_within`, which is how a `VirtualList` builds the rows its
slice of the viewport calls for. Both receive `&mut Document` and are reached
through `beui_core::layout::measure` and `beui_core::layout::layout`, which
take the element out of the arena for the duration. Reach children through
those two functions rather than calling another element's methods directly.

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

`min_width` answers the least width the node can be laid out at without
breaking what must not break. It defaults to nothing; `Text` answers with its
longest run that cannot be wrapped when it wraps, a node that wraps one child
forwards the child's answer (a `Frame` adding its padding, or answering its
own fixed width), and a `List` adds up a row and takes the widest of a
column. A row reads it for its `Percent` and shrinking children only, and it
is memoised beside the measurements.

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
handlers, a cursor, a tab stop: only the accessibility tree hears of it.
`state_mut` is for state the pointer dispatcher keeps on an `Interactive` -
hovered, armed, panning - which nothing laid out, painted or published reads,
so no one hears of it; what that state changes reaches the screen through the
callbacks it calls. A field
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
`measure`, the rect `interact` is handed and a `CanvasView`'s
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
changed, when its size or the painter it is handed changed, when a child it
paints moved or was culled or uncovered, or when a repaint it asked for falls
due; otherwise the shapes it recorded last time stand, and its children are
only visited when something under them has to paint. Being laid out again is
not by itself a reason: a node whose paint reads something its own `layout`
works out says so with `Element::paints_from_layout` (an `Offset`, a `Canvas`,
a `VirtualList`), and a `Text` asks for its own repaint from `layout` only
when the lines it placed are not the ones it placed before. `paint` must
therefore be a function of the node, its rectangle, the painter and the
rectangles its own layout gave its children - anything else it reads goes
unnoticed when it changes. A scroll's new offset relays out its `Offset` alone,
from the place it was last given, rather than every node around it.

What a node records is a tree (`display.rs`): its own shapes, its children by
id and translation, and group nodes around them - `Group::Clip`,
`Group::Scroll`, `Group::Mask` (a fade), `Group::Transform` and
`Group::Filter` - which a painter opens (`with_clip_rect`, `shifted`, `faded`
with the space of the fade that opened it, `in_document`) rather than writing
them onto each shape or child: a recorded shape's own `clip` is always
everything, and `WindowOutput::shapes` folds the groups into it.
`PainterState::within` is what each group does to the painter inside it, so a
node visited without painting again hands its children the painters its tree
says they had. When a node paints again its new tree is set against its old one by
identity, never by comparing what it painted: its own shapes damage where
they were and where they are, except a `Drawing` that is the same drawing in the
same place, which damages only what it says it redrew; a child, matched by id, damages its old and
new place when it was added, removed, moved or restacked, and nothing when it
stayed; a mask whose fade changed damages its bands; and a scroll group, matched
by the space it scrolls, whose offset changed is a copy of what it showed rather
than damage. A repaint that paints the same thing still damages its shapes, so
a node that repaints without changing is fixed where it is asked to repaint.
Damage is held as at most four rectangles (`damage::Region`), each drawn as its
own pass, and two of them become one whenever their bounds are at most twice
the area they cover, so a large change spread over nearby spots is drawn once. A
`Drawing` whose content changed in part hands its `Drawing` node
`draw_gpu(Some(drawing.redrawn(region)))` rather than a new `Drawing`: only that region, in the
drawing's own coordinates, is damaged (the plugin host does this with the
rectangles each plugin frame reports it changed). beui's own tests, and every test that drives a plugin through
`block-ui-test` (which turns on `beui::verify_paint`), paint every frame again
from scratch and fail when the retained painting differs from it or changed
outside the damage. The second check (`paint/verify.rs`) sets the frame before
against this one node by node, matched by id: a node's own shapes, the fades
over it and its place among its siblings are what it shows, and every pixel a
difference touches, rounded out to whole pixels as a renderer repaints them,
must be damaged, or, inside a region the frame copied, must be what the copy
brought there. Painting from scratch runs every paint again, so a test that
counts paints skips the ones made while `Context::is_verifying_paint` is set.
With `BEUI_OVER_REPAINT=1` in the environment (or
`beui::detect_over_repaint(true)` on the thread that paints) they also report
every frame whose damage is more than four times the area of the shapes that
changed, to stderr and to `beui::take_over_repaints`.

A frame hands the renderer these trees, with the frame's own groups around them
(`Context::clipped`, `Context::scaled` and `Context::apply_filter` wrap what
they cover), rather than a flat list of shapes; `WindowOutput::shapes` flattens
them on demand, for tests and software rasterising. `Renderer` encodes a node's
shapes once and keeps their instances in a GPU buffer, only walks the trees that
reach the damage, and draws a scroll group through a translation and clip of its
own, so a scroll or a pan encodes nothing again. A scroll whose offset changed
reports its copy as `WindowOutput::moved`, and a host that keeps its last frame,
as `beui::run` does, copies that region by the scroll with `Renderer::shift` and
repaints only what the copy cannot supply - the rows it exposes, whatever
changed, and whatever does not move with the rows, such as a fade's bands or a
scrollbar drawn over them. Only one region is copied per frame: when several
scrolls moved at once the largest is copied and the rest are repainted (one per
window). The context keeps a copy only when its distance is whole pixels at the
window's own `pixels_per_point`, so a scroll snaps its offset to that window's
pixel grid; and in the main window only when nothing painted outside the
document (an inspector, a filter) overlaps it. Space copies each window's
`moved` inside that window's panel texture with `Renderer::shift` before
repainting its damage. `Repainting::repaint` on the `WindowOutput` covers the
moved region for hosts that do not copy: plugin surfaces, the DRM screens, and
the web canvas, which repaints its whole frame whenever anything changed.

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
repository keeps one test per file.

Every test that runs a document or an app runs it through `beui::testing::Stepped`, the way
a platform does: through the real `Runner` under one drive layer, on the headless window's `Simulation`
(`Stepped::new`) or on a `PlatformServices` the test supplies (`Stepped::on`), with a gpu-free
`testing::Screen`, the clock stopped, and one frame interval per frame.
`testing::Showing(document)` is a `Document` as an app. Its gestures are `drive`'s commands,
run by the same drive layer (crates/beui-platform-headless/src/testing.rs): `click(Target::id("wheel").at(offset))`, `drag`, `down`,
`up`, `hover`, `scroll`, `zoom`, `pinch`, `fingers`, `type_text`, `key`, `key_down`, `wait`
and `settle` are typed forms of them, a `Pos2` is a target too, and `ask(&["back", "0.5"])` sends
any other. Each settles as `drive` does, unless the test turned that off
with `settle_after_gestures(false)`, when it answers after the frame that took the input.
`frame(inputs)` pushes the `Input`s it is handed in points (`Incoming::points`) and paints one
frame, for a test of what a single frame does, and hands back its output. Like every input,
they go through the runner's intake, so a test sends what a platform would: an
`Input::Modifiers` before a key or a press it means to be held with (`Harness`'s
`pressed_with` sends a chord that way), `Input::PointerLeft` for the pointer leaving, and
`Input::Scroll(ScrollDelta::Pixels(..))` for a scroll in points. There is no way to hand the
runner an `Event` directly, and input reaches a `Context` only through a `Runner`:
`Context::run(frame)` paints one frame with no input, for drawing offscreen (the canvas's
export) or a test of painting alone. A test that stamps samples with times pushes `Incoming::Time`
through `runner().push` and then paints `frame(Vec::new())`. A failure prints the steps so far as `drive`
commands (guides/running/drive.md), which replay against a `:dev` app.

beui's own tests use `Harness::new(document)` or `Harness::sized(document, size)` from
`crates/beui/src/tests.rs`: a `Stepped` with settling turned off, plus what those tests
share: finding and measuring nodes, and driving the inspector and the dock.
Tests of the `Runner` itself take `testing::Screen` rather than a renderer of their own.

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
failure.

## HTML: strings and live DOM

`beui-html` builds ordinary HTML elements, laid out by the browser's CSS. It
uses `beui-tree` components and control flow, independently of the GUI's paints
and layout. The same component code has two backends: it serializes to an HTML
string anywhere (`render`, `render_fragment`, `Node::to_html`), and on
`target_arch = "wasm32"` it also builds live browser nodes (`beui_html::dom`).

```rust
use beui_html::{elements as html, Text, render, view};

let page = render(|| view! {
    <html::Html>
        <html::Body>
            <html::P class="message"><Text value="Hello" /></html::P>
            <html::Button name="action" value="deny" formnovalidate=true>"Deny"</html::Button>
        </html::Body>
    </html::Html>
});
```

### Styling

Style pages with plain CSS: give elements ordinary `class` attributes and put
the rules in one `<Stylesheet css={...} />` (a `<style>` element), usually in
the head; use a `style` attribute for a one-off rule. `class` and `style` are
props like any other attribute, so signals of them update reactively.

```rust
use beui_html::{elements as html, Stylesheet, view};

const CSS: &str = "
body { font-family: system-ui, sans-serif; }
.continue { padding: .5rem 1rem; border-radius: .375rem; }
";

view! {
    <html::Html>
        <html::Head><Stylesheet css={CSS.to_string()} /></html::Head>
        <html::Body>
            <html::Button class="continue">"Continue"</html::Button>
        </html::Body>
    </html::Html>
}
```

A component prop can take a view directly: `prop=<Component />`, or a run of
children with `prop=<>...</>`. This works on any component prop.
Fragments `<>...</>` also work anywhere children fit, including at the root
of `view!`; they group children without a backend node. Empty fragments
`<></>` build empty children. Expressions, reusable components and control
flow work inside fragments.

The compiler grammar lives in `beui-macros/src/lib.rs`; the source formatter has
its own grammar in `fix-rust-source/src/views.rs`, using lossless source tokens.
Changes to view syntax must cover both grammars and their tests.

`render` builds in a fresh reactive scope, flushes bindings, writes the doctype
and HTML, then disposes the scope. For a retained tree, build inside a scope you
keep and call `Node::to_html` after updates. String children are escaped static
text; `Text` binds reactive text. `Show`, `ShowKeepAlive`, `ForEach` and the other
`beui-tree` control flow components work with HTML children.

Build HTML through `view!` and components; do not call builders directly.
Import `beui_html::elements as html` for ordinary HTML elements. Their declarations
in `beui-html/src/elements.rs` generate normal components with shared global
props and element-specific attributes. Extend those declarations for missing
standard attributes. Rust keywords use raw identifiers (`r#type`, `r#for`).
Boolean props control attribute presence; numeric attributes take numbers.
Other attributes take strings. Omitted attributes are absent, and signals of
`Option<T>` remove or restore attributes reactively. Static props need no effects.
Void elements have no children prop and reject children at compile time.

Use `@data={("test-id", value)}` and `@aria={("expanded", "false")}` for
namespaced attributes; ARIA boolean values are strings. `Element tag="custom-tag"`,
`@attr={("custom-name", value)}` and `@flag={("custom-flag", enabled)}` are
escape hatches for custom markup. All attribute names are checked and values escaped.

Multi-child `view!` produces `Children<Node>` without a wrapper. Components can
return that type, and iterators of nodes or child runs collect into it. Both HTML
renderers accept a node or a child run; `render_fragment` omits the doctype.
Low-level node construction is private to the backend. Use
`<Stylesheet css={...} />` and `<Script kind="module" source={...} />` for CSS
and JavaScript/import-map source. These preserve source text and reject a closing
tag for their container. CSS can live in separate files read with `include_str!`.

`view!` and `#[component]` locate their support crate automatically, including
renamed dependencies, through `proc-macro-crate`. A caller can depend on `beui`,
`beui-view`, `beui-html`, or `beui-tree`; no macro path setting is needed.

### Live DOM and hydration

Both backends build the same retained tree of `Node`s, and it stays the source
of truth. `dom::mount(container, || view! {...})` builds browser nodes for it;
`dom::hydrate(container, || view! {...})` claims the nodes a server rendered
from the same view instead. Both take a `web_sys::Node` (an element, or the
`Document` for a page `render` wrote), own all of its children, flush the build
before touching the page, and return a `dom::Mounted` holding the reactive
scope: keep it for as long as the page stays live.

After that, changes patch nodes in place and nothing is rebuilt: `Text` sets its
text node, attribute bindings set or remove the attribute (and the `value`,
`checked` or `selected` property of form controls), and a control-flow fill
moves, inserts or removes nodes only between its two anchors, so rows a
`ForEach` keeps keep their DOM nodes.

Every run of children that control flow opens in an element or at the root
(`Show`, `ForEach`, `Dynamic`, `Keyed`, `Loading` and the rest) sits between
`<!--[-->` and `<!--]-->`, in both backends; nested control flow shares its
outer run's pair, and static children have none. Elements whose content the
parser keeps as text (`script`, `style`, `textarea`, `title`, `noscript`) get
no markers, and hydration rebuilds their content. Hydration walks the tree and
the page in serialization order, claiming elements by tag, text by content
(splitting text nodes the parser merged, creating empty ones) and markers. It
trusts the server's attributes. A mismatch warns on the console in debug builds,
builds that node fresh and removes the server nodes left unclaimed in that
element or run; it never panics.

Elements take event props `on_click`, `on_input`, `on_change`, `on_submit`,
`on_keydown`, `on_keyup`, `on_focus`, `on_blur`, `on_pointerdown`,
`on_pointermove` and `on_pointerup`, and `@on={("name", handler)}` listens to any
other event. Handlers are `Callback<beui_html::Event>`; write a closure in
braces with its argument typed, `on_click={move |_: html::Event| ...}`. `Event`
has `key`, `target_value`, `target_checked`, `prevent_default`,
`stop_propagation` and, on wasm, `raw`, so a handler compiles for both
backends. The string backend ignores listeners. The live backend flushes after
each handler; for signals set anywhere else, the host installs a flush with
`reactive::wake_with`.

`web-sys` and `dom` exist only for `target_arch = "wasm32"`, so native builds
never compile them. beui's HTML core builds its elements with beui-html
([Cores](#cores)); beui-html's own BUCK has `:web`, the library built for the
web app's wasi (`wasi_app_library` in `buck/wasm/defs.bzl`), which puts the live
backend under `//:check`, and crates/beui-shared-tests' `checks()` exercise
mounting, hydration and listeners in Chromium (guides/testing_a_gui.md, 7).

The web entry points are generated by `local-tools web-page` through its
`:block-index` and `:demo-index` Buck targets. Their scripts, import maps and
styles live under `crates/local-tools/src/web_page/`.
