# Beui

Beui is BE3's user interface toolkit, a SolidJS-like framework: a `Document`
owns a persistent tree of nodes, `view!` builds that tree once, and reactive
signals update the nodes in place afterwards. There is no virtual DOM, no
diffing and no per-frame rebuild. Build the view once, keep the `Document`, and
call `Document::show` with it every frame to lay out, dispatch input, expose
accessibility and paint. A component body runs **once**; the closures and
effects it leaves behind are what run again. Everything else in this guide
follows from that.

The quickest introduction is the component catalog in `crates/beui-demo`: a
dock whose Components pane opens a page for each group of styled components,
for the unstyled components painted by hand, and for the base nodes. Inside
be-host it is the block type beui demo (editor
`crates/be-app-example-beui-gallery`, which stores nothing). A function marked
`#[sample]` (from beui-macros, above its `#[component]`) also gets a
`Name::SOURCE` holding its text exactly as written, which the catalog shows
beside each sample.

The [reactive guide](reactive.md) is the reference for signals, attribute
syntax, children and render props, controlled state, keyed lists, scopes and
context; this guide is about using beui itself.

## The rules that matter most

### Every function that builds part of a view is a `#[component]`

A function that returns a `NodeId`, or a value built around one such as a
`CanvasItem`, is a `#[component]` named in `CamelCase`. The attribute makes it
usable as a tag, gives it `@test_id`, `@node_ref` and `@sizing`, makes
`ComponentRef::fill`, `component_accessibility` and `component_placed`
available inside it (they panic anywhere else), and gives it a scope of its
own, so the contexts it provides reach only what it builds. be-tool-fix-rust-source
(run by `./scripts/verify`) rejects any function containing `view!` without
the attribute, since it would build into its caller's scope; test code,
framework trait entrypoints, methods taking `self` (a component cannot, so
`#[component]` rejects a receiver), and views inside document/HTML rendering calls or
factories storing deferred render callbacks are exempt. Helpers that return
child runs rather than a single node are components too.

A node belongs to the reactive scope it was created in, and disposing that
scope removes the nodes it still owns along with its effects
(guides/reactive.md, "Effects, entries, and ownership"). A subtree that must
outlive the code that placed it, such as one shown through a `Portal`, is built
in a scope that lives as long as it should.

The child types of the node parents (`ListChild`, `GridCell`, `Layer`,
`CanvasItem`, `TextItem`) are all `Placed<P>` for the parent's node type `P`: a
node plus the `P::Data` the parent keeps beside it (a `Sizing`, a span, or
nothing), so a child built for one parent does not fit another. A new parent
implements `ParentKind` for its node type, and `BareChild` when a bare `NodeId`
should go in it unchanged.

**Value components** return a value rather than a node, such as
`unstyled::MenuItem` (a menu row: label, icon, detail, a `checked` state
where `Some` makes a check box item, or a radio item with `radio`, an
`action`'s own tick by default; disabled and danger flags, a separator above
it, `on_click`, and its own submenu or page items; `MenuNodes` ticks a
submenu row with no glyph of its own while a radio item inside it is chosen) and `unstyled::ChoiceOption` (an option of a tab bar,
listbox, radio group or select). They still get a scope that lives as long as
the scope that built them, so data items can have their own context, memos and
cleanups and be written as tags whose fields follow signals. With no node,
`ComponentRef::fill`, `component_accessibility` and `component_placed` panic in
them, and `@test_id` and `@node_ref` do not compile (they need an output
implementing `BuildsNode`). Declare the type with `value_child_type!`, which
makes it a `ChildValue` and says how a run of it is kept, so `show` and
`for_each` can build one; a generic type names its parameters first:
`value_child_type!(<K> DockNode<K>)`.

Functions that build no part of a view (deriving a colour from theme tokens and
interaction state, mapping a value to a label) are plain functions, like
`box_fill` in `beui-components-styled`'s `checkbox.rs`.

### A component usually ends with one `view!` and nothing after it

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

Signals, memos, callbacks and handle destructuring go above it. `view!` builds
nodes the moment it runs, so a subtree built into a local and used
conditionally is in the document whether or not it ends up in the tree, and a
subtree built in one place and parented elsewhere obscures which scope owns it.
The shape is not enforced; break it where another is clearer. The base layer is
the usual exception: `List` calls `create_list` and binds setters with effects.

Render-slot closures write markup directly:
`<ForEach keys={keys}>|key: Key| <Row key /></ForEach>`; the formatter removes a
`{ view! { ... } }` wrapper that has no setup statements.

Control flow belongs in the view, not in Rust around it:

- `Show` builds its children each time its condition becomes true and disposes
  of them when it turns false.
- `ShowKeepAlive` builds them once; while hidden it takes them out of the tree,
  keeps their identities and pauses their effects (a tab panel whose scroll and
  state survive switching away). Shown again, it runs once, with current
  values, each effect whose sources changed meanwhile, even if a memo came back
  to its old value, so a controlled `focused` that went false and true takes the
  focus back.
- `Dynamic` rebuilds a subtree when a value changes shape; `Keyed` only when its
  key changes.
- `ForEach` keeps a keyed child per item.

`Show` and `ShowKeepAlive` accept sibling children and components returning
`Children<_>` without a wrapper. A component that is genuinely two trees is two
components with a `Dynamic` or `Show` choosing between them.

None of these builds a node. Each keeps a run of children in a slot of its
parent, which lays them out with its own direction, spacing and alignment while
the children around keep their places. So a `ForEach` has no spacing,
`@sizing` goes on its rows, `@test_id` and `@node_ref` on one do not compile,
and one in a single-child slot like `Frame`'s does not compile until a `List`
(or `Scroll`, or `Canvas`) goes around it. A component whose whole view is a
`Show` returns `-> DynamicSegment<ListChild>` (or whatever child its parent
takes).

### Call components from `view!`, never their builders

`#[component] fn Button` also generates a hidden `__Button_Builder` and a
`Button()` constructor; they are what `view!` writes into, not an API. Write
`<Button label="Save" variant=ButtonVariant::Primary on_click={save} />`, not
`Button().label("Save").variant(...).build()`. Only the macro reports a missing
or doubled prop at the tag, routes `@test_id`, `@node_ref` and `@sizing`,
enforces child arity and keeps render props unbuilt until called, and the
`view!` formatter cannot see a builder chain. To pass a component around, hand
over a `Render`/`RenderFn` closure that writes a `view!`.

### Components in traits and impls

A trait or impl block marked `#[component]` turns its methods also marked
`#[component]` into components, used as tags through the type: `<A::View editor />`
for `A: BeuiRoot`, `<Shelf::Row />` for a method of `impl Shelf`. Without the
mark on the block, a method's `#[component]` fails to compile and
be-tool-fix-rust-source names the block.

```rust
#[component]
pub trait BeuiRoot: 'static {
    #[component]
    fn View(editor: Editor) -> Root;
    #[component]
    fn Preview(editor: Editor) -> NodeId { view! { <Frame /> } }
}

#[component]
impl BeuiRoot for CounterRoot {
    #[component]
    fn View(editor: Editor) -> Root { view! { ... } }
}
```

The trait owns the props: `#[prop(...)]` goes on its declaration, and an impl
writes the same signature without them. Each method becomes a hidden `__View`
holding the body and a provided `View()` constructor returning a builder
generated next to the trait (or the inherent impl). A body runs as a component
named after the type that wrote it (`CounterRoot::View`, or `BeuiRoot::Preview`
for a default), so the inspector tells implementations apart. A component
method takes no `self`.

### Use fine-grained reactivity

Bind a signal or memo to a prop instead of reading it while building or
rebuilding a subtree. A `Prop<T>` takes a `T`, `Signal<T>`, `ReadSignal<T>` or
`Memo<T>`; the reactive forms install an effect that writes just that property.
A prop holds a signal or memo as it is and boxes only a closure
(`Prop::dynamic`), so pass a memo rather than `move || memo.get()`.

A `Prop<T>` is `Copy` when `T` is. A non-`Copy` prop such as `Prop<String>` is
cloned into each closure that keeps it. A tag's children are built in a `move`
closure, so a child `{value}` moves `value`; inside a render closure that runs
more than once (`Show`, `ForEach`), clone it in a block head and work out
anything a later prop needs (a `format!`) above the `view!`:

```rust
<Show condition>|| { let value = value.clone(); view! { <Text>{value}</Text> } }</Show>
```

Wrapping a prop in `create_memo` just to read it twice adds a graph node, and a
`T` that is not `PartialEq` cannot go in a memo at all.

A callback prop (`Callback<V, R>`, `ClickCallback`) takes a closure or an
existing callback, so a wrapper passes its own straight on. A `Callback<T>` also
takes a `Signal<T>`, which it sets: `on_hover_change={hovered}`. State a control
both shows and changes (`checked`, `value`, `open`, `selected`) is one
`Binding<T>` prop: pass a signal (`<Slider value={volume} />`), omit it for the
control to own, or pass `Binding::new(read, write)` (guides/reactive.md,
"Bindings").

**Lists.** Rebuilding every row on each change throws away nodes, scopes,
measured text and any focus or caret, and costs the whole list for a one-item
edit. `ForEach` keyed by item identity reconciles in place: rows that stayed
keep their nodes, and only bindings reading what changed run.

```rust
<List spacing=8.0>
    <ForEach keys={items.keys()}>|id: Uuid| {
        let item = items.get(&id);
        view! { <ItemRow item /> }
    }</ForEach>
</List>
```

The parent decides a run's child kind: `ListChild`s (which can carry `@sizing`)
in a list, plain nodes in a `Scroll`, `CanvasItem`s in a `Canvas`; a row builder
returning the wrong kind is a compile error. Key by identity, not content: a
key containing the row's text changes with the text and rebuilds the row.
`KeyedStore` supplies one signal per item plus the order signal, and
`reconcile` writes only the items that differ.

Elsewhere:

- `create_memo` makes a property wake only when the derived value changes.
- `#[derive(Store)]` splits a struct so writing one field does not wake readers
  of the others; `use_theme()` returns one, so a component binds
  `theme.accent.clone()`, not the whole theme.
- `create_selector` answers "am I the selected row?", so moving a selection
  wakes two rows, not all.
- `Keyed` beats `Dynamic` when only part of a value decides the shape.
- `VirtualList` suits a collection where building every row is the cost.

## Crates

`beui` is the facade every program and editor depends on. It re-exports the
family under the paths this guide uses (`beui::reactive`, `beui::styled`,
`beui::unstyled`, `beui::dock`, ...), so code outside beui names only `beui`
(and `beui_host` for the engine). Plain values from `beui-types` sit at its
root (`beui::Sides`, `beui::Align`), not under `beui::reactive`. What reaches
the native document directly (`Document`, `with_document`,
`try_with_document`, `enter`, `with_reactive_scope`) is in `beui::native`;
prefer a `beui::reactive` API (`TreeNode`, `set_focus`, `use_preferences`, ...)
where one exists.

`beui` is authoring only and has no features: no renderer, fonts, inspector,
drive or platform, so editing those rebuilds no editor library
([decision](../decisions/beui_facade_excludes_the_engine.md)). `beui-host` is
beui plus that engine, for whatever runs documents (programs, demos, test
harnesses, the app runtime). `beui_host::native` is `beui::native` plus `build`
(a document with the inspector and on-screen keyboard installed), `context()`
and the fonts, `testing`, `drive`, `Headless`, `Renderer`, `GpuSetup`,
`OpenDevice`, and the runners `run`, `run_with` and `run_web`. Its features pick
the runner: `window` (default: winit, Android, space, headsets) or `web`. A
host still depends on beui, since `view!` names it. An editor's tests reach the
engine through be-sdk-test or a beui-host dev-dependency; an app's document gets
the inspector and keyboard from `be_sdk_beui_platform_api::runtime().build`.

The rest depend on each other in one direction:

| Crate | Owns |
| --- | --- |
| `be-lib-ui-keys` | `Key` (drive names `from_name`/`name`, `evdev`, and the platform mappings `from_typed`, the one rule for a character key, `from_value` for W3C key values, `evdev_from_code`/`from_evdev`), `Modifiers`, `KeyPress`, `KeyChord`; generated from WIT `be3:keys`, depends only on serde so the terminal's types share it |
| `be-lib-ui-preferences` | `ThemeKind`, `Motion`, `CanvasTouch`, `Preferences`, `PreferenceLayer`, and the display and pointer settings `beui-platform-drm` applies and block settings store (`DisplayMode`, `Rotation`, `InputDevice`, `PointerSettings`, `PointerSpeed`); generated from WIT `be3:preferences`, depends only on serde so be-block and the data stack below beui can use it alone |
| `be-lib-ui-types` | the vocabulary beui shares with the app protocol (depends only on serde, `be-lib-geometry` and the two crates above, which it re-exports): pointer buttons, touch, pen and back phases, `ImeEvent`, `CursorIcon`, `Pasted`, `PickedFile`; `Input`, the raw input every source produces; `WindowRequests` and `ImeArea`; the window vocabulary of window-hosting layers (`WindowId`, `WindowPlacement`, `HostPlacement`, `WindowEdge`, `WindowRole`, `WindowRequest`, `WindowReport`, re-exported by `beui_core::window`) |
| `beui-types` | plain values with no nodes in them, shared by both cores: colours (`Color32`, `Hsva`, `Oklch`, `Color`, `ColorVar`, `color_var!`), `Image`, `Thumbhash`, and layout, text, layer, raw pointer and wheel input, hit area and gesture values (`Sizing`, `Sides`, `Measured`, `SpanStyle`, `Placement`, `PointerInput`, `HitRegion`, `PointerPress`, `ScrollGesture`, ...), `KeyPhase`, `SystemBars`, and the `drive` protocol both cores answer (`drive::describe`, for which each core fills a `Description`, `render`, `ids`, `waited`, `parse`; `target`); re-exports `be-lib-geometry`, `be-lib-ui-types`' input values and lucide's `icons`. Each value has one path: below the facade as `beui_types::...` by the name `beui` gives it, in unstyled and styled as `beui_view::types`, in programs as `beui::`. Values holding a node (`NodeId`, `OverlayAnchor`, `KeyAt`, `KeyEvent`, `ImeCursor`) go through `beui::reactive` |
| `beui-tree` | what makes a tree of components, whatever it is made of: `#[component]` and `view!`'s runtime, `Prop`, child slots (`Children`, `Run`, `SlotChild`, `SlotHost`), `Show`, `ShowKeepAlive`, `Dynamic`, `Keyed`, `ForEach` |
| `beui-core` | `Document` and `Context`, retained nodes and layout, input dispatch, accessibility, paint output and damage, the font and image interfaces, icon codepoints, the `Program` contract |
| `beui-runner` | the `Runner`, the `Platform`, `PlatformServices` and `Layer` contracts, `RunOptions`, `Incoming`, the `BEUI_*` environment (`environment`), the `Renderer` trait (`beui_runner::renderer`); only the engine and hosts depend on it, so editing it rebuilds no editor |
| `beui-font-freetype` | `FreetypeFonts`: FreeType and HarfBuzz shaping and rasterizing, and the fonts beui compiles in |
| `beui-font-browser` | `BrowserFonts`: text measured with the browser's fonts through a canvas; unused so far |
| `beui-view` | the API both cores share: `beui::reactive` (beui-tree plus actions, timers, `NodeRef`, `measure`, the node functions and `Inspect`), `beui::primitives` (`Frame`, `List`, `Text`, ..., declared in `src/components/` with a body per core in `src/native/` and `src/html/`), `CORE`, and under HTML `running` (`beui::html`) |
| `beui-core-html` | the HTML core ([Cores](#cores)), used by beui-view under `cfg(beui_html)`: an arena from `NodeId` to a `beui-html` element, `tree`, inline CSS (`css`), the frame loop, clock and timers (`frame`), measurement, containers (`scroll`, `rows`, `canvas`, `overlay`, `portal`), `text`, browser focus and keys, the hidden IME text agent, page preferences, `run`, `hydrate`, `render_to_string`, and `automate`, which answers `drive` in a browser |
| `beui-web-input` | what both web cores read from the browser: keys (`key`, `keystroke`, `prevents_default`), wheel deltas and pinches, `ImeMirror`, the hidden text agent |
| `beui-components-unstyled` | `beui::unstyled` and `beui::datetime` |
| `beui-components-styled` | `beui::styled`: themes, styled controls, the styled `Frame`, `Text`, `Span` and `Icon`, and every primitive with no styled version re-exported unchanged |
| `beui-inspector` | the inspector, the simulated screen reader, the simulated mouse and keyboard |
| `beui-dock` | `beui::dock`: what an editor writes a dock with: `Docking` and the components that declare its panes, tabs, windows, dialogs and bars, the `DockingLayout` handle, the link a dock declares itself through, and what tab and bar bodies use (`use_dock_tab`, `use_dock_bar`, `dock_menu`, `styled::{DockBarGrip, DockBarSurface}`); it re-exports `beui-dock-state` |
| `beui-dock-host` | `beui::dock_host`: `DockHost`, what lays a declared dock out and draws it (dragging, windows, bands, the stacked and compact modes, the switcher, the keyboard, the window manager), as `dock_host::unstyled` (painting nothing) and `dock_host::styled` (painted with theme tokens), and the `DockHostLayout` handle |
| `beui-dock-state` | the dock's layout model: `DockState`, `DockSpec` and their operations, with no views |
| `beui-keyboard` | `beui::keyboard`, the on-screen keyboard: `keyboard::unstyled` (layouts, tables, gesture engine) and `keyboard::styled`; beui-host's `build` installs it on every document |
| `beui-draw` | `beui::native::{Draw, DrawAt, drawing, painted_drawing, texture_quad}`: the interface a gpu drawing implements and the one pipeline that draws a texture into a rect ([Draw with the gpu](#draw-with-the-gpu)) |
| `beui-renderer-wgpu` | the wgpu renderer, its shaders and filters, presenting to a surface |
| `beui-platform-winit`, `beui-platform-android`, `beui-platform-web` | each platform's `Platform` and `PlatformServices`: event loop, window or view, input (the web's through `beui-web-input`), IME, clipboard, file picker, accessibility adapter (winit's is its own `accessibility.rs` over `accesskit_unix`/`_windows`/`_macos`, since `accesskit_winit` has no release for winit 0.31) |
| `beui-platform-drm` | Linux displays and input through DRM/KMS and libinput: a `Platform`, `PlatformServices` and `Renderer` with one window per display (Displays under beui-platform-drm below). Each display keeps a retained frame of its window, drawn on flip; `pacing.rs` paces the program to at most one frame per flip and none while nothing changes; the flip gets a fence (or the event loop waits, without atomic fences) instead of blocking on the GPU. Offered in `RunOptions::platforms`, picked by `BEUI_PLATFORM=drm` |
| `beui-layer-space` | the VR simulator `Layer` (`BEUI_LAYERS=space`) over a native or headless platform: windows as panels in a 3D scene ("Space: the VR simulator layer" below) |
| `beui-layer-xr` | space on an OpenXR headset (`BEUI_LAYERS=xr`): runtime, Vulkan device, session, swapchains, frame pacing, and `XrSpace`, the `Layer` that drives `Space` ("Headsets" below) |
| `beui-layer-drive` | `beui_host::native::drive`: `Drive`, a `Layer` (or a tap under each of several layers sharing a session), its `Inbox` and `Reply`, socket protocol, and the gestures it shares with editor tests (guides/running/drive.md) |
| `beui-platform-headless` | `Headless` (draws offscreen, no window or display server), `Simulation` (the headless window and stand-in services) and `testing` (steps a program frame by frame under a drive layer), all under `beui_host::native` |
| `beui-host` | the engine items of `beui_host::native`: the inspector and keyboard `build` installs, font and context makers, and the runners (`runner.rs` picks platform and layers from `BEUI_*`; `web.rs`; saved preferences of `RunOptions::saving_preferences`); beui's document tests live here |
| `be-sdk-beui-platform` | the `Platform`, `PlatformServices` and `Renderer` of an app module. Its `AppRuntime` (crates/be-sdk-beui-platform-api, through which be-sdk-beui reaches it without depending on it) launches the module's one program and registers its runner as the module's `ViewHandler`, whose batched calls drive its frames. Each view the host shows is a window of that program, opened from the `Views` handle provided through `Setup` (guides/adding_an_app.md). It depends on the app framework, so beui-host does not offer it; it reads no `BEUI_*` variable |

Inside the family, crates name each other directly
(`beui_core::document::Document`), and the component crates declare
`extern crate beui_view as beui;` so the `::beui::reactive` paths the macros
expand to resolve.

### Hooks core leaves to the crates above it

- The inspector is a `Tools` object on the document, installed by
  `beui_host::native::build` (`install_inspector`) and handed the frame by
  `Document::show`; `beui_view::reactive::build` installs none. The simulated
  mouse is an `InputSimulation` the inspector installs on the `Context`.
- Text is shaped by the `Context`'s `FontBackend`, one line at a time
  (`FontBackend::shape`, no `\n`); core breaks and aligns lines once, in
  `RichLayout`, which `Fonts::layout` and `Painter::layout` use.
  `beui_host::native::context()` has only the bundled fonts (what tests use);
  `system_context()`, which the runners use, adds system fonts for what those
  do not cover. A `FontLibrary` can be shared between contexts and given more
  fonts later, which lays their text out again.
- The theme store lives in a typed slot on the document (`Document::extension`,
  reached by components as `tree_local`), created in the document's own scope so
  it outlives the component that first asked for it.
- A gpu `Drawing` (native only) holds whatever its renderer draws;
  `beui::native::drawing` makes one for wgpu.

### Platforms and the runner

A platform is a `beui_runner::Platform` trait object beui-host picks
(`native_platform`, `web_platform`) and runs with a `Launch` (`RunOptions`,
`Context`, `Program`). It owns the event loop and only translates native input
into `Incoming` values. Everything a frame does is `Runner::frame`, shared by
every platform: it runs the program, publishes accessibility, and tells the
platform's `PlatformServices` what the program asked of the window.
`PlatformServices`' methods all have defaults, so a platform implements only
what it has. A new platform implements both traits and hands its window or
element to the renderer loader beui-host gives it.

- **Window state.** What the window shows for the program is one
  `beui_core::window::WindowState`, filled by the `Context` during a frame and
  carried in the main window's `WindowOutput::window`: `requests`
  (`WindowRequests`, the same type an app sends its host: cursor, pointer lock,
  back handling, keyboard wanted, intercepted keys, paste kinds, IME area,
  keyboard toggles), `cursor_image`, `typed` and `appearance`. The runner calls
  `Platform::show_window` with the new and previous state only when they differ,
  so a platform compares only the fields it uses, and a new field means changes
  there and in the platforms that act on it. `Runner::window` is the state last
  shown. The IME area is in the window's own units (logical pixels, CSS pixels,
  an app region's points). One-off actions (copy, paste, file pick,
  `set_fullscreen`, close) have calls or fields of their own.
- **Typed.** Typing moves the platform's own copy of a text field (the web's
  hidden textarea, Android's editable) even when the program rejects the text,
  so `typed` counts frames that delivered IME or text input, and the web and
  Android platforms resync their copy to the program's text when it changes.
- **Cursors.** While `Context::touch_emulation` is on, the cursor is
  `CursorIcon::Touch`, a finger-sized circle defined once in
  `beui_core::touch_cursor` (as an image, an SVG, and a painter call). Hosts
  that draw their own cursor (mouse simulation, space) draw the others with
  `beui_core::drawn_cursor::paint`. `cursor_image` (`beui_types::CursorImage`,
  straight-alpha RGBA with size and hotspot in points) comes from the hovered
  `Interactive`'s `cursor_image`, which wins over its `cursor`. winit shows it as
  a custom cursor (`drive state` prints its size and hotspot); the web,
  Android, space and touch emulation show the icon. It is not in the app
  protocol. An app
  region passes its icon to its host unchanged; a platform with no custom
  cursors shows `CursorIcon::Touch.name()`, a crosshair.
- **Reports.** The window reports back through `Runner::reports()`, a
  `WindowReports` handle a platform may clone and keep: `resized` (physical
  pixels), `scaled` (pixels per point) and `fullscreen`. A window drawn at
  another raster (a DRM display's, an app region's) gets
  `Incoming::Window(WindowReport::Rastered)` instead. The runner applies them at
  the start of the next frame, resizing renderers and calling
  `Context::set_pixels_per_point` before layout, so platforms never resize a
  renderer or pass a scale themselves. A report made during a frame asks for
  another frame.
- **Fullscreen** goes both ways: `Context::set_fullscreen` asks (the runner
  passes on requests that differ from the last report), and the window reports
  what it did, read as `Context::fullscreen` or `use_fullscreen()`. DRM is
  always fullscreen; Android and apps report nothing.
- **Fitting a window to its content.** `<Window fit={Some(max)}>`
  (`WindowSpec::fit`) ignores its given size and takes min(content, max). Before
  `layout::layout`, `Document::refit_windows` measures the content at an
  unbounded width and again at the widest allowed, then lays the window out once
  at that size, so rows that fill the window fill the width it took. The content
  is the window frame's child or the node `Document::fit_window_to` names (an
  overlay's content for an overlay, such as an app's menu);
  `Document::window_size` and `fitted_window_size` give the size taken. An app
  view placed sized to its content is such a window
  (crates/be-sdk-api/PROTOCOL.md, `child-placement.fit`), marked by be-sdk-beui
  from `View::fit` and fitted to the node `Editor::fit_to` names.
- **File picking.** `Platform::pick_file` gets a `FilePickAnswer`, sendable from
  any thread; it wakes the runner and the pick arrives at the next frame.
- **Accessibility** trees are published only between
  `Runner::accessibility_requested` and `accessibility_deactivated`
  (`Context::accessibility_requested`), so a host asks what it embeds to
  describe itself only while something listens. A document redescribes only the
  nodes a change or move reached (`FrameWork::walked_nodes` counts visits). Each
  node is placed relative to its accessible parent in its layout space, and each
  space (`Document::enter_space`, a scroll's content, a canvas's spaces) is a
  `GenericContainer` whose transform is the space's, so a scroll or pan changes
  one node.
- **Sleeping.** `Frame::wait` is how long the platform may sleep (`None` until
  woken), and `Frame::next` the same once it knows whether the presented frame
  asks to be presented again.
- **Closing.** After the frame in which the program calls `close_window` (or a
  driver's close is accepted), `frame` returns `None` with `Runner::exited` set
  and the platform stops its loop (Android finishes its activity, the web closes
  the tab where allowed, an app region's `Frame` says `stopped`:
  `Stopped::Closed` or `Stopped::Failed`, which the host shows in its place). A
  window closed from outside calls `Runner::os_close`, which asks the program's
  `close_requested`; the web asks on `beforeunload`, where a refusal is the
  browser's leave-the-page prompt.
- **Panics.** A panic out of the program's `update` stops the runner rather than
  unwinding through the event loop: the program hears `exiting`, `frame` returns
  `None`, `Runner::failure` says why, and the platform returns it as an error
  (`Runner::finish`), releasing the screen in order. A program that can survive
  a panic in one part catches it there.

### Clipboard and paste

- `Context::copy_text` writes text; `Context::copy_image`
  (`beui_reactive::copy_image`) writes a PNG (winit through arboard, the web as
  a `ClipboardItem`, an app through the shell interface's `copy-image`, the
  headless `Simulation` pastes it back as `Pasted::Image`; Android and DRM drop
  it). `Context::watch_copies` hears every text copy.
- `Context::read_clipboard` reads the clipboard's text for the program itself,
  outside any focus (answered at the start of a later frame, `None` without
  text). be-host-wayland uses this and `watch_copies` to bridge the clipboard
  with Wayland programs.
- The focus reads the clipboard by pasting. An `Interactive`'s `pastes` lists
  the `PasteKind`s (text, image) it takes, preferred first; one with only
  `on_text` takes text. `request_paste` (a Paste menu) and Ctrl+V (when nothing
  on the key's way took it, or the focus forwards it) ask `Platform::paste` once
  a frame for the first kind the clipboard holds. The answer enters the intake
  as `Input::Paste` after earlier input and reaches the focus as `Event::Paste`
  (`Pasted::Text`, or `Pasted::Image` as a PNG `PickedFile`) via `on_paste`, or
  `on_text` for text without one. Platforms that deliver Ctrl+V themselves
  (`Platform::delivers_paste_shortcut`: the browser, from its paste event and
  `paste_kinds`, and an app's host) are not asked again. winit reads images;
  Android and DRM text only.
- An `Interactive` with `clipboard_keys` (a terminal) takes Ctrl+C, Ctrl+V,
  Ctrl+X and Ctrl+A as plain keys while focus is in it
  (`Document::clipboard_keys`, sent as `WindowRequests::clipboard_keys`, which
  an app's region passes to its own catcher). This matters only on the web,
  which decides in `keydown` before the program sees the key: it stops the
  browser's copy, paste and cut (and its inspector's Ctrl+Shift+C) but lets
  Ctrl+Shift+V make a `paste` event. A `copy_text` made on such a key still
  reaches the web clipboard (it is inside the key's user gesture), but a node
  that takes these keys and leaves Ctrl+V unanswered gets no paste on the web.

### Input

A platform hands `Runner::push` an `Incoming`: an `Input` in physical pixels
(`Incoming::from`), logical pixels (`Incoming::logical`, converted with the
scale the frame lays out at, so no platform reads its scale at event time) or
points (`Incoming::points`); an `Incoming::FileDrag` (files hovering, the hover
cancelled, or a `DroppedFile` with its name and host path or bytes; these live
in beui-core because they never cross the app protocol, whose host turns them
into `drag-over` messages); or `Incoming::Time`.

`Input` covers pointer moves and buttons, a `Keystroke` (the key a shortcut
matches, its text and evdev code, no modifiers; guides/beui_keyboard.md), a
`ScrollDelta` in pixels, lines (`SCROLL_LINE`) or pages (`SCROLL_PAGE`),
touches, pen samples, text, IME edits, a paste, the modifiers held and window
focus. The runner's intake is the one place it becomes `Event`s; drive's
gestures, the on-screen keyboard and typed pastes push `Input` there too, so
they are normalized like a window's.

At the start of each frame the runner converts input to points and:

- stamps keys, buttons and intercepted keys with the held modifiers, drops a
  held modifier's repeats and a shortcut's text, drops control characters, and
  passes `Input::Modifiers` on only when it changes;
- turns the primary mouse button into a finger under
  `Context::touch_emulation`;
- holds `PointerGone` back while a button is held, sending it after the release
  unless a move inside the window came first;
- releases the modifiers when the keyboard is lost (`Input::Focus(false)` means
  only the keyboard).

It queues input with `be_lib_ui_types::input::enqueue`, the same rules an app's
host merges its batches by: scrolls and zooms fold into the last of a run,
repeated modifier reports drop, and each pointer is heard once per frame at its
latest move, but an event between two moves keeps both, so a click lands where
it was made. Every platform sample is its own event (the web pushes each of
`getCoalescedEvents()`).

- **Every sample.** An `Interactive` with `every_sample` (a `Prop<bool>`, set
  while needed, as the canvas pen does during a stroke) makes the document set
  `WindowRequests::every_sample` (`Context::want_every_sample`); the runner then
  keeps every move for that window and `on_drag` runs per sample. App window
  requests carry it to the host, which keeps every sample in its batches.
- **Timestamps.** A platform pushes `Incoming::Time(Instant)` just after the
  event it stamps; it becomes `PointerEvent::time` and `PointerPress::time` and
  drives touch fling velocity. The web, Android and DRM use the device's
  timestamps; winit stamps on arrival. An event pushed without one (tests,
  drive) happens at the frame clock's `now`, so tests step time by stepping
  frames.
- **Prediction.** After a pressed move the web pushes `Input::Predicted` (the
  last of `getPredictedEvents()`), attached to the preceding move as
  `PointerEvent::predicted` and carried by its drag as
  `PointerPress::predicted`. The next sample replaces it, so a component draws
  it only as a transient tail (as the canvas pen does) and does not store it.
  Other platforms report none; an app's host forwards it.
- **Pinch.** A trackpad pinch is an `Event::Zoom` everywhere: winit's
  `PinchGesture`, and in the browser a Ctrl-wheel (Chrome and Firefox; scaled by
  `PINCH_SPEED` per pixel) or Safari's `gesturechange`. The web ignores gestures
  while fingers are down (those touches pinch already) and prevents their
  default so the page never zooms.
- **Touchpad gestures** also arrive whole as `Input::Touchpad`
  (`TouchpadGesture`: swipe, pinch or hold, with finger count, update, end or
  cancel), from libinput and winit (`touchpad.rs`; winit has no swipes). Only
  the forwarding node under the pointer receives them, for whatever it forwards
  to (a Wayland window in be-host-wayland).
- **Trackpad scrolling.** Fingers down push `Input::ScrollStarted` and lifting
  `Input::ScrollEnded` (forwarded by app hosts). Between them
  `ScrollGesture::held` is set and a `Scroll` follows the fingers like a touch
  drag, stretching past an end; the lift springs back or flings and bands. A
  start alone (resting fingers, macOS's `MayBegin`) goes to the press claimant
  under the pointer, catching a flinging or bouncing `Scroll` or sheet. A wheel
  with no start (a mouse) clamps at the ends. winit maps `MouseWheel` phases
  `Started`/`Ended` and on macOS drops the momentum events after a lift
  (`beui-platform-winit/src/wheel.rs`)
  ([decision](../decisions/beui_winit_drops_macos_scroll_momentum.md)). The
  browser reports neither phase, so web wheel scrolls never stretch or fling.
- **Cancelling.** `Input::PointersCancelled` ends every press, touch and pen
  session without a release: the runner releases buttons and emulated touch,
  and the document cancels touches, ends pen and mouse
  (`Event::PointersCancelled`) and cancels a pointer grab, which stops
  autoscrolling. winit and the web send it when the window loses focus, and
  winit on Wayland also when the pointer leaves with a button held (a compositor
  grab sends `wl_pointer.leave` and no release). Android maps `ACTION_CANCEL`,
  the web `pointercancel`, DRM libinput, and an app's host ends each pointer
  through `Replay`.

### Renderers

Platforms draw through a `beui_runner::renderer::Renderer` trait object and
name no renderer crate: wgpu's `WindowSurface` (native windows) and
`CanvasSurface` (a browser canvas) implement it, and beui-host hands the runner
a loader returning `Loaded` (the renderer, plus its own `FontBackend` if it
needs one, installed by `Runner::start` with `Context::replace_fonts`). The
runner keeps it in a `Rendering` the platform attaches and detaches through
`Runner::rendering`. A renderer is given a raw-window-handle `WindowHandle` with
`attach` or is made with its element, and draws with `prepare` then `present`.

The wgpu renderer splits into `beui_renderer_wgpu::Resources`, shared by every
target on one device and surface format (pipelines, samplers, the 2048x2048 R8
glyph atlas, the RGBA colour glyph atlas made on first use, uploaded images kept
while any renderer drew them last frame), and `beui_renderer_wgpu::Renderer`
(`Renderer::new(&resources)`), one target's buffers, display-list cache, filters
and scratch texture. Give every window or panel on one device a renderer over
the same `Resources`, as be-sdk-beui-platform does per app module,
beui-layer-space for its panels and beui-platform-drm for its displays. A full
atlas is replaced rather than cleared and a renderer draws with what it was
prepared against, so renderers sharing `Resources` may prepare, render and
submit in any order.

### Other trees

`beui-tree` knows nothing of `Document`, layout or painting, so its components
and control flow can build any tree: a DOM, a 3D scene, a chat message with
buttons. The tree's child type implements `ChildValue` (where a component's
scope lives) and `SlotChild` (how a child is kept), and what keeps children
implements `SlotHost` (appending, filling a `Show` or `ForEach` slot) and
`IntoSlotHost`, so a component can `children.mount(...)` into it. The macros
expand to `::beui::reactive`: `beui-tree` names itself `beui` for its own tests,
and a crate built on it does the same or re-exports `beui_tree::reactive` as
the `reactive` module of a crate it calls `beui`. An untyped `Render`/`RenderFn`
child or `Child` prop is `beui::reactive::Child`, a `NodeId`. `beui-tree`'s
tests build a small DOM this way.

Modifiers are ordinary functions: `@name=value` expands to `name(built, value)`
after the component is built, and `@my_module::decorate=value` works too
(qualify one when a local shadows its name). A modifier without `=` uses its
path as its value, like prop punning. Modifiers run in source order, repeats
included, can change the output type, and also follow expression and string
children (`{child} @decorate=value`). Ordinary props are set on the builder
before any modifier runs, wherever they are written. The GUI prelude
(`beui::reactive::*`) supplies `sizing`, `test_id` and `node_ref`; other trees
supply their own.

## Cores

beui has two cores under the same components. The native core, `beui-core`,
lays out, focuses, hit-tests and paints retained nodes itself (with wgpu), on
every native platform and in be-host's web build. The HTML core,
`beui-core-html`, makes each primitive a DOM element and lets the browser lay
out, focus, select and paint. beui-view holds both behind one API, and
unstyled, styled and everything above are the same source on both
([decision](../decisions/beui_has_an_html_core_on_the_web.md)).

`--cfg beui_html` picks the core at compile time; the buck platforms
`wasi_html` (browser) and `linux_x86_64_html` (host, for server rendering and
HTML paintings) set it (guides/buck2.md), and every other platform builds
native. Code switches on the constant `beui::CORE`
(`if CORE == Core::Html`); `#[cfg(beui_html)]` appears only in beui-view, beui
and beui-core-html. Such a switch is a deliberate choice for one component, not
a way around a primitive one core lacks.

The inspector and app hosting are native only (`beui::open_inspector()` returns
whether one opened, never on HTML). The dock and on-screen keyboard run on
both: on HTML a `Window` (so a `DockWindow`) shows as its overlay, `window_of`
is always the main window, and `stacked_at` orders nodes by the open overlays
holding them.

### One public API

Code that type-checks against beui on one core type-checks on the other; only
bodies differ. be-tool-fix-rust-source enforces the layout:

- Every public item of beui-view and beui is declared once in shared code:
  node-free plain values (`NodeId`, `NodeRef`, `KeyAt`, `KeyEvent`,
  `OverlayAnchor`, `ImeCursor`, `Placed` and its aliases, `ForwardedInput`, ...)
  in beui-types, everything else in beui-view's shared modules (`reactive.rs`,
  `components/`, `measure.rs`, `painter.rs`, `drag.rs`, ...).
- `src/native/` and `src/html/` give only `pub(crate)` bodies, called as
  `crate::core::<module>::<name>(..)` from the shared declaration, so each body
  is checked against the one signature.
- A handle whose insides differ by core is one type with a private per-core
  field and methods written once (`TextGeometry`, a snapshot natively and the
  live DOM on HTML; `TextGeometrySignal`, `Painter` and its `Draw`, `Timer`,
  `SelectableState`). A method only one core can give is not on the type.
- One-core items live in the modules listed, with reasons, in
  `ONE_CORE_MODULES` in crates/be-tool-fix-rust-source/src/core_api.rs:
  `beui::native` (programs, platforms, runners, `Document`, renderers, fonts,
  GPU `Drawing`s, `draw_gpu`, and `core_painter`, the `NativePainter` behind a
  `Painter`), `beui::html` (`run`, `hydrate`, `render_to_string`, ...) and the
  beui-view modules they re-export (`native_api`, `running`). The tool rejects a
  `pub` item in `src/native/` or `src/html/`, or under a `cfg(beui_html)` choice
  elsewhere in beui-view or beui. A new one-core item goes in one of those
  modules, or in a new module added to the list with its reason.

### The contract both cores keep

- **Answers may come late.** Whatever layout decides (`measure`,
  `measure_text`, a scroll's position, an overlay's side) is a signal that is
  `None` until the first layout and may arrive a frame after its cause, so
  callers handle the late answer. Native answers in the same frame, which
  virtual scrolling and scroll anchoring rely on to stay frame-perfect.
- **The browser owns focus on HTML.** Focus is real DOM focus (`tabindex`,
  `:focus-visible`); `set_focus`, Tab order and `focused` bindings go through
  it, so focus has moved only once code hears so.
- **Each core does the best its platform allows**: the browser's own
  selection, find in page and scrolling into view on HTML; native's own
  selection and frame-perfect layout.
- **Every feature exists on both cores.** A new prop or hook gets a body in
  both; where HTML cannot do it yet, its body is a visible stand-in (an outlined
  pink box, or an ignored prop) listed in plans/html_core.md.

Measurement is `measure(node) -> ReadSignal<Option<Measured>>` (`rect` in
window space, `size` in the node's own units), `measure_text(node)` for a
`Text`'s `TextGeometry`, and `lay_out_text(text, font, layout)` for text not in
the tree ([Measurement](#measurement)). On HTML `measure(node)` is `None` until
the frame after the node is first measured, so a key handler that measures at
event time fails the first time; anchor an overlay to the node
(`OverlayAnchor::Node`) instead. Colours are `Color`s that may reference
declared variables, so both cores and a server-rendered page agree on them
([Colours](#colours)).

### Primitives on the HTML core

Bodies are in `beui-view/src/html/`, larger parts in `beui-core-html`
([Develop a base component](#develop-a-base-component)):

| Primitive | On HTML |
| --- | --- |
| `Root` | `div.beui-root`, fixed to the viewport and clipping overflow, so only `Scroll`s scroll and the page needs no sizing styles; it holds the colour definitions, and every overlay's element stays inside it to see the same variables |
| `Frame` | a one-cell CSS grid; `translate` and `rotate` a transform, `opacity` CSS opacity, `fade_edges` a mask image |
| `List`, `Spacer` | flexbox and a flex child; `@sizing` becomes flex grow, shrink and basis |
| `Grid`, `Layers` | CSS grid; `Layers` stacks every child in one cell |
| `Text` and its spans | a `div` of `span`s and text nodes with `white-space: pre-wrap`; `marks` are CSS Custom Highlights, `carets` positioned boxes, `glyphs` `::before` content, `Break` an `::after` newline no browser copies; a `TextItem` with `at` an absolutely placed box a layout pass moves onto that index's caret |
| `Offset` | an `overflow: hidden` element moved through `scrollTop`/`scrollLeft`, not a transform |
| `VirtualList` | rows in flow between two spacer elements, measured by `ResizeObserver` |
| `Canvas` | an inner element translated to the view, `--beui-scale` scaling items |
| `Overlay` | a `<dialog>` (`showModal()` if it blocks everything, else a manual popover) placed with CSS anchor positioning, a one-cell grid while open; `window` and `shown_in` are ignored |
| `Portal` | moves the shown subtree's element (`moveBefore` where available) |
| `Interactive` | an element with `tabindex` and listeners; `on_forward` hears the pointer (captured from its down); `pastes`, `key_active`, `claims_touch` and the other forwarding props do nothing yet |
| `Selectable` | the browser's selection; the root is `user-select: none`, so only `Selectable`s and native text fields select |
| `Drawing` | a `div` holding an inline `<svg>` the `Draw` callback paints, sized as natively |

`Drawing` is the escape hatch for what is not an arrangement of nodes, and
`Embed` (native only) for what is not beui's. On HTML a `Drawing` runs its
callback into an SVG `Painter` (`beui-core-html`'s `drawing.rs`) and sets the
result as its element's markup (`beui_html::Node::set_markup`), rebuilding only
that SVG when the callback, its measured size or a resolved colour changes
([decision](../decisions/beui_drawing_is_inline_svg_on_html.md)). The callback gets
the measured size, or its `size` before measurement, so a drawing with neither
(one that fills its parent) draws nothing in a server-rendered page.

`Painter` (beui-view's `painter.rs`) has what both cores can draw: rectangles
(rounded per corner, filled or stroked inside), `line`, `fill_path` and
`stroke_path` (fill rule, joins, caps, miter limit, dashes), `image` (PNG data
URL, cropped, rounded, tinted, pixelated unless `smooth`), `lay_out_text`,
`draw_text`, `transformed`, `rotated` (rotations stack), `with_clip_rect`,
`clip_rect`, `pixel_grid` and `resolve_color`. Its colours are `Color`s, so on
HTML references become `var(--…)` and `color-mix()` and follow the scheme
without a redraw. Native only: a gpu `Drawing` (`beui::native::Drawing`, drawn
by `draw_gpu`) and the rest of the native painter (GPU-prepared paths, rich text
runs, fades), reached with `beui::native::core_painter(painter)`. The colour
wheel paints its faces as a CPU image without the `gpu` feature
(`color_wheel/cpu.rs`).

`TextArea` and `TextInput` are the same unstyled code on both cores, built on
`Text`'s `marks`, `glyphs`, `carets` and `hang` and on `measure_text`, typing
through a hidden textarea on HTML; only a plain single-line `TextInput` is a
real `<input>` on HTML ([Forms and text fields](#forms-and-text-fields)).

A component's accessibility node (`component_accessibility`) becomes ARIA on
its element: role, `aria-label` and states (checked or pressed, selected,
expanded, disabled, range value and bounds, heading level, ...), mapped by the
one table in `beui-core-html`'s `aria.rs`. An element that already is the
matching native element (a plain `TextInput`'s `<input>`, `<button>`, `<a>`, an
overlay's `<dialog>`) keeps its own role. When one element holds several
components' nodes, the outermost describes it, in the DOM and in `automate`'s
tree. Drive lines come from the accessibility nodes, not the ARIA, so they read
the same on both cores; a web drive script checks ARIA with `role`
(guides/testing_a_gui.md).

### Running and server rendering

A program on the HTML core starts with `beui::html::run(element_id, build)`. For
a page that shows before any script runs:

1. Render it on the host with `beui::html::render_to_string(build)` in a program
   built for `linux_x86_64_html`, at build time with the `prerendered_page` rule
   in buck/program/defs.bzl, or per request as be-server's account portal does
   with crates/be-server-portal-ui.
2. Put `beui::html::stylesheet(font_base)` in a `<style id=STYLES_ID>` in the
   head, `font_base` being the URL the bundled fonts' `@font-face` rules use.
3. Call `beui::html::hydrate(element_id, build)` with the same view in the
   browser; it claims the server's elements instead of building new ones
   ([Live DOM and hydration](#live-dom-and-hydration)).

`run` and `hydrate` read the browser's preferences before building, so the first
frame matches the page; `beui::html::choose_theme` overrides the browser's
scheme. crates/beui-html-spike is the worked example: be-host's account picker
(crates/be-host-accounts-ui) prerendered by `:ssr` and hydrated by `:web`
(guides/running/web.md). crates/beui-demo-html runs beui-demo's pages on this
core.

### Testing on both cores

- Behaviour that must match goes in a shared script in crates/beui-tests (a
  screen and a `drive` script), run natively, natively with late measurement,
  and in Chromium (guides/testing_a_gui.md, 7).
- `beui_host::native::testing::delay_measurement(true)`
  (`BEUI_DELAY_MEASUREMENT=1` for a driven program) makes native tests take the
  late path, so a component that relies on synchronous measurement fails
  natively too.
- Browser paintings check what the HTML core draws, rendered from
  `render_to_string` on `linux_x86_64_html`:
  `./scripts/test //crates/beui-view:html-test` for primitives,
  `./scripts/test //crates/beui:html-test` for unstyled and styled components
  (tests in `crates/beui/src/html_paintings/tests/`), and the spike's
  `:html-test`. Build the page's `<style>` from `stylesheet("/")` so its
  `@font-face` rules name the bundled fonts the browser rasterizer serves at the
  root, and text and icons draw as natively. Paintings show layout, colour and
  text, not what needs browser layout (marks, carets).
- After a change to the HTML core or styled, rasterize and compare the spike's
  `crates/beui-html-spike/snapshots/prerendered_accounts_page_dark.browser.paint`
  with be-host-accounts-ui's
  `an_account_in_a_company_says_its_administrators_can_read_it.paint`, the same
  screen on the two cores. They differ only in text rendering (glyph advances,
  hinting, and the browser rounding ascent and descent to the nearest pixel
  where FreeType rounds outward, moving a 20px title and some icons by a pixel).

## Component layers

Beui separates mechanism, behavior and appearance, and each layer depends only on the one below:

| Layer | Location and public path | Responsibility |
| --- | --- | --- |
| Base | declared in `crates/beui-view/src/components`, with a body per core in `src/native/` (over `beui-core`'s nodes in `crates/beui-core/src/base`) and `src/html/` (over `beui-core-html`); `beui::primitives`, which beui's own crates use and programs reach through `beui::styled` | Layout, painting, visibility, focus, pointer input, offset content, and text, implemented by each core (see [Cores](#cores)). |
| Unstyled | `crates/beui-components-unstyled`; `beui::unstyled` | Accessible interaction behavior composed from base components, without theme colors, typography, borders, or spacing. |
| Styled | `crates/beui-components-styled`; `beui::styled` | Application-ready controls that compose an unstyled control and paint its state with base components and `styled::theme` tokens. |

Work at the highest layer that can express what you need:

- **A feature in a program:** compose styled controls with base layout and painting primitives.
- **Existing behavior, different appearance:** use the unstyled component and paint it yourself. Focus, keyboard, pointer, touch and accessibility handling belong to the unstyled component, so a styled one that reimplements them drifts from every other face of the same control.
- **New behavior:** add an unstyled component composed from base components.
- **A new base component:** usually an unstyled one is the right answer instead. A base component is for a primitive the retained tree lacks: a new way to lay out, paint or receive input that no arrangement of existing nodes expresses (`Portal`, which shows a subtree it does not own, is one). A concern that fits `Frame`'s single-child box model extends `Frame` rather than adding a pass-through node. See [../decisions/beui_base_layer_kept_small.md](../decisions/beui_base_layer_kept_small.md).

The base layer is `Frame`, `List`, `Spacer`, `Layers`, `Grid`, `Text`, `Offset`, `VirtualList`, `Canvas`, `Drawing`, `Overlay`, `Interactive`, `Portal` and `Root`. Things that look like primitives but are compositions:

- `unstyled::Picture` is a `Drawing` with a size; `Window` (`beui_view::window`, re-exported as `beui::Window`) is an `Overlay` given a `window` spec.
- Back is a hook, `on_back`, not a node.
- `TextArea` and `TextInput` are unstyled compositions of `Text` (`marks`, `glyphs`, `carets`, `hang`), `Interactive`, `measure_text` and `lay_out_text`, the same on every core.
- `Embed` is in `beui::native`, since only the native program composites.
- `Selectable` is the one component each core builds its own way (the browser's selection on HTML) rather than as a composition (see [Long lists](#long-lists)).

Pure presentation components such as styled text and cards compose base components directly, since they have no behavior to delegate. The re-exports in `unstyled.rs` and `styled.rs` are the authoritative lists of each layer.

### Layout nodes

**`List`** arranges siblings in a line: `direction` (vertical unless given), `align` for the cross axis, `justify` for the main axis (`Start`, `Center`, `End`, `SpaceBetween`, `SpaceAround`, `SpaceEvenly`, so pushing a toolbar's last item to the far end needs no spacer) and `spacing`. There are no `Row`/`Column` aliases: a row is `<List direction=Direction::Horizontal spacing=8.0>`, and one that centres its children adds `align=Align::Center` ([../decisions/beui_list_props_over_row_column_aliases.md](../decisions/beui_list_props_over_row_column_aliases.md)). `Align::Baseline` lines a row's children up on the first baseline of their text, which a node reports through `Element::baseline`; a column treats it as `Start`. `wrap=true` flows children onto more lines: a child too wide for a line by itself is measured against the line's width, so long text wraps inside it, and each line's `Percent` children share that line's leftover.

A child's `@sizing` is an `ItemSize` (`Intrinsic`, `Fixed`, or `Percent`, a weight in the bounded space left over) or a `Sizing` built from one:

- `.min(..)` and `.max(..)` bound its length;
- `.shrink(weight)` lets it give up length down to its `min` when the row overflows (the label beside a fixed-size button);
- neither a shrinking child nor a `Percent` one in a row gets less than its own `Element::min_width` (wrapping text's longest word);
- `.align(..)` overrides the list's `align` for that child, and `.gap(..)` replaces the list's `spacing` before it.

```rust
@sizing={ItemSize::Percent(100.0).max(320.0)}
```

**`Frame`** puts optional sizing, an aspect ratio it centres its box within, padding, fill and outline on one retained node, one prop per concern.

- `width` and `height` take an `Extent`: an exact length (`Extent::px`, or a plain `f32`), a share of bounded offered space (`Extent::fraction`), and `.min(..)`/`.max(..)` bounds. A `max` caps the room the frame takes, not its content, like CSS `max-width` on a block: `Extent::at_most(320.0)` is as wide as offered up to 320, however little its content needs (a dialog, a settings column). A box as wide as its content up to a cap (a menu, a tooltip) is instead a list child sized `@sizing={ItemSize::Intrinsic.max(320.0)}`, in a horizontal `List` if nothing else wraps it; `ItemSize::Intrinsic.min(..)` gives it a floor. A memo of `Option<f32>` hands an axis back to intrinsic measuring when it is `None`.
- `padding` takes `Sides` (`Sides::all`, `symmetric`, `horizontal`, `vertical`, then `.left(..)` and its siblings). A negative side lays the child out that far past the frame.
- `align` takes an `Align2` (a plain `Align` sets both axes) and places a smaller child inside the frame rather than stretching it, which is how a badge sits in a corner.
- `fill` is a `Color32`, `radius` `Corners`, `outline` an `Option<Outline>`.
- Effects that need no node of their own, all leaving the space the frame takes alone: `translate` moves its box and child by a vector (snapped to the pixel grid) and paints nothing while both are off screen (how a sheet slides in); `rotate` paints it turned clockwise by that many radians about the middle of its box, with clips inside growing to the upright bounds of what they clip; `opacity` (1 by default) paints all of it, fill and border included, at that opacity; `fade_edges` fades it toward each edge over the widths it names. Opacity and edges are one mask (`Painter::faded`, `Group::Mask`): the renderer multiplies every shape's coverage by it, so overlapping shapes inside show through one another, and a change damages the frame's rectangle. Pointer input and the accessibility tree follow `translate` but see a rotated frame unturned, so `rotate` suits pictures (a canvas's turned text, a preview of a turned screen), not controls. Rotation never swaps a box's width and height; for a quarter turn, turn a square box, as the display settings' rotation tiles do.
- Frames paint no shadows: a floating surface stands apart by its fill and border.

Nothing has a hidden state while it stays in the tree. What shows only some of the time goes in a `Show`, or a `ShowKeepAlive` to keep its state while gone, so it takes no room, spacing, focus or place in the accessibility tree. In a single-child slot like a `Frame`'s, put the `Show` in a `Layers`, which fills the slot as the child would.

**`Layers`** lays every child over the same box, sized to the largest, later children on top (content over a background, a badge over an icon). A child that should not fill the box goes in an aligning `Frame`. Pointer input reaches the topmost catcher under the pointer and the catchers around it, never one in a layer beneath.

**`Grid`** lines cells up in shared columns, filling row by row: `columns` is a `Track` per column (`Fixed`, `Intrinsic` as wide as its widest cell, or `Fraction`, a weight in what is left), with `column_spacing` and `row_spacing`; each row is as tall as its tallest cell; `<GridCell span=2>` spans columns. It is for forms and property panels whose labels share one width; a list of rows is a `List` or a `VirtualList`.

A table of values to compare is `styled::Table`: column headings along the top (each a `TableColumn`, which can carry a caption), a `TableRow` per row with its heading down the side, and a `TableCell` per column with an optional caption under its value. Rows share the column weights, so cells line up, and it reads as a table to assistive tech through `unstyled::Table`, `TableRow`, `TableColumnHeader`, `TableRowHeader` and `TableCell`, each wrapping whatever its face lays out.

**`Spacer`** (base) is an empty box with no props: given a `Percent` `@sizing` it fills the rest of a row, or it stands in for a missing child. A fixed gap is the styled `Spacer`, whose `width` and `height` are `Space`s. **`Separator`** (styled) runs `Direction::Horizontal` unless given and takes a line's thickness across its `direction` and the list's space along it, so a divider in a row is `<Separator direction=Direction::Vertical />` with no `@sizing`; `length` pins the long axis in a row that centres rather than stretches its children.

**`Offset`** takes exactly one child, lays it out at its full length along a `direction` and shows it from an offset; a run of rows goes in a `List` inside it. It answers no input, so it does not scroll anything by itself (see [Scrolling](#scrolling)). With `fit` it measures as long as its child. Without `fit` it measures nothing along its direction (it fills what it is given), except when measured with no limit along it, as a `Percent` child inside a content-sized box (a fitted sheet) is: then it measures as long as its child. Rows far outside its viewport are culled like any node out of sight.

**`VirtualList`** is an ordinary box that stands for one item per key, each of an estimated `item_size`, and builds only those its slice of the viewport reaches (see [Long lists](#long-lists)).

**`Portal`** takes a `NodeId` and lays out and paints that subtree where the portal stands, leaving it alone when the portal goes away, so a subtree can move between places without being rebuilt. Exactly one portal shows a node: claiming it takes it from the portal that had it. The subtree lives as long as the scope that built it. A node no portal shows keeps the arena parent of the portal that last showed it (`Arena::parent` is the last container that listed it), so code walking up from a node has to check that each step still lists it; for example a `Canvas` lays out the item holding the focus only if it is one of its own items.

**`Embed`** (`beui::native::Embed`) reserves a rectangle for something outside the document, such as an editor the host composites behind the surface. It publishes the rectangle and the clip it was laid out in through the `EmbedSlot` it was given (a `Copy` handle owned by the scope that made it), and cuts that rectangle out of the surface so what is behind shows through; `punch=false` keeps the surface whole for something the host draws over it. The placement carries its window's raster over the document's scale (`EmbedPlacement::raster`, 1 unless a host draws the window finer or coarser), announced again when a `WindowReport::Rastered` changes it, so the host can draw the embed as sharp as the window. What the host draws covers the document, overlays aside. A node that has to stay above embeds painted before it marks itself with `above_embeds(node, Some(AboveEmbeds::Reaches))`, which keeps its rectangle for its own input (a dock splitter's reach over a child editor), or `AboveEmbeds::Paints`, which also has the host draw the document's pixels there over them (the splitter's line and focus ring). The marks join the overlays in `Document::overlay_layers` (by paint order, so only embeds painted earlier are covered), `overlay_rects` and, for `Paints`, `overlay_painted_rects`, which an app reports as occluders, floating and floating painted rectangles. Because the host draws the document over the embed there, a mark over a large empty area hides the embed under it; mark only what paints a few pixels.

### Styled tokens

The primitives take concrete values; the styled components programs use take design tokens (`styled::tokens`):

- `padding`: a `Padding` of `Space` steps (`Padding::all(Space::Md)`, `symmetric`, `horizontal`, `vertical`, then `.left(..)` and its siblings; a plain `Space` pads every side).
- `width`/`height`: a `Length`, an `Extent` for fill, fraction and at-most, or a `Size` for a fixed length.
- `fill`: a `ColorRole`, a theme colour or `ColorRole::Custom` for one from content (a user's tag colour); `ColorRole::custom(..)` wraps a colour or a signal of one.
- `radius`: a `Radius` (`Xs` to `Lg`, `Tab`, `Full`, or `Custom(f32)` for a corner that scales with content, such as a card on a zoomed canvas), with `corners` a `CornerSet` when only some corners round.
- `border`: an `Option<Border>` (`Border::DEFAULT`, `Border::FOCUS`, `Border::new(role)` with `.width(..)` and `.offset(..)` taking a `Stroke`: `Hairline`, `Focus`); `focus_border` shows one only while the focus ring is visible.
- Styled `List` and `Grid` take `spacing`, `column_spacing` and `row_spacing` as `Space`, and styled `Gap` its `width`; the base ones take pixels.

`Space` steps are `None` 0, `Xs2` 2, `Xs` 4, `Sm` 6, `Md` 8, `Lg` 12, `Xl` 16, `Xl2` 20 and `Xl3` 24. `Size` steps are `Track` 6, `Xs` 16, `Sm` 20, `Md` 24, `Lg` 32, `Xl` 40, `Xl2` 48, `Xl3` 64, then the named `Row`, `Toolbar`, `Field`, `TouchRow`, `Menu`, `Popover` and `Dialog`. Both resolve through `use_density()`: under `Density::Comfortable` each `Space` from `Xs` up is one step larger, a `Size::Row` is tall enough to tap, and a `Size::Field` grows as much as a regular button does, so a field and a button in one row stay level. `use_space` and `use_size` resolve a token to the pixels an unstyled prop takes. `Space::Custom` and `Size::Custom` are for lengths that come from content (a measured indent, a board cell, a canvas); a gap the scale already has uses its step.

A styled prop takes no raw number: `8.0` where a `Space`, `Padding`, `Size`, `Length` or `Stroke` goes does not compile. The one way to write pixels into a styled `Length` is `Extent::px`, so the source lint rejects `Extent::px` outside beui's own crates; write a `Size` (`Size::Custom` for content). When converting pixel values, snap 10, 14 and 18 to whichever neighbouring step looks right in each place. Bar popups take the popover width, `Size::Popover`. The `Custom` variants of `ColorRole` and `TextColor` are, like `Space::Custom` and `Size::Custom`, for values from content ([decision](../decisions/beui_styled_components_take_design_tokens.md)).

The focus ring is the theme's accent and cannot be turned off; like `:focus-visible` it shows only when the keyboard brings the focus there. A `FocusRing` takes the `radius` of what it rings and an `offset` `Stroke`, and rounds by the two together so the ring stays concentric.

### Text nodes

A base `Text` takes concrete values and knows nothing about the theme: a `font` (a `FontId`, 14 points proportional unless given; it carries `bold` and `italic`, both synthesised by FreeType rather than loaded as separate faces), a `color` and `underline` (painted from the galley's baseline, so switching it on moves nothing), and the paragraph props: alignment, `overflow`, `line_height`, `line_padding` (above and below each line), `marks`, `glyphs` and `carets`.

`overflow` is an `Overflow`:

- `Wrap { at, max_lines }`, the default (`at: WrapAt::Words`, no `max_lines`), breaks lines to the given width. With `max_lines` it lays out at most that many, and the last ends in an ellipsis standing for the rest. `WrapAt::Words` breaks after whitespace, `-`, `/` or `\`, or after a `break_after` span, and between letters when no such break fits. `WrapAt::Anywhere` breaks between any two grapheme clusters, so each line holds as many as fit: in a monospace font at a width of `cols` advances, exactly `cols` characters. Measure the advance as `cursor_pos(_, 1).x` of one character, since a galley's `size()` is rounded up to a pixel.
- `Ellipsis` keeps each line to one line cut to its width with an ellipsis; a newline or `Break` still starts a new line.
- `Visible` lays each line out unbroken at its natural width and paints it whole, for text inside something that scrolls or clips: a text area with wrapping off, a terminal's rows, a map label. There is no clip mode; text that has to be cut is `Visible` inside something that clips.

`on_truncated` reports whether an `Ellipsis` or `max_lines` text is cut short, after the layout that changed it, so a component can show the whole text another way (`styled::SearchList`'s tiles use a tooltip).

A Text's children are its content:

- string literals and `Prop<String>`s, in the Text's own style. A Text whose only child is a string or `Prop<String>` keeps it as its own content rather than as a span, costing no more than `Document::create_text`.
- `Span`s, value children with no node, each overriding the Text's `font` whole or any of `size`, `family`, `bold` and `italic`, plus `color`, `underline` or `strikethrough`, a `background` (a `SpanBackground`: colour, corner radius and outset) and `break_after` (a wrap opportunity). An unset span prop takes the Text's. Computed runs are a `ForEach` of `Span`s over the caller's own tokens. A newline in a span's text is a real line break that a copy keeps. A tab is its own run reaching the next tab stop, every `TAB_COLUMNS` (4) spaces of its span's font from the start of its line.
- `Icon`s: spans that paint one glyph from the icon font. An icon holds no bytes, so the caret steps over it and selection and copy skip it: `<Text><Icon glyph=ICON_SAVE />"Save"</Text>` reads and copies as "Save". `Document::text_icons` lists a text's icons. An icon is the Text's font size times `scale` unless given a `size`, is centred in its line rather than set on the baseline, and makes its line no taller than text of the size it scales from. It keeps `spacing_em` (a fraction of its own size) from whatever sits beside it, so the gap to its label is layout rather than a space a selection could take; an icon alone keeps none, and an icon with an empty glyph takes no room. Underline and strikethrough never cross one.
- `TextItem`s, whose child is laid out inline as one character (U+FFFC in the text). Given `at`, it is placed at that index as if it were a caret instead of in the flow, which is how something floats beside a text position.
- `Gap`s, room of a given width holding no bytes (the indices either side are the same).
- `Break`s, which end the visual line and hold no bytes: the index at a break starts the next line, the caret steps over it, and a selection or copy across one carries no newline, so a soft-wrapped terminal row drawn `Overflow::Visible` copies as one line.

Layout details:

- Each run is as wide as its text's advance, so spans in a monospace font stay on its grid.
- Alignment, ellipsis truncation and `line_height` apply whatever the content.
- Offered an unbounded width, wrapping text measures at its natural width, one line per paragraph. Its `min_width` (see [Develop a base component](#develop-a-base-component)) is its longest unbreakable run, a word and the space after it, so a row never squeezes it into a column of letters.
- Each span's shaping is cached, so only a span whose props change is shaped again, and line breaking resumes from that span's line. A colour or decoration change, and any change to `marks`, `glyphs` or `carets`, only repaints.

Paint order: each span's `background` behind its range on every line it is laid out on (one shape where neighbouring spans share a background, such as an inline code span split by styles); then `marks` (selections, flat); then the text; then `glyphs` (`TextGlyph`: a string in a font and colour, centred across the laid-out box of a byte range on every line it is on and set on that line's baseline, taking no room, as a text area's whitespace marker over a real space) and `carets`, blinking ones included. Marks, glyphs and carets are byte ranges over the joined content.

`measure_text(node)` answers where an index is and which index is under a point from the node's last layout (see Measurement), and its `bounds()` is where its laid-out text reaches (its natural size, not the rect it was given).

### Drawing

`Drawing` is the one base node that paints rather than arranges. It takes a `Draw`, a callback handed the `Painter` and the rectangle the node was laid out at, for content whose shape is computed (a plot, a waveform, a canvas of strokes). An arrangement of nodes is preferable wherever one says the same thing: a row of labels is a `List` of `Text`.

- Build the callback in a memo over the page it draws, so the closure is replaced only when that page changes, and cull to `painter.clip_rect()` inside it, so a document far taller than the viewport costs only the screenful shown.
- It measures to nothing, taking its size from whatever places it (a `Frame` with a width and a height, or a `CanvasItem`), unless given a `size`: then it measures to that, scaled down to the offered width and keeping its shape, as an image does. A sized `Drawing` clips its callback to its rectangle, so a callback drawing in the units of its `size` scales itself in with `painter.transformed(offset, scale)` (as the demo's Paths page does).
- Fading masks (`fade_edges`, scroll fades) do not fade custom `Drawing`s.

Text in a `Drawing` goes through its `Painter`: `painter.lay_out_text(text, font, layout)` returns a `TextBlock` (`size`, `line_height`, `baseline` from its top, and `lines()`, a `LineBox` per line with its bytes, rectangle and baseline, all from the block's top-left), and `painter.draw_text(origin, &block, color)` draws it with its top-left at `origin`, so what is measured is what is drawn. The `TextLayout` (beui-types) gives wrap width, alignment, line spacing and line height. The same `TextBlock` comes from `lay_out_text` outside a drawing (see Measurement), and `has_every_glyph()` says whether the fonts cover it. Natively a `TextBlock` is a cached FreeType galley, so the same word twice costs a hash lookup. On HTML the core breaks it into lines itself, measured with canvas `measureText` in a browser or the bundled fonts' advances (`ttf-parser`) on a server, and draws one SVG `<text>` per line with `textLength` holding it to the measured width.

`styled::Picture` (whose `radius` is a `Radius`) is the picture component for code outside beui. It can take an image larger than it is drawn, such as a 64-pixel icon in a 16-point chip: the wgpu renderer uploads each `Image` with mipmaps and samples the level its drawn size calls for, and the painting rasterizer does the same, so it is averaged rather than aliased.

### Layout scale and raster

Layout and raster are two scales, like pinch-zoom and Ctrl+/- in a browser.

- **Layout scale**, `Context::pixels_per_point`: the platform's scale times the zoom factor. A platform DPI change or Ctrl+/- changes it and lays the document out again. Text is laid out and everything snaps to the pixel grid (`Document::pixel_grid`, `Painter::pixel_grid`) at this scale; font pixel size (`round(size * scale)`), line heights, baselines, advances and wrap points come from it, and galleys are cached by it. `pixels_per_point()` is also available as a signal on the document, for layout that depends on it.
- **Raster**, `PainterState::raster`: how much finer than that a subtree is drawn (a hosted window's scale over the context's, times each zoomed canvas's `Zoom::raster`). Only painting reads it: changing it lays nothing out, but a node painted under a new raster records again. Under a raster other than 1, `Painter::galley` goes through `Context::rastered`, which asks the font backend (`FontBackend::raster`) for the same layout with glyphs at that many times the pixels: FreeType places each glyph at its laid-out position times the raster and rasterises it unhinted at the scaled size (`GlyphId::raster`, the factor's bits; `GlyphId::LAID_OUT` is 1), so outlines match the scaled advances, and `Galley::pixels_per_point` becomes the layout scale times the raster. The browser's backend keeps the galley as it is.

### Buttons and popup triggers

`unstyled::Button` shows the layer split. It is a focusable `Interactive` that owns button semantics, disabled behavior, pointer and keyboard activation and accessibility: it takes the `label`, `glyph`, `role` or `Action` and builds its own accessibility node from them. Its content closure receives a `ButtonHandle` of reactive `hovered`, `pointed`, `active`, `focused` and `disabled` state and the resolved `label`, `glyph`, `icon_only` and `tooltip`. `styled::Button` wraps it and uses the handle to choose fills and paint a focus outline.

A button that opens something (`MenuButton`, `Popover`, `SplitButton`'s arrow, the date and colour pickers) is a `PopupTrigger`: an `unstyled::Button` that toggles an `open` binding and marks itself `expanded` with the kind of popup it opens. Its `PopupTriggerHandle` carries the `ButtonHandle` as `button`.

A control that can be turned off (`Button`, `Link`, `Checkbox`, `Select`, `TextInput`, `NumberInput`) takes `disabled`: it stops answering pointer and keyboard, leaves the tab order, publishes itself as disabled to a screen reader and paints muted. A read-only editor binds it to `editor.read_only()` rather than leaving a live control that throws edits away.

The unstyled module has `Button`, `Pressable`, `Toggle`, `Choice`, `Slider`, `TextInput`, `TextArea`, `Disclosure`, `Tree`, `SearchList`, `Select`, `CommandPalette`, `ContextMenu`, `MenuButton`, `Popover`, `Container`, `PanZoom`, `PointerLock`, `Draggable`, `DropTarget`, `Tooltip`, `Floating`, `Scroll`, `Scrollbar`, `Stack`, `Calendar`, `DateTimeField`, `TimeList`, `ColorArea`, `ColorWheel` and `Picture`. Code outside beui reaches the ones that draw nothing of their own (`Draggable`, `DropTarget`, `Floating`, `PointerLock`) through `beui::styled`, which re-exports them. The styled module supplies themed buttons, icon buttons, menu buttons, links, text styles, cards, checkboxes, switches, choices, text and number inputs, a multiline text editor with find and replace, menus, popovers, split buttons, tabs, trees, a calendar, date and time fields, a color picker and input, progress, scrolls and scrollbars, tooltips, a docking workspace, and responsive layout.

`Draggable` and `DropTarget` meet on `unstyled::drag_board()`, one `Board` per document that also carries drags no `Draggable` began (an app's block, a dock bar lifted into a window). It finds targets through two hooks in `beui_view::drag`: `space()`, where each core says where a node is and what of it is visible, and `drive(..)`, which hands the board to the native core so it follows the pointer across windows, notices it leave every window, and drops or cancels on the release whichever node pressed. The HTML core ignores `drive` and leaves the dragging to the `Draggable`.

### Text areas

`unstyled::TextArea` is the multiline editor. It is built from the same nodes as everything else: a row per document line in a `VirtualList`, each a `ForEach` of the row's spans, a gutter of `Text`s beside it, and list markers, voids, embedded widgets and the caret's anchor as `TextItem`s.

It owns a `beui_text_editor_core::Core` through the `TextAreaState` its caller holds (a `Copy` handle owned by the scope that made it; `with_core` and `with_core_mut` reach the `Core`). It lays the document out with a gutter (line numbers and a border line; the area paints no background), wrapping, collapsible sections and markdown checkboxes, and lays out the inline and block `TextWidget`s the caller names: `block` builds what goes under a block widget's line, and `selected_widget` what floats under an inline one while selected. That is how a block editor embeds a block in the text and drives the same document from its own toolbar.

- **State sync.** The state republishes what it shows whenever one of its own commands runs. Nothing polls the document, so code that changes it from behind (adopting a remote edit) calls `sync()`, or `external_edit()` when the edit should also break the undo group.
- **Completion.** A `completer` names a trigger character and a search: typing it after a space opens the menu `completion_menu` draws under the caret, filtered by what follows, with arrows, Enter and Escape; `styled::TextArea` uses it for emoji (`:rocket`). The menu is a passing layer with an `on_dismiss`, so Escape, back and a press outside close it alike. The styled find bar takes `on_back` while open, so back closes it as Escape does.
- **Modes.** `placeholder` shows while the document is empty; `password` masks every character; a touch tap leaves a caret handle that drags the caret and opens its menu. `disabled` takes the area out of focus, pointer and keys and reads as disabled. `read_only` keeps it focusable and selectable (arrows, Ctrl+A, copy, find) but refuses every edit (typing, IME, paste, cut, undo, checklist boxes, the duplicate-line and replace actions, the text menu's Cut and Paste) and reads as read-only. `find=false` registers no find actions, so Ctrl+F, Ctrl+H and Ctrl+G reach the surrounding program.
- **`single_line`** lays it out on one unwrapped line with no gutter, scrolled sideways to keep the caret in view; Enter submits and Tab leaves. `frame` wraps the field in the caller's chrome inside the area's own focus and pointer handling.
- **`TextInput`** is that single-line mode over a plain-text buffer it owns, bound to a `value`, so a fix to text editing lands in both. `multiline` makes it the multiline area instead, with `line_numbers` off, Enter starting a new line, and the chrome wrapping the whole scrolling area. A `TextInput` face gets `clear` and `copy` in its `TextInputHandle`, so `styled::TextInput`'s `clearable` and `copyable` buttons only draw them.
- **`prose`** (a `Prose`, on `TextArea` and `TextInputStyle`) lays the multiline area out as plain text rather than a document (no whitespace markers or document padding), naming the weight, horizontal alignment, padding around each line, wrapping, and where lines sit when shorter than the area.

`styled::TextArea` takes the same `disabled`, `read_only`, `focused`, `line_numbers`, `prose`, `on_content_size` and `find` (false also drops its find bar), plus `size` for body text (a `TextSize`, `Custom` for a size from content), `color` (a `TextColor`: `Default` keeps the theme's syntax colours and muted placeholder; any other draws all text in it with the placeholder a faded copy) and `padding` (a `Padding`, `Space::Custom` for an inset from content; the unstyled area takes `Sides`). It has no font-size or scale prop: text drawn zoomed sits in a scaled space.

**Sizing.** Text is measured only from the tree: a `TextArea` reports the size its rows reach through `on_content_size` (padding included, in the area's own units even inside a scaled space), and a caller sizes a box from that, as the infinite canvas grows a text entity while it is edited. A `TextArea` takes its view size from its own layout, not its place on screen, so one inside a scaled `CanvasItem` is not laid out again when the view pans or zooms; a `VirtualList` reads the clip only while some of its rows lie outside it. The canvas draws each text and label as a rich `styled::TextArea` over a `LiveText` in the entity's scaled item, edited or not (disabled while not, `find=false`), so a checklist shows the styled `Checkbox` at its text's size (a `Checkbox`'s `size` is the `TextSize` it sits beside). The canvas's export is the one place text is laid out away from the tree (`TextAreaState::lay_out_prose`): the area hands each line's `Piece`s and `RichOptions` (plain values in `beui-types`) to a layout function the caller passes (the canvas's painter's `rich_layout`), so the area names no core's layout. Export does not draw markers or quote bars yet: it gives each marker a line-high square, and the canvas paints those and the bars as pink boxes.

### Rich text

A document whose language is `TextLanguage::RichText` is structured the way Slate structures one: the document knows nothing of bold or headings, a `Schema` (beui-text-editor-core `schema.rs`) gives keys their meaning, and the page renders the result.

- The bytes are the text alone, and every line is a block. Formatting is a set of `Marks` beside the bytes, each setting one key to a `Value` or clearing it, over a stretch of text (`Scope::Inline`) or whole lines (`Scope::Block`, a block's props).
- Marks are anchored to the sequence's positions, the latest wins where two overlap, and two copies exchanging marks in any order agree (the demo's Rich text page sends them over a simulated network).
- Undo removes the marks an edit made (`RichOp::Unformat`, which a copy sends on like a mark, and which a mark arriving after its removal cannot bring back) rather than marking the old values over them, so text an undo brings back never falls under a mark made for the edit, and a concurrent mark from another copy stays. Redo makes those marks again under new ids.
- `Marks` keeps the resolved `Formats` and updates them from each splice and new mark rather than resolving the document again, so a document holding marks passes every splice its sequence reports to `Marks::splice` (and calls `invalidate` if it replaces the sequence wholesale).
- Each inline mark records whether it grows when text is typed at its end (`Expand::After` for bold, `Expand::Never` for a link). Typed text takes the growing marks before it (or after it at a paragraph's start).
- `Core` owns only generic operations: `EditorCommand::SetMark` styles the selection or what is typed next at a caret, `SetBlock` sets a prop on the selected blocks or one at a position, `InsertVoid` puts an atomic inline item (U+FFFC carrying its own marks, such as a mention) at the caret, and `Transform` runs a function over `Transform`'s range-level operations as one undo step.
- The schema decides what Enter (given the line, its neighbours and whether it is empty), Backspace at a block's start and Tab do to a block; its tools (a name, a shortcut, whether the selection has it, the command it runs); each leaf's `SynHlStyle`; and which blocks a highlighter decorates (a code block's language). `schema::basic` has the usual marks, headings, lists, checklists, quotes and code blocks.
- No block traps the caret at the document's edge: Down at the very end of a document whose last line has props, Up at the very start of one whose first line has, or a click below the last line opens a plain line past it, and a press there anchors a drag at that caret, so dragging up selects from the end.
- `TextAreaState::with_schema` makes an area for a schema, `TextAreaState::selected` is the selection's marks and block props, and `styled::FormatBar` shows the tools it is given glyphs for. Copy and paste carry the text without its marks.

The area draws rich text with no gutter or whitespace markers. In markdown and plain text, whitespace stays real text (a space and a tab take their own advance in their own font, a `\r` none), and the area shows markers as the row `Text`'s `glyphs`: trailing whitespace always, any other only while selected, so selecting never moves text. A line's newline takes the width of an invisible `mm` in the line's font after its text (the caret at the line's end sits before it), so a selection over it shows, and its marker is drawn centred there while selected.

### Text area looks

Three render hooks, as in Slate, each keyed by the line description (`LineLook`) that markdown and rich text both give:

- `leaf` (`RenderFn<TextLeaf, Span>`) turns a run of text, its resolved `SpanStyle` and its `SynHlStyle` into a `Span`, so every run stays in the line's one `Text` and wraps and carries the caret across style changes. The leaf draws an inline code span's background (`SynHlStyle::code`, set by the markdown highlighter for a code span and by `schema::basic` for the code mark, never for a code block) as the span's `background`; `LeafSpan` is the span with a given background.
- `marker` (`RenderFn<TextMarker>`) draws a list marker (bullet, number, checkbox with a `checked` binding that toggles it) or a void (`MarkerKind`) as the content of a `TextItem`.
- `line` (`RenderFn<TextLine>`) wraps a line's text node in whatever goes around it (quote bars, a code block's background, a list's indent), keeping the horizontal padding it is handed.

The engine says what is on a line, never how big: each marker is measured by its `Text` during layout, and wrapped lines hang at the laid-out x after it. A markdown line's marker covers its leading indentation and its `>` and list marker bytes, so the marker item draws them (the caret still steps through them). Both kinds of list look the same: `MarkerColumn` puts a list marker in a column `MARKER_COLUMN_EM` of the text size wide, right aligned so numbers line up on the dot (a wider marker widens only its own line), and `TextLine::indent` indents by `LIST_INDENT_EM` per nesting level and `QUOTE_INDENT_EM` per quote, whatever the source's spaces. A markdown line continuing a list item (`LineLook::continues`) hides its indentation the same way and is indented to its item's text.

The three hooks come in a required `TextAreaLook` beside the required `TextAreaColors`, `font_size` and `padding`, with the area's `TextAreaGeometry`: every absolute size it draws or lays out with (caret width, padding around a line, room below the last line, the gutter, run chips, inline widget pills, and block spacing and table cell padding, given for `BODY_SIZE` text and scaled with `font_size`). Unstyled keeps only proportions of the text size (list and quote indents, margins, heading sizes). `TextAreaLook::plain(geometry)` draws text and nothing else, which `TextInput` uses with `TextInputStyle::geometry`. `styled::text_area_colors(&theme)`, `styled::text_area_geometry()` and `styled::text_area_look()` build them for a theme; the styled look draws with `DocumentLeaf`, `DocumentMarker` and `DocumentLine`, and a caller that draws voids handles `MarkerKind::Void` and hands every other marker to `DocumentMarker`.

### Menus and anchored popups

Callers do not choose between a dropdown and a sheet: every menu opens as a dropdown when a mouse opened it and as the same items in a `ModalSheet` when a finger did ([../decisions/beui_anchored_popups_sheet_on_finger_touch.md](../decisions/beui_anchored_popups_sheet_on_finger_touch.md)). That choice is `unstyled::Anchored`'s, the surface every anchored popup sits on (`MenuPopup`, `Select`'s options, `Popover`, and through it the date and colour pickers): an overlay under `anchor` at `placement`, or the same content in a sheet.

- It reads whether the press that opened it was a finger's (`touched_by_finger`, `Document::touched_by_finger`: whether the last press was a finger's, until a key is pressed; a pen counts as a mouse), so no trigger tracks touch itself. `sheet` (`SheetWhen::Touched` by default, `Always` or `Never`) overrides it.
- Its `AnchoredStyle` takes both looks with no default: `dropdown` wraps the content in a panel, and `sheet` renders an `AnchoredSheetHandle` (`open`, `on_close`, and `content`, a `Render` the sheet builds inside itself so focus lands within it), styled as a fitted `ModalSheet` (`styled::popover_surface(max_width)` for a popover's).
- Its content is a `RenderFn<AnchoredHandle>`, built once per look the first time that look opens; the handle's `sheet` says which, and `shown` holds while that one shows, so focus binds to `shown` rather than to `open`.

A `MenuStyle` carries a menu's rows for both looks and the row spacing (`MenuStyle::new(spacing, row, panel)`; the styled `menu_style()`); an unstyled one without a sheet stays a dropdown. Every component with a menu has a required `MenuStyle`; a text field's context menu is `TextInputStyle::menu: Option<MenuStyle>`, where `None` means no menu.

Menu items:

- Children cascade as a submenu in a dropdown; in a sheet, or with `page`, they open in place of the menu as a page led by a back row titled by the item (`<id>.back` for an item with a `row_test_id`). Left, Escape and the back row return to the item, and the dropdown or sheet resizes to each page.
- `then` says what choosing does: `MenuThen::Close` (default), `Back` to the page before (a location picked on a page), or `Stay`.
- `MenuCaption` is an unchoosable row of text. `MenuGrid` lays its child items out as tiles in `columns`, walked by the arrow keys in two dimensions and left for the rows above and below.
- A page of rows with a `depth` is a tree: `expanded`, a `Binding<Option<bool>>` (`None` for a leaf), draws the chevron; Right, Left and the chevron write `Some(open)`, and the caller shows open branches' rows (a `ForEach` over them), as with `Tree`.
- A row whose `checked` is `Some(true)` shows a check (with `radio`, read as one choice of a group), and a page opens with the keyboard on that row.
- A row with an `action` takes its label, glyph, enabled state, tick and first shortcut (drawn muted at the row's end, or `shortcut` given) from it. `MenuNodes` builds rows from a description of actions, submenus and separators (`MenuNode`, see guides/beui_keyboard.md, Actions and the command palette), each row's test id `<test_prefix>.<id>`, so one description fills a context menu, the dock's More menu and an app's frame menu.
- `lead` is a few muted words before an item's icon on the same line, read with its label (`New in` before a folder's name).

The dropdown is as wide as its widest row, between a least and a greatest width past which labels end in an ellipsis; as tall as its rows up to the window's height, scrolling beyond; and it scrolls the keyboard's row into view.

- `MenuPopup` is the menu opened by something other than a button, such as a model holding a request: it shows while `open` holds, under `anchor` at `placement`, as a sheet when a finger opened it or `sheet` holds, at least `min_width` wide, and reports `on_dismiss` for Escape or a press outside. A host that opens one for an app's button (the host's menu) anchors it to the button's rect, which the app sends from `node_anchor`; the host routed that press, so the surface knows whether a finger made it.
- `MenuButton` opens a menu under itself, for a toolbar where `Select` would imply the choice sticks. Its face is text, an icon (`icon_only`) or a picture (`image`, the tray's icons), in any `ButtonSize`, and it looks pressed while its menu is open.
- `ContextMenu` is the same menu on a secondary press, and takes an `open_at` point so a touch gesture can raise it where the finger was. A text selection's menu sets `selection` and stays beside the selection under a finger too.

### Touch and pen input

- **Long press.** A finger held still for the long-press delay (`Context::set_long_press_delay`, which a test sets to zero) is a secondary press where it rests, so every context menu opens on tap-and-hold; the press the finger began is cancelled and lifting it is not a click. An `Interactive` hears that cancellation, and a second finger landing, as `on_cancel`, where a gesture in progress is dropped rather than committed.
- **Drags become scrolls.** A finger dragged across an `Interactive` scrolls whatever holds it that scrolls that way (where nothing does, it stays the catcher's drag), unless the catcher sets `touch_drags` (a `TouchDrags`, CSS's `touch-action` turned round): `TouchDrags::All` (or `true`) for a canvas that draws or moves things under the finger, or `TouchDrags::Along(direction)` to keep drags along one direction and leave the other to the scroll around it (a dock tab in a scrolling tab bar).
- **Touch reach.** A finger that lands on no control goes to the nearest `Interactive` that takes presses within `TOUCH_REACH`, for the whole touch, so every control's touch zone is bigger than it looks. A direct hit always wins, so a neighbour never takes a tap aimed at the control beside it. A catcher that only watches presses over a region (a dock pane focusing itself) sets `claims_touch=false`, so landing on it is not a direct hit and the controls inside keep their reach.
- **Finger taps.** A quick tap with two or more unmoving fingers is a finger tap, which `on_finger_tap` hears the way `on_shortcut` hears keys, unless a layer that blocks everything is open; the editor frame's top bar undoes on two and redoes on three.
- **Pen.** `Event::Pen` is a `PenPhase` (`Move` in range, tip down or not; `Down` and `Up` of the tip, which is `Primary`, and of its barrel buttons; `Gone` on leaving range) and a `Pen` with pressure from 0 to 1, tilt in degrees and whether it is the eraser end. It is `PointerId::Pen` and behaves as the mouse does: it hovers, moves the pointer, presses and drags with no slop or long press and never scrolls what it is dragged across, so every control works with it unchanged (`PointerId::hovers` is true for both). Its `PointerEvent`s carry the `Pen`, and the `PointerPress` a catcher hears carries it as `pen`, sampled at the press and each drag; a finger's `PointerEvent`s carry its `force` where the platform reports one. Sources: libinput's tablet tools on DRM, pointer events with `pointerType` `pen` on the web, a stylus or eraser `MotionEvent` on Android, winit's tablet-tool events on Wayland and Windows (X11 and macOS report none), and drive's `pen` commands.

### Sheets

`Sheet` is the panel that rises from the bottom of a narrow screen. It scrolls what it holds itself, so its content is not wrapped in a vertical `Scroll` (one inside would measure nothing tall).

- A swipe anywhere on it raises it to its top stop before scrolling the content, and lowers it once the content is back at its start. It is never taller than its content, and pulled past its top it stretches like an overscroll. Let go, it springs to the stop nearest where the flick was heading, or closes below the lowest; its content flings and bounces as a `Scroll`'s does, and a fling reaching the content's start bounces the whole sheet without changing its stop.
- Dragging the handle (mouse too), or the backdrop above a modal sheet, resizes an unscrolled sheet between its stops. When the content is scrolled, the same drag translates the whole sheet instead, and release returns it to fully open or dismisses it. Going back closes the sheet.
- It slides up easing out and down easing in from the flick's speed; a `ModalSheet` stays up while it leaves and fades its scrim with it. A sheet opening within a moment of another closing in the same document (a menu's sheet giving way to the dialog it opened) takes the other's place, starting at the height the other showed (`Slide::Morphing`), so the two read as one sheet changing content.
- Styled sheets fade content at edges with more to scroll; unstyled sheets take a concrete `fade` length. The sheet's offset is beui's on both cores.

### Scrolling

Scrolling exists at all three layers, and program code wants the styled one. `styled::Scroll` is a scroll with the project's scrollbar over its edge, so a panel that scrolls is one tag:

```rust
view! {
    <Scroll @sizing=ItemSize::Percent(100.0)>
        <Rows />
    </Scroll>
}
```

`Scroll` takes a `direction`, so the same tag is a column of rows or a strip of cards. A horizontal one answers Shift+wheel, a sideways trackpad swipe, a touch drag and the left and right arrows; a plain wheel is left to whatever is around it, as a browser leaves a horizontal strip alone. A wheel reaches only the innermost scroll under the pointer. `on_change` reports the position.

**Padding and bleed.** Moving the focus scrolls the control into view with its ring, but a ring drawn outside its control needs room inside the scroll at the content's edges, which is why padding goes inside a scroll. A scroll that is one part of a padded surface, its content lined up with what sits beside it, takes `bleed` instead (`Bleed::POPOVER`, `Bleed::CARD`, `Bleed::DIALOG`, with `.top(false)` and siblings to keep a side in): the scroll and its bar reach into the surface's padding up to its border, and the content is padded back in by as much. The unstyled `Scroll` takes the same as `Sides`, built from a `Frame` with that much negative padding around it.

**Length of content.**

- Content shorter than the scroll is laid out as long as the scroll, like a browser's `minmax(100%, max-content)` track, so a percent-sized child (a `Spacer`, a field filling a toolbar that scrolls when narrow) fills the rest of it whether or not the content overflows.
- Content longer than the scroll is laid out as short as it can be without overflowing, down to the scroll's length, like a flex box in `overflow: auto`: shrinking children give up length first, and only what is left past that scrolls. How short is `Element::least_length` (`layout::least_length(doc, painter, id, direction, cross)`): a list counts children that neither shrink nor are `Percent` at their natural length and the rest at their least length (a scroll along the same axis at nothing); text that wraps or ellipsises counts its `min_width`; a frame adds its padding; a node that does not say counts its natural length. Unlike `min_width`, it never undercounts. It walks the subtree once, stopping at scrolls along the same axis, so a full layout stays linear, and is memoised per direction and cross length (`FrameWork::least_lengths` counts nodes asked afresh, `reused_least_lengths` answers reused). `ToolPill` lets its content shrink this way, so a `.shrink(..)` child in a bar narrows before the bar scrolls (the desktop's Windows bar scrolls only its chips, its grip staying put, along the bottom or down the side).
- With `fit` it measures as long as its content, so a content-sized box can still scroll once squeezed. `max_length` (a `Prop<Option<Size>>`, so a signal or memo can move it) fits and bounds it: as long as its content up to that length, then scrolling, as a popup listing a few rows or many wants. A `Frame` with `max_height` around a scroll does not do this, since a `Frame` with a maximum fills the space offered up to it.

**Following the end.** With `follow_end` (a log, a terminal) the scroll starts at its end and stays there while its content grows, as long as it was at the end: scrolling away leaves it put, and scrolling back to within a point of the end follows again. `ScrollApi::scroll_to_end()` (from the `api` prop) puts it at the end and, if it follows, follows again (a terminal does this when you type). Both are the base `Offset`'s (`follow_end` and `Document::offset_to_end`): while pinned to the end it lays its content out from the end it measured, skipping the anchor and any shift its content reports, and tells the content through `Document::pinned_to_end` while placing it. A `VirtualList` along that direction then builds its rows from its last one upwards, measuring each, and lines the last up with its own end, so rows that measure differently from the estimate change only the length above them and the next layout settles on the same rows.

### Scrollbars

`unstyled::Scroll` owns the base offset, the input that drives it, the position it reports, and the layer that lays the bar over the content's far edge. It takes a `ScrollbarStyle` saying what to put there: the width of the strip the bar covers and a builder handed a `ScrollHandle` of the live `position`, the `direction`, a `scroll_to` that drives the offset as the wheel and arrow keys do, the bar's `visibility`, and an `on_engage` to report the pointer on the bar:

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

Without a bar style the scroll shows no bar. A control that scrolls something of its own takes a `ScrollbarStyle` and passes it down (as `unstyled::Select` does to the scroll behind its options), so the styled control decides the bar and the unstyled one names no colour.

The bar is an overlay, as on macOS: it takes no space and reserves no gutter, so a scroll that grows past its viewport does not shift its content ([../decisions/beui_overlay_scrollbars.md](../decisions/beui_overlay_scrollbars.md)). A scroll keeps its content clear of the bar, if needed, by padding inside it. `visibility` goes to 1 when the offset or overscroll changes and while the mouse is within the strip's width of the edge or on the bar (`on_engage`, which `unstyled::Scrollbar` reports for its whole rect and while held), stays there for `SCROLLBAR_LINGER` after the last of those, then falls to 0 over `SCROLLBAR_FADE_OUT` on a timer that stops once hidden. `unstyled::Scrollbar` takes it as `shown` and turns down every point while it is false or the content fits, so a press or a finger on a hidden bar reaches the content; a finger can drag the bar only while a scroll has just shown it. The bar paints nothing while the content fits. The styled bar fades its colours by `visibility`, paints only the thumb while the scroll moves, and adds the track while engaged.

`unstyled::Scrollbar` answers the pointer itself: it takes the position, direction and an `on_scroll_to`, hit-tests the press against the thumb it would paint, drags the thumb, pages by a viewport towards a press on the track either side, and hands its content a `ScrollbarHandle` of `hovered` and `dragging`. `thumb_start` and `thumb_length` are the fractions both layers work in, so what is painted and what is pressed cannot drift apart. `ScrollbarThumb` paints the thumb at those fractions of its rect rather than laying it out, so a new position reaches the bar without a layout; a bar of your own puts one inside its track. The styled bar paints its track inset by `SCROLLBAR_INSET` from the scroll's outer edge and both ends, clear of the separators around a panel; the end insets sit outside the unstyled bar, so the fractions are measured over the painted track. A press on the bar stops any fling momentum.

`ScrollbarStyle::fading(length)` also fades the content toward each edge that has more beyond it, over at most `length` points and only as far as the content has scrolled, so rows that end exactly at the edge still read as scrollable. The styled style fades by `theme::SCROLL_FADE`, so `styled::Scroll` and every styled control that scrolls fade. `unstyled::Scroll` works out the widths from its position and wraps the offset in a base `Frame` with those `fade_edges` (a mask group the wgpu renderer applies in the shader).

### Offset, overscroll and anchoring

Under both sits the base `Offset`: it shows its one child along a direction from an offset, with no bar, theme, fade or input. The wheel, touch drag and arrow keys are `unstyled::Scroll`'s, which puts its children in a `List` inside the offset, wraps that in a focusable `Interactive`, keeps the momentum of an unfinished fling, and drives the offset from all three. Use `Offset` for content shifted under a viewport and nothing more, and a `Scroll` for anything that scrolls.

`Offset` never clamps: it shows its child at whatever offset it is given, past either end included, and the overscroll is the part beyond them (`ScrollPosition::overscroll`, with `clamped` the part within). Whatever drives it keeps it in range. When content shrinks under a `Scroll`, or an offset is set past an end while nothing moves it, the `Scroll` puts it back at the end in its report of that layout and holds the report back, so the next layout pass of the same frame reports the settled position and nothing past the end is painted.

A `Scroll`'s momentum is one motion across the edge: it keeps how far its content has gone past an end unbanded and shows `rubber_band` of it. `rubber_band::bounce` moves that stretch, an undamped spring outward (a fling keeps its speed across the end) and a critically damped one back from its furthest point; a band let go while heading back hands its speed to a fling into the content.

**Anchoring.** The offset is anchored to a node, not measured from the top of the content: the scroll remembers the node at the top of its viewport and where it started, so a row further up growing or shrinking (a wrapping label, an image that finished loading) leaves what is being read in place. It finds that node by walking down from its child through nodes that lay children out in place (`Element::passes_scroll_anchor`: `List`, and a `Frame` neither translated nor rotated), taking the first child that reaches past the viewport's top each time. What the content's layout decides about the anchor comes back as part of that layout's result: a `VirtualList` whose rows above the view measured differently from their estimate reports the difference with `Document::report_anchor_shift`, and the nearest enclosing offset sums that with the anchor's drift and any requested reveal. A non-zero sum moves the offset, laid out again in the frame's next layout. The anchor is picked once a frame, from the layout that was painted, and drift is measured against it in each of the frame's layouts, so content that changes in a later layout (a `Container` that heard its new width) keeps it in place too. Anything that moves the offset from outside its layout (`set_offset_value`, a reveal) calls its `on_steer`, which a `Scroll` uses to stop a fling or spring.

### Scrolling on the HTML core

`unstyled::Scroll` leaves scrolling to the browser (`if CORE == Core::Html` in scroll.rs, `BrowserScroll`): it builds a real `overflow: auto` `<div>` through the [platform escape](#the-platform-escape), with the content slotted inside and `scrollbar-width: none` when its `ScrollbarStyle` has no bar. The browser scrolls it on the compositor (wheel, touch, keys, its own scrollbar), with none of beui's momentum, rubber band, autoscroll or drawn bar. Its `scroll` listener and measured sizes feed `on_change`; `offset` and `ScrollApi::scroll_to_end` set `scrollTop` (or `scrollLeft`); `follow_end` and `reveal` work as natively; the focusable `Interactive` sits on the scroller itself; `ScrollApi::animating` is always false. Revealing a node with no beui `Offset` around it uses the browser's scroll-into-view, and a `VirtualList` is clipped by any clipping ancestor, so both work inside such a scroller.

### Long lists

Layout skips the work for nodes far from the view. Every node is still measured, since sizes need them all, but `layout::layout` *culls* a node whose rect ends more than `CULLING_MARGIN` outside the visible part of its parent's painter: it keeps its rect, so `measure`, `reveal_node` and `Document::node_rect` still find it, but nothing inside it is laid out, painted or interacted with (`Document::is_culled`). This holds for any container's children. The node holding the focus and its ancestors and descendants are never culled, and a `Canvas`, which lays out only the items its view reaches, always lays out the item holding the focus, so a field panned out of sight or under an on-screen keyboard keeps the focus and its input method. Each placement records its `Sight`, the range of visible regions it stays right for (a node that reads its space through the painter is right only exactly where it was), and a parent's sight takes in its children's, so a scroll lays a subtree out again only once a node in it crosses the margin. A long scroll of ordinary content needs nothing more; a `VirtualList` is for when building every row is itself the cost.

A `VirtualList` takes `keys` as `ForEach` does, reports one `item_size` per key as its own length, and builds only the rows its slice of the enclosing viewport reaches, so it goes wherever a tall child would: directly under a `Scroll`, beside siblings in one, or nested a few containers deep.

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

**Sizes.** `item_size` is an estimate; a row that measures differently is laid out at its measured size. The list remembers what each built row measured and reports the sum, counting unbuilt rows at the average, so its length is close from the first layout and the last row can always be scrolled to. A row measured above the viewport would move everything below it, so the list keeps drawing from the row it drew from last frame and asks the scroll to move its offset by the difference, which the scroll applies before placing anything. Outside a scroll a `VirtualList` builds what fits in its box (one sized by a `Stack` builds a screenful in one layout and a hundred pixels' worth in the next). `overscan` builds that many pixels of rows beyond each edge, so the row a key press moves to already exists. A list scrolled to its very top shows a row inserted above the rest rather than holding the rows below in place. Changing `item_size` keeps the rows but forgets measured sizes; rows in view are measured again as laid out.

**Rows belong to keys.** Scrolling reuses rows that stay in view and keeps those that just left (as many as are in view, while within a window's length), so scrolling back does not rebuild them; anything further is disposed with its effects, so a long jump or a list scrolled out of view keeps only the row holding the focus. That row stays a child of the list, laid out at its own place off screen, until the focus leaves it. Changing `keys` keeps every row whose key remains: an inserted key shows only its row, and only if it lands in view; a removed one releases only its row; measured sizes move with their keys. A key inserted or removed above the view moves the scroll's offset by the change, so rows on screen stay put. Data addressed only by position can pass `(0..count).collect()` as keys.

**Recycling.** A disposed row is recycled: the item builder gets a `ReadSignal` of its key, and when a key comes into view the list sets that signal on a row that left instead of building a new one. The builder runs once per row built, and the key signal changes once per key shown. So the builder has to derive everything it shows from the key signal (a memo of the label, a test id, the selection) and read it untracked in event handlers, and any state it keeps of its own carries over to the next key. A row that should start fresh per key wraps its content in `<Dynamic value={key}>`, which rebuilds on a key change at the cost of a full build. A row holding focus is never handed another key, and the list keeps at most as many spare rows as it has ever shown at once.

**Focus.** Tab order runs over every key, built or not: Tab and Shift+Tab reaching unbuilt rows build them one at a time from the end they come in at until one has a Tab stop (rows with none are released, and the search gives up past 4096), and the focus then reveals that row. Elements opt into this through `Element::focus_children`, `build_for_focus` and `release_for_focus`.

**Leaving layout.** A list the layout stops reaching (scrolled out of view beside other content, or in a scroll that collapsed) is told through `Element::unplaced`, which layout calls on every node it placed last pass and not this one; the list releases its rows there.

### Selecting text in lists

`Selectable` (`beui::primitives::Selectable`, with `SelectableState`, `copy_selection` and `select_all` in `beui::selectable`; `styled::SelectableText` over it) is each core's own read-only selection: the browser's selection on HTML, and `beui-view/src/native/selectable.rs`, built on `beui_view::selection`, natively. The native one selects across the plain `Text`s under it and across any `VirtualList` under it. Each `VirtualList` registers its rows (`beui_view::selection::SelectableRows`: its keys, which key each built row shows, and its optional `text` prop, a `Callback<K, String>` used only for selection), and the selection walks the region in tree order, taking a list as one item.

- A caret inside a list is a key, the index of the `Text` within that row and a byte index, never a node, so a recycled row cannot carry a selection to another key. A press maps the built `Text` under it to its row's key; every built row paints which bytes the selection covers, including rows built later.
- Select All goes from the first key's start to the last key's end without building anything. Copying walks the keys between the ends, reading built rows from their `Text`s and the rest from `text`, one line each (a row's several `Text`s are joined by line breaks; a list with no `text` copies unbuilt rows as empty lines).
- A key removed under the selection moves that end to the nearest remaining row (the start forward, the end backward), and a row whose text shrank is clamped to it.
- Dragging past the top or bottom of the nearest vertical scroll around the region scrolls it while held there (`repeat_drag`).
- `selected_text` reads the document, so it works only from a handler or inside `with_reactive_scope`.

### Slider scales

A slider spreads its range evenly along its track unless given a `scale`. `SliderScale::Midpoint(value)` curves it so the centre of the track reads that value, giving most of the track to values bunched at one end. The curve is exponential: the inspector's blur slider runs to 120 px with a midpoint of 12 and reads 3 px, 12 px and 39 px at the quarters, spending a tenth of the track below one pixel ([../decisions/beui_slider_midpoint_exponential_curve.md](../decisions/beui_slider_midpoint_exponential_curve.md)). The midpoint may sit above the centre of the range, giving the top end the fine part instead. A midpoint outside the range, or one where the centre already is, leaves the slider linear.

The curve applies wherever value and track meet: dragging maps the pointer through it, the knob sits where the value falls on it, and keyboard steps move by a share of the track, so an arrow key moves a little near the fine end and a lot near the coarse end. The step a screen reader is told follows the value the next step would reach.

A slider given a `thumb` length maps the pointer to the thumb's centre, which travels the track inset by half of it, and a press on the thumb drags it from where it was grabbed rather than jumping; `ColorArea` takes the same prop for both axes.

### Dates, times and colors

`beui::datetime` holds the plain values the pickers trade in: `Date`, `Time` (to the minute), `DateTime`, `Weekday` and `HourCycle`. They carry no time zone; `DateTime::from_unix` and `to_unix` read and write seconds as UTC, which is what block content stores.

`styled::DateTimeField` is the field for any of them, chosen by `parts` (`DateTimeParts::Date`, `Time` or `DateTime`): a row of spin-button segments (year, month, day, hour, minute and, on a 12-hour `hour_cycle`, AM/PM) beside a button that opens a popover holding a `Calendar`, a `TimeList`, or both.

- A click anywhere in the field, or Alt+Down in a segment, opens the popover over the field with an editable copy of the segments on top, focused on the clicked segment, so typing goes on while the calendar shows the month being typed; closing it returns focus to the field's segment.
- The popover is laid out for what it holds: a calendar that grows with the field, a `TimeList` as a grid with an hour to a row, the two side by side, or on a narrow screen Date and Time tabs where picking a day moves on to the time.
- `value` is a `Binding<Option<DateTime>>`: an empty field shows placeholders, the binding is written only once every segment is filled, and a half-filled field reverts when focus leaves. A `Date` field keeps the time of its value, a `Time` field the date.
- The behaviour (the popover, its focus, what each pick does) is `unstyled::DateTimePicker`; the styled field gives it faces and the popover's layout, and says when to page with `paged`.

`styled::Calendar` is the month grid alone, with `min` and `max`; its title is a month button and a year button, opening a grid of months and a grid of twenty years, and `show` moves it to a month without selecting.

`styled::ColorPicker` is a saturation and brightness area, hue and opacity sliders, hex, RGB and HSL fields and a row of swatches; `styled::ColorInput` is a hex field whose swatch opens one. Both keep the hue while the color passes through grey or black, and report a drag through `on_preview` while it moves and write `value` once when it ends, as `NumberInput` writes a scrub, so an edit lands in undo history once per gesture. That state is `unstyled::ColorPickerState`, and `unstyled::HexText` keeps a hex field in step with a color. `unstyled::ColorInput` holds the preview and puts the popover and hex field together; its `swatch`, `picker` and `field` renders draw the parts and its children lay them out.

`styled::SwatchButton` is one round swatch picking one colour, for a row of presets that is not a whole picker: a button labelled with the colour's name (its tooltip too), ringed in the accent while `selected` and in the border colour under the pointer. `radius` is `Full` by default; a palette of square cells passes `Radius::Xs`.

`styled::ColorWheel` is a hue ring around a triangle whose tip points at the hue. `styled::OklchColorWheel` is the same over OKLCH: the triangle is the lightness and chroma plane, black to white with its tip at the hue's cusp lightness (`Oklch::cusp`) and a chroma past every hue's cusp, so it holds all of sRGB. Turning the hue keeps lightness and chroma. A point outside sRGB is painted and reported clamped to the most colorful color at its lightness and hue (`Oklch::clamped`), and a line on the triangle marks where sRGB ends. Both share `unstyled::ColorWheel`, which works in `WheelPoint`s (hue plus the triangle's saturation and value) and leaves the color model to its caller; `ColorPickerState` is generic over the model (`Hsva` or `Oklch`). The faces are a `Drawing` painted by a shader on the wgpu renderer, behind beui-components-styled's `gpu` feature; without it (the HTML core) they are computed on the CPU into an `Image`.

`unstyled::Popover` is what both open: a trigger and an overlay, built the first time it opens, that traps Tab, closes on Escape, a press outside or its handle's `close`, and returns focus to the trigger on close. It dims nothing, so its layer passes input through (`LayerPolicy::Pass`): the pointer keeps hovering the document, and a press outside asks it to close and then lands on what was pressed, except a press on its own `trigger`, which is left to the trigger so a second click closes rather than reopens it. Whether it is open is its `open` binding, as with `MenuButton`: a caller that opens it or reacts to it passes a signal rather than listening for a change. Its content decides where focus lands on open by binding a `focused` prop to the handle's `shown`, as the calendar, time list and color area take. It is an `Anchored`, so a finger opens it as a sheet; its required `surface` is the `AnchoredStyle`, so its content is only what goes inside the panel (`styled::popover_surface`).

### Tooltips

`unstyled::Tooltip` shows something after the pointer has rested on its child for a moment, and `styled::Tooltip` is the bubble version:

```rust
view! {
    <Tooltip label="Reveal the block being shown">
        <Text color=TextColor::Muted><Icon glyph=ICON_LOCATE_FIXED /></Text>
    </Tooltip>
}
```

An icon-only button needs no wrapper: `unstyled::Button` and `unstyled::Toggle` take `icon_only`, and their handle's `tooltip` is the label (with the action's shortcut) while the button is icon-only and nothing it opened is showing, and empty otherwise. A button's `description` (why it is disabled, say) is its accessibility description and a line of the tooltip under the label, on any button. The styled faces wrap themselves in a `Tooltip` of that text, so the `label` a screen reader hears is what the bubble shows, and a menu button's tooltip goes away while its menu is open. A styled icon-only button is at least `Size::Md` square however small its glyph. Anything else whose meaning is not on screen (a bare glyph in a row, a truncated cell) can take a `Tooltip` of its own.

The bubble lives in an inert `Overlay` (`LayerPolicy::Inert`): it paints above what it hangs from but takes no input, so the document under it keeps answering the pointer, Escape reaches whatever it would have, and clicking the described control clicks it. The dwell is a `create_timer`, started when the pointer arrives and stopped when it leaves, so an unhovered tooltip costs no frames.

### Time: timers and animation

beui has no per-frame hook. Work runs because a signal it read changed, and anything that depends on time asks for it with `create_timer(work)`. The timer belongs to the reactive scope that created it and is dropped with it; the `Timer` handle is `Copy`, and calling it after that scope is gone does nothing. `start(delay)` schedules `work` (keeping an earlier deadline if set), `restart(delay)` replaces the deadline, `stop()` cancels it, and `running()` says whether one is pending. The document runs due timers inside its reactive scope at the top of a frame and asks the context for a repaint at the next deadline, so nothing is drawn while no timer is due. `work` returns `Some(delay)` to run again (an animation returns `Some(Duration::ZERO)` for the next frame) and `None` once settled, which is how a fling or an indeterminate progress stops costing frames. Starting a timer from the event or effect that begins the motion keeps an idle program from waking.

`create_animation(step, finish)` is the tool for UI motion. Its scope-owned, `Copy` `Animation` handle has `start()`, `stop()`, `finish()` and `running()`: starting an active animation keeps its clock, stopping cancels without completion, and finishing applies its terminal state exactly once. The step receives an `AnimationFrame` (`now`, `elapsed` since start, `delta` since the previous step) and returns whether another frame is needed; returning false finishes it. A callback may start another animation; disposing the scope cancels without completing. Publish animated values through signals. Components decide interaction and completion actions and leave frame scheduling to these.

An animation asks for the very next frame, and a driven program settles only once no frame asks for another (guides/running/drive.md), so motion that never ends (an indeterminate progress bar) as an animation would keep `drive settle` waiting forever. Tick such motion from a timer that asks for its next step a frame's time later (`Some(Duration::from_millis(16))`), as a caret's blink waits out its interval: a frame asked for later does not keep a program from settling.

`tween(elapsed_seconds, duration_seconds, Easing)` gives duration-based progress, and `Spring::advance` a displacement and velocity relative to a target. Tweens catch up to the frame clock after stalls; springs solve damped motion analytically rather than clamping away elapsed time. Retarget a spring by changing its target while keeping its current value and velocity. `Fling` supplies the shared inertia curve.

`Motion::Instant` (one of the preferences, below; the default is `Animated`) finishes active animations and completes new ones without scheduling frames. Each document exposes `motion()` and the reactive `watch_motion()`. Instant mode disables inertia and rubber-banding, keeps carets visible and indeterminate progress bars still, and leaves held gestures following input. Functional timers (expiry, key repeat, media, clocks) still run, so a visibility delay kept separate from the fade after it still waits while the fade is skipped. Switching back does not replay completed transitions; repeating indicators resume explicitly. Appearance and motion are independent preferences, even for the e-paper theme.

### Preferences

`Preferences` (a `ThemeKind`, a `Motion` and a `CanvasTouch`) is what every document follows. Their types live in `crates/be-lib-ui-preferences`, shared by the app protocol and settings blocks, so an app's preferences are the host's without conversion. A `Context` holds three `PreferenceLayer`s, each an `Option` per preference, and resolves them top down, falling back to `Preferences::default()`:

- `PreferenceSource::Developer`: the inspector's Simulation tab. Not saved; its No override choices clear it.
- `PreferenceSource::Program`: what the program chose, from its own settings.
- `PreferenceSource::System`: what the platform prefers, reported with `WindowReports::system_preferences`: winit from the window's theme and, on Linux, the desktop portal's `color-scheme` and `reduced-motion`; the web platform from `prefers-color-scheme` and `prefers-reduced-motion`; Android from night mode and a zero animator scale; the DRM platform says e-paper and instant motion on an e-paper panel; the app platform reports the host's resolved preferences, which reach an app in its acceptance and through the host interface's preferences-watch; app tests send them with `BeuiTest::set_preferences`.

`Context::set_preference_layer(source, layer)` sets a layer and `preference_layers()` reads them. Each document mirrors them at the start of a frame (`preferences()`, and `watch_theme()`, `watch_motion()` and `watch_canvas_touch()` for one each); `Document::watch_preference_layers()` is the reactive form, for settings that show where a value comes from. Components read them as `use_preferences()` and `use_preference_layers()`, and write a layer through `use_preference_layer(source)`. A program that keeps its own choices passes `RunOptions::saving_preferences(name)`, which loads and saves the program layer in the platform's config folder (beui-demo does); one that does not still follows the system. be-host stores them in its display settings instead (guides/the_new_block_stack.md).

### Overlays, and things that float

An overlay is laid out and painted above the rest of the document. Every open
overlay is a layer, and the layers form a tree: a layer's parent is the layer
holding the node it hangs from (its anchor node if it has one, else the overlay
node itself), and the document root is the bottom layer.
`Document::layers()` is that tree as one list, bottom to top; paint, hit
testing, focus, Escape, back and key dispatch all walk it. It is computed once
per change to the tree or the layers, so asking again is free.

Stacking order:

- Children sit above their parent and rise with it.
- Among siblings, those that block everything sit above those that don't;
  otherwise they are ordered by when they were opened or raised
  (`Document::raise_overlay`), so raising a window never lifts it over a dialog.
- A `level` (`LayerLevel`) puts a layer and everything above it over every
  layer of a lower level: `Raised` for things over their windows but under
  menus and dialogs (a dock's floating and popped-out tool bars; a layer that
  blocks everything at `Normal` counts as `Raised`, `LayerLevel::stacked`, so a
  menu or modal still opens over them), `Top` for things over every window and
  dialog (a drag preview, a volume OSD, `styled::Toasts`, so a toast stays
  pressable while a dialog is open), and `Lock` for the lock screen.
- Opening a layer raises it and every layer it hangs in, so a menu opened in a
  covered window brings its window up. An inert layer rises alone, so a tooltip
  over a covered window leaves the window where it was.

### Overlay anchors and placement

An overlay is anchored (`OverlayAnchor`) to a node, a point, a rect in document
coordinates, or:

- `Pinned { node, rect }`: a rect in document coordinates, held by its offset
  from the node's nearer edge on each axis from the first placement on, so it
  follows the node as it moves or resizes; the layer hangs from the node.
- `Band { node, band }`: the node pushed out to `band`'s edges on the axis the
  placement opens along (`layer::banded`), so a button's panel in a bar opens
  just past the whole bar while lining up with the button. Both `Popover`s take
  it as `band`; `DockBarControl::surface()` is the node a `DockBarSurface`
  draws on.

`bounds` (a rect in the overlay's window coordinates; taken by `Overlay`,
`Anchored`, both `Popover`s, `MenuPopup`, `MenuButton` and `ContextMenu`, and
passed on by a menu to its submenus) narrows the room the overlay is placed,
shifted, flipped and shrunk in. A panel or menu that must not cover a dock's
bands takes the dock's area. The HTML core ignores it.

A menu always fits its room: `BelowStart` opens below, else above, else on the
roomier side shrunk to it; a submenu (`RightStart`) opens right, else left, and
slides vertically to stay inside; a panel taller than that scrolls, and the
arrow keys reveal the row they focus. The HTML core does the same with CSS
anchor fallbacks (`flip-block`, `flip-inline`) and a `max-height` of the
roomier side (`layer::room_beside`), set each frame.

`Placement::Popover { gap, margin }` centres content on the anchor, `gap` below
it, flipping above when only that side has room and shifting sideways to stay
`margin` inside the screen (the vertical choice and the shrinking are
`BelowStart`'s). Given a `pointer` frame in its content, the overlay positions
that frame, rather than the content's padded edge, `gap` off the anchor; given
a `beak` reach, it draws a beak on the frame's top or bottom edge (whichever
side it opened on) pointing at the anchor's middle, filled and outlined like
the frame and kept clear of its rounded corners (no beak while the anchor's
middle is past them). The beak lies outside the frame's box and takes no room,
so give it room with a padded frame around it: an app's host draws what is
outside an overlay's content under the editors it holds. The styled dock gives
its cards `POINTER_REACH`.

### Overlay content and lifetime

An `Overlay`'s children (or `content={move || ...}`) are built when it opens
and removed when it closes, like a `Show`: a closed tooltip, select, menu or
prompt costs only its overlay node, and nothing in it is focusable, hit or in
the accessibility tree. `keep_alive=true` makes it a `ShowKeepAlive`: built on
first open, then kept with its state while closed (a dialog, a popover, a
dock's window). Since children can be built more than once they are an `Fn`; a
component holding a `Render` it can call only once passes `render.once()` to a
`keep_alive` overlay, which never calls it twice. State the rest of the
component reads (which rows a select's search hides) belongs outside the
content, derived from data rather than from nodes that may not exist yet.

An animated close belongs to the overlay, not its owner: the content calls
`use_overlay_presence()` and hands `linger_while` a memo that stays true until
its exit finishes, and the overlay stays open with its content until then. The
owner just writes `open` false. `ModalSheet` slides away this way.

### Layer policies

Each layer has one `policy` (`LayerPolicy`):

- `BlocksAll` (the default): a menu, a select popup, a dialog. Its content
  takes the pointer; a press anywhere else is shut out (it asks the layer to
  close and goes no further), and nothing beneath is hovered. One that dims
  what lies beneath (a scrim, as `Modal` and `ModalSheet` draw) or locks the
  session (`LayerLevel::Lock`) is modal to assistive technology too
  (`Layer::modal`, `Document::inert`): what it covers is a `hidden` node with
  nothing under it, actions aimed into it are refused, and an `Embed` beneath
  hears it is inert (`EmbedSlot::on_inert`), so a host leaves that app out of
  its description, as be-host does. A menu, with no scrim, is not modal to
  assistive technology.
- `Pass`: a window, a toast, a popover. Its content takes the pointer where it
  is, and everything around it stays live.
- `BlocksParent`: modal to the node it hangs from alone. A press on that node
  is shut out and asks the layer to close; the focus can't enter the node (Tab
  skips it, and focusing inside it focuses the layer's first focusable
  instead); assistive technology skips it (`hidden` in accesskit, `inert` on
  the HTML core), and the layer is not `aria-modal`. Everything else stays
  live. A modal `DockDialog` is one.
- `Inert`: a tooltip, a drag preview. It takes no input at all.

`traps_focus` (on by default) keeps Tab inside the layer and makes it a key
boundary (guides/beui_keyboard.md): menus, dialogs and popovers trap; windows
don't. A layer that blocks everything but doesn't trap, such as the touch
selection's Copy and Paste menu, leaves the focus in the text field beneath.

### Dismissing overlays

Closing is requested, not done: `on_dismiss` is called with a `Dismiss` reason
(`Escape`, `Back`, `OutsidePress`), and the owner closes the overlay by writing
`open` false, or doesn't (a modal `DockDialog` pressed on its parent flashes
instead). An overlay with no `on_dismiss` can't be dismissed. Each request goes
to one layer:

- Escape that nothing on the focus path took goes to the nearest dismissable
  layer holding the focus, or with nothing focused, the topmost trapping layer.
- Back (Android's back gesture, the Back key or mouse button) goes to the
  topmost layer that has an enabled `on_back` handler or is dismissable,
  stopping at the first layer that blocks everything; the root's handlers come
  last. `WindowRequests::handles_back` says whether anything would take it.
- A press outside goes to every dismissable layer it is outside of, top down,
  until it lands in one or is shut out by one that blocks everything (and then
  to the chain of blocking layers that one is nested in, so a press outside a
  submenu closes the whole menu). A blocking layer still lets the pointer reach
  the blocking layers it is nested in, so a menu's rows stay hovered and
  pressable under its open submenu, and pressing one closes the submenu. A
  mouse asks on the press, a finger on the tap, so a scroll that starts outside
  a layer leaves it open.
- A press on the `trigger` of a layer above is left to that trigger, so a
  `MenuButton` or `Select` closes its popup on a second click rather than
  reopening it. Every component whose button opens an overlay names the button
  as the overlay's `trigger` and toggles the overlay on click. A host of
  escaped layers does the same (`be_host_runtime::dismissed_by_press`), except
  that a layer with `blocks_owner` whose trigger is in the blocked view is
  closed by the press, which the view never hears.

Which layers the pointer can reach is one rule (`Layers::reachable`), shared by
the hit test and input forwarding to a forwarding catcher.

Within the layer, a request goes first to the most recently made enabled
`on_back` handler in that layer whose component is in the tree (not in hidden
`ShowKeepAlive` content or a closed overlay), and to `on_dismiss` only when
there is none, so Escape, back and a press outside all play the same
slide-away. An overlay closed because its parent closed is told afterwards, as
`Dismiss::WithParent`. The owner writing `open` false
(`Document::close_overlay`) runs nothing, so a prompt whose dismiss means
"cancel" needs no guard against its own Keep button.

### Focus and layers

Every layer gives the focus back when it closes, however it closes (owner,
parent closing, or its node removed while open). It remembers the node focused
when it opened, or the last one focused outside it since, and refocuses that
node if it still exists and the focus is inside the layer, or is nowhere and
the layer traps focus. A close that deliberately moved the focus outside the
layer keeps it there, as does a focus change after the close. Components rely
on this instead of refocusing their triggers, so a select, the command palette
and a dialog return focus to what opened them, and a forwarding catcher (an
app's region, a Wayland window) gets the keyboard back.

What a layer remembers is a focus scope; the document keeps one stack of them
(`focus_scope.rs`). An open layer pushes one, closing pops it. Anything else
that takes the focus for a while pushes its own with
`Document::push_focus_scope(holder)` and pops it with
`pop_focus_scope(holder)`, which refocuses only if the focus is still inside
`holder`; be-host's region does this when an app asks for the keyboard. A
non-trapping layer (the toasts) also gives focus back when its focused part is
removed while it stays open, so answering a toast from the keyboard returns
focus to where it was before Tab entered the stack.

Focus requested beneath the topmost trapping layer never lands, so nothing
beneath hears it gained focus. It is remembered until the next input event: if
the layer closes first, the focus goes there instead of to what the layer
remembered, so a dialog's button can close the dialog and focus something
outside it in either order.

The focus is checked when something that could take it away changes (a layer
opening or closing, a frame hiding, a node removed or moved) by walking up from
the focused node, not by searching the tree; a node whose parent link is gone
is outside the tree.

### Back and `BackSlide`

`on_back(enabled, handler)` is a hook like `on_key`: called in a component, it
hangs a handler on that component's node (on the root outside one) for as long
as the calling scope lives. It only routes: the handler hears every phase of
the back gesture, and a back with no gesture (a key, Escape, a press outside)
as `BackGesture::Invoked` alone. It moves nothing.

`unstyled::BackSlide` supplies the motion. It wraps a page: held, the page
follows the finger; let go, it carries on off the edge before `on_back` runs; a
cancelled gesture eases it back; a back with no gesture carries it off from
where it rests. Given `behind`, a function building the page back goes to, it
builds that page under the moving one for the gesture's length, shaded and
coming in from the side under `shade` (transparent unless given), so `on_back`
swaps in what was already showing. The moving page needs an opaque background;
a page that must keep its state (a scroll position) is built once and handed to
a `Portal` in both places. The stacked dock shows the tab back goes to this
way. Given `open`, a back only asks: `on_back` runs at once, a held page eases
back, and the page slides away whenever `open` falls, calling `on_gone` once
it's off.

On Android the runner passes `handles_back` to the activity's
`setBackHandled`; when nothing takes back, the system's own back plays.

### Modals

`unstyled::Modal` is every modal's outer part: an overlay with a scrim that
fades with the modal's presence, and dismissal (Escape, a press on the scrim,
back) through `on_dismiss`. It works like a `BackSlide` given `open`: those
reach `on_dismiss` at once, and the modal slides away and fades its scrim only
when its owner closes it, so an owner that asks first ("discard changes?") or
an app that answers later keeps it in place. While leaving it lingers inert.

- `placement`: `ModalPlacement::Center` or `Fill` (the whole window).
- `motion`: `ModalMotion::Slides` (content in a `BackSlide`), `Still`, or
  `Reported` (the content reports its own presence through the `ModalHandle`
  it is given, as `ModalSheet`'s sheet does; a `Sheet` sinks with the gesture
  and slides on down from there).

`styled::Modal` picks the scrim by `Scrim` (`Soft`, `Dim`, `Strong`, `Opaque`);
`Fullscreen` fills the window over `Scrim::Strong`. `styled::Dialog` and
`Fullscreen` sit on `unstyled::Modal`.

### Dialogs

A dialog's buttons are data, not buttons in its body
([decision](../decisions/beui_dialog_actions_are_data_with_roles.md)): `actions` is a list of
`DialogAction`s, each a label, an `ActionRole` (`Primary`, `Destructive`,
`Secondary`, `Cancel`), and `disabled`, `busy` and `shown`:

```rust
DialogAction::primary(label, on_click).disabled(blank)
DialogAction::cancel(label)
```

The dialog places, orders, draws, focuses and binds them (`unstyled::DialogBody`,
painted by `styled::DialogBody`).

- Cancel runs the dialog's `on_dismiss`, as do Escape, back, a press on the
  scrim and the close button.
- `DialogKind::Alert` is a decision (leave, discard, delete): its actions are
  its only way out, its role is AlertDialog, and focus starts on the primary
  action, or on Cancel when that action is destructive.
- `DialogKind::Dialog` (the default) is content (settings, members, inspect): a
  title row with a close button, and a footer only when it has actions.
- Enter that nothing in the dialog takes runs the `Primary` action unless it is
  disabled or busy, so a field needs no `on_submit`; a `Destructive` action
  never runs on Enter.
- A wide footer is right-aligned: `Secondary` actions (and a `Destructive` one
  beside a `Primary` one) on the left, then Cancel, then the primary (the macOS
  and GNOME order); it wraps rather than stacking. A narrow or touched footer
  stacks its buttons full width, primary first.
- `busy` disables an action and shows progress in its button (`Button`'s
  `busy`).
- With `id`, the card is `{id}`, the buttons `{id}.confirm` (primary or
  destructive), `{id}.cancel` and what `named` gives a secondary one, the close
  button `{id}.close` and the footer `{id}.actions`.
- A confirmation inside a dialog is an alert of its own, opened over it; a
  control or row action (a member's role, share's Add) stays in the content.

`styled::Dialog` is `DialogFrame` (the outer part, on `unstyled::Dialog`) plus
`DialogBody` (the inner part). `DialogFrame` picks its presentation in one rule,
`dialog_presentation`, from its kind, `size` (`DialogSize::Small`, the default,
or `Large`) and `anchor` (`DialogAnchor::None`, the default,
`Control(node_ref)` or `Rect(Option<Rect>)`):

- An alert is always a centred `DialogCard` in a `Modal` over a scrim.
- A `Dialog` whose window is narrower than `SHEET_BELOW`, or that a finger
  opened (`touched_by_finger()`, as `Anchored` decides), is a `ModalSheet`,
  fitted when small and full height when large.
- Otherwise: with no anchor, a centred card; large with an anchor, centred on
  its frame (`CenterOn`) over a scrim; small with an anchor, a card anchored to
  a rect (`Popover`, no scrim).

It draws its beak only when that rect is something the user can see: the
control that opened it, or the rect it was handed (a tab, a card). A control is
resolved when the dialog opens with `opener_of`: a control inside a menu that
has closed, or inside a menu with a trigger, stands for what that menu hangs
from (its trigger, or the node a `ContextMenu` was opened on), menu after menu.
A control that can't be found stands for the frame's anchor. With nothing
visible to point at (`Rect(None)`, or a lost control in a frame with no
anchor), the card opens under a zero-height rect at the top centre of its frame,
with no beak. Anchored presentations escape a window that has a layer space;
centred ones stay.

`provide_frame_anchor(memo)` / `use_frame_anchor() -> Memo<Option<Rect>>`
(unstyled) is where a window's frame shows to the user, in the window's points:
be-sdk-beui provides the editor's tab (its handle in the dock's strip, or the
rect a parent placed it at when it has no tab, or the anchor of the last menu
pick), and `None` when nothing of it is visible, such as a fullscreen tab.

Which anchor to pass:

- A small form about one object, opened from a control: `Control` of that
  control (database's Filter and display button).
- From a menu row: fill a `NodeRef` with `focused_node()` in the row's
  `on_click`, since a `MenuItem` is no node (file-tree's Inspect).
- From a point on a canvas: `Control` of an empty `Frame` in a `CanvasItem`
  over that point (logic-grid's storage, a calendar slot).
- From an action (run by the frame menu on the tab, the palette or a
  shortcut): `Rect` of `use_frame_anchor()` (text settings, canvas Export), so
  it points at its tab.
- A large form or embedded editor: `size=DialogSize::Large`, with `Rect` of
  `use_frame_anchor()` when it belongs to a frame (be-sdk-shell's Share and
  artifact settings); a host-drawn one with no frame (be-host's Members,
  Contacts, Agents, Unsent edits, the conflict compare) passes no anchor and
  stays centred.
- Alerts and program-level modals pass neither.

`DialogFrame` builds its content once, on first open, and moves it between card
and sheet with a `Portal` if that changes while open; it provides a
`DialogLayout` (`narrow`, `sheet`, `bottom_inset`) to what it holds. A sheet is
fitted to the screen beui's on-screen keyboard leaves, so its footer stays
above it.

`DialogBody` is the title row, the content in a fitted `Scroll` with
`Bleed::dialog` that gives up length when the screen is short (or, with
`scrolls=false`, the content given the rest of the height to scroll itself,
reaching the card's sides with a `Scroll` or `Tree` whose `bleed` is
`Bleed::DIALOG`), and the pinned footer. It reads `narrow` and `bottom_inset`
(how much of its bottom is covered; its padding keeps the footer above it) from
its `DialogLayout`. Code that places its own body uses the parts: be-host's
keep-changes prompt is a `DialogCard` with a `DialogBody` in a centred `Modal`,
and an app opened as a dialog draws a `DialogBody` that be-sdk-beui hands a
`DialogLayout` from its frame's `dialog`.

`DialogBody` handles Enter, its close button and Cancel; Escape, back and a
press on the scrim reach whoever owns the outer part. An outer part in another
document provides a `DialogDismissal`, which the `DialogBody` inside claims
while it lives: `handled()` says whether one did and `dismiss()` runs its
`on_dismiss`, so a form with unsaved changes can ask before closing.
be-sdk-beui provides one in an app dialog and forwards the host's dismissal,
Escape and back to it.

### The lock screen

A layer at `LayerLevel::Lock` that blocks everything is a lock screen
(be-host's `LockScreen`). It is above every other layer, so anything opened
while it is up opens beneath it; it is never dismissable; the focus can't leave
it, not even to nothing; `Document::locked` is true; the listeners above the
tree (`on_global_key`, window and host actions) hear only media keys
(`Key::is_media`); and a forwarding catcher beneath it loses any captured
press, as one beneath any layer that blocks everything does.

### `Floating`

`unstyled::Floating` pins its child to the top or bottom edge of a node named
by `NodeRef`, taking no space in the layout around it:

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

### Claiming presses

A control that has to win a press from a surface around it (an add button in a
row that is itself a button, a close cross on a tab) sets `capture_presses` on
`unstyled::Button`. The press stops at that button, so the row doesn't open
too.

An `Interactive` given `claims` (a list of `Claim`s, each modifiers and an
`Area`) takes every primary or secondary press in a claim's area while all its
modifiers are held; a claim with no modifiers takes every press there. This is
how a window manager takes Super+drag from the windows it holds. Claims are
searched from the top of the tree down, before `capture_presses`, so the
outermost claimant wins; the press, its drag and its release reach the
claimant alone, none of its descendants, and no forwarding catcher under it
unless the claimant is that catcher. A Wayland program's window is a
forwarding catcher like an app's region, so a press claimed over it never
reaches the program. `Document::press_claims` lists the rectangles of the
claims that need modifiers, laid out this frame, which an app reports to its
host.

`on_press_outside` hears every primary press outside the `Interactive`'s
visible rectangle, wherever it goes and whatever takes it, at the same moment
and from the same place a press outside asks layers to close (a mouse's press,
a finger's tap; `Document::pressed_outside` does both; a long press counts as a
secondary press, which asks the layers but not these). The host uses it to hand
back an embedded editor's active child when someone clicks elsewhere.

A secondary drag stays with the node that took it while the pointer crosses a
floating window above that node; only the start of one is hidden from what a
floating window covers.

A canvas a pen draws on is an `Interactive` with `drawing` (an `Area`: `true`
for all of it, or a `HitRegion`). While the `canvas_touch` preference is
`CanvasTouch::Off`, a finger that lands on one reaches nothing (no press, drag,
long press, scroll or pinch, for that node or any around it) until it lifts. A
pen, a mouse, a touchpad, and a finger on anything the drawing surface doesn't
hold (its floating chrome among them) are unaffected. It is a user-set switch,
not palm rejection ([decision](../decisions/touch_off_toggle_instead_of_palm_rejection.md)).
`Document::drawing_rects` lists the `drawing` rectangles, which an app reports
to its host (`FrameReport::drawing`) so the host keeps the finger from the app
and from its own pan and zoom.

### Trees

`unstyled::Tree` makes a row a tab stop with the tree's keyboard and its
`TreeItem` accessibility, and nothing more. Where a pointer must land to select
a row is the face's business, since a face that draws its own chevron outside
the name wants the chevron, the indent beside it and the name to mean
different things. The handle carries `select`, `toggle` and `hover` for the
face to call where it decides.

- Arrow keys, Home, End and typing move the keyboard between rows without
  selecting; Enter or Space selects the row it is on. `toggle` puts the
  keyboard on its row too.
- Given `on_open`, Enter and a second click select the row and report
  `on_open`, for a tree whose rows are chosen as well as selected (the block to
  link).
- A press that travels further than a few pixels reports `on_drag_start` and
  drops the click it would have become, so a row can be dragged out without
  selecting what it left.
- Row menus: given `menu`, a `TreeMenus::new(style, menu)` pairing a
  `MenuStyle` with the function, it wraps each row that has one in a
  `ContextMenu` and hands the face a `TreeRowMenu` in the handle's `menu`,
  which `unstyled::TreeRowMenuButton` turns into a menu button anchored to
  itself, so the dropdown hangs from the "…" button.

`styled::Tree` is the face program code uses. Everything left of a row's
highlight (indent and chevron) is one button, tooltipped and labelled for a
screen reader, so a finger anywhere left of the name expands the row without
opening it, and pressing the highlighted row selects. The content builder gets
a `TreeRowFace` carrying the row's `key`, `item`, `selected`, `focused` and
`hovered`. Given `menu`, which answers a key with a `TreeMenu` (its items, each
acting through its own `on_click`) or `None`, every row with one shows a "…"
button at its end, on desktop too; the same menu opens from it, a right click
or a long press, as a dropdown under a mouse and a sheet under a finger.

- Row height follows `use_density()`: `Density::Compact` on desktop,
  `Density::Comfortable` (tall enough to tap) where `provide_density` asks for
  it, which an app's frame does when its host says the window is a phone's.
- `row_test_id` names rows for tests as `<id>.row`, `<id>.chevron` and
  `<id>.more`.
- `outline` gives one row an outline of its own, which is how a drop target
  says whether it will take what is over it.

A row's name is edited in place through the controlled `editing` prop (the key
being renamed, or `None`). While it names a row, the row's face gives way to a
field of the row's height holding `TreeItem::label`, after what `leading` built:
`leading` builds a row's icon (any run of children, from the same
`TreeRowFace`) ahead of its content and keeps it beside the field while
editing. The field, `<id>.rename` for tests, is focused with all of it selected
and scrolled into view. Enter, or losing the focus (a press elsewhere, the
window losing the keyboard), reports `on_rename` with the trimmed name; Escape,
or an empty or unchanged name, reports `on_cancel_edit`. After Enter or Escape
the keyboard goes back to the row, and the caller clears `editing` in either
case. F2 on a row asks for an edit with `on_edit`, when given. Keys typed in the
field don't reach the tree's navigation. `unstyled::TreeRowEditor` is the field
a face places for a row whose handle reads `editing`, and owns all of that.

Choosing a row never expands, collapses or scrolls anything; only the chevron
and the arrow keys do. The styled tree owns its `Scroll` (`padding` goes inside
it) and follows the `selected` key. Given `ancestors`, which answers with the
keys above one, outermost first, it marks the chevron of the deepest row still
shown above a selection hidden in a collapsed one, and while the selection is
hidden or scrolled out of view it floats a button over the top or bottom edge
that reports `on_reveal` (the caller expands the ancestors), then scrolls the
row into view once it is laid out. The file tree and the beui inspector both
work this way, and both hold the tree's `TreeApi` through its `api` prop to
find its rows (`row_node`) and its focus (`focused`).

### Overlays stay in their window

A document knows nothing of the monitors it is shown on. Every overlay is laid
out within the window it was opened in (the main window's rect, or a hosted
window's own `(0, 0)`-cornered rect, less any on-screen keyboard there):
`Center` and `Fill` take that whole area, and one anchored to a node, a point
or a rect (`OverlayAnchor::Rect`, in that window's coordinates) is placed
against it and kept inside. Where a program shows one window per monitor
(be-host in a login session), a dialog opened in a window stays on its monitor.
`use_viewport_size()` is the size of the window the caller is built in: the
hosted window around it (an app region is one), else the main window (a
`Dialog` reads it to become a sheet on a narrow one).

A `Center` overlay is never larger than its window: taller or wider content is
laid out at the window's size, so its bottom can't fall off a short window.

An overlay that is not a window of its own shows in the window it was opened
in, unless `shown_in` names another of the document's windows: it is then laid
out, painted and counted (`Document::window_of`) in that window. An anchored
overlay of a window with a layer space escapes it instead (Layers that escape
their window, below); that is how an app's menus, tooltips and popovers reach
past its frame.

be-host's keep-changes prompt and lock screen are one card in a `Modal` that
fills (or centres in) its window, one in each display's window. The desktop
app's `LevelOsd` is one card in an inert `Fill` overlay, shown in each
display's region.

### Toasts

`unstyled::Toasts` is the toast stack: an overlay at an `edge` that lets input
through, each toast's timer on the frame clock, and a live region (a `Log`,
each toast a `Status`, or an assertive `Alert` when it is `danger`). A toast
goes after its `duration` unless it is `sticky` or `danger`; an error stays
until dismissed. It shows only the newest `limit` toasts (`TOAST_LIMIT`, 3) and
draws `more` with the count of the rest; a hidden toast's timer starts once it
shows. The stack scrolls inside its anchor when still too tall.
`styled::Toasts` takes any anchor, so the desktop app stacks its toasts in a
display's corner above its bar; it passes width, margin and spacing, draws the
count, and draws each card from a `ToastHandle` (the `Toast`, `dismiss`,
`activate` and `act`), cutting the title to two lines and the message to four
(the desktop's notification list shows them whole).

### Layers that escape their window

A window given a layer space
(`Context::set_layer_space(window, Some(LayerSpace { to_host, bounds }))`,
`beui_types::layer`) lets its anchored overlays out of its rectangle. `to_host`
maps the window's points to the host's (`host = origin + point * scale`);
`bounds` is where layers may go, in host coordinates. Without one nothing
changes: native be-host, tests and standalone programs never set it. In the app
protocol an escaped layer is an anchored view and a layer space is its view's
anchor space (`screen-request.anchor-space`; be-sdk-beui-platform's
`escaped.rs` maps one onto the other, guides/adding_an_app.md).

`Placement::escapes()` decides what escapes: anchored placements (`BelowStart`,
`RightStart`, `Above`, `Over`, `Around`, `CenterOn`, `Popover`) and
`Edge(LayerEdge)`, a sheet attached to an edge of the bounds
(`ModalPlacement::Sheet`, used by `ModalSheet`), do; `Center`, `Fill`, `At`
(windows, dock windows) and the `Inside*` placements (toasts, `Floating`) stay.
So menus and submenus, `ContextMenu`, popovers, tooltips, the selection toolbar
and anchored dialogs escape; centred modals, toasts and banners stay. An
overlay that is a window of its own never escapes.

Layout: an escaping overlay is laid out at full size in host units whatever
`to_host.scale` is. Its content sits in a zoomed space of its overlay (slot 1,
scale `1 / to_host.scale`, translated by `-origin / scale`) whose coordinates
are host coordinates, placed against `bounds` with its anchor mapped through
`to_host`, so it flips, slides and shrinks against the bounds, and a menu from
a region shown at half size is as large as any other. An `Edge` sheet fills the
bounds, so a `ModalSheet` works out its stops from them. A blocking scrim still
covers only its window, in window coordinates.

Coordinates and input: its rects as `Rects::get`, hit testing and
accessibility see them are still window coordinates, which `to_host` maps to
where it is drawn (`Document::escaped_layer_of(node)` returns the layer and
that `to_host`). Hit testing, focus, trapping, Escape and back are unchanged.
Input that landed on a layer is pushed as `Incoming::layer(id, input)` with
host positions; the runner maps it into the owning window and hit-tests it
there. `Context::dismiss_layer(id)` asks the layer to close as a press outside
does (`Dismiss::OutsidePress`), with its nested layers. It is heard after the
frame's input, and not at all if that input already asked the layer to close or
closed it, so a press on a popover's trigger toggles it closed once.

Accessibility: the tree places every space node, this one included, from the
space's own transform rather than its host node's, so an escaped layer opened
from deep in the window is described where its rects are. An escaped layer's
nodes are left out of its window's tree (`FrameOutput::window_accessibility_tree`,
which drops escaped roots from its nodes' children) and described on their own
by `FrameOutput::escaped_accessibility_tree(layer, size, origin, to_host)`,
rooted at the layer's overlay node and mapped into points from `origin` (the
layer's top-left in host coordinates), for a platform that shows each layer as
its own view. `Fragment::escaped` and `escaped_roots` carry which nodes belong
to which layer. An overlay that starts escaping after being described in its
window (its window's layer space arrived late), or falls back into its window
while open, makes the next accessibility update a full one, so neither tree
keeps a stale copy.

Painting and output: an escaped layer is left out of its window's painting and
of the window's `overlay_rects`, `overlay_painted_rects`, `floating_rects` and
`overlay_scrims`, and painted into a paint window of its own.
`Document::escaped_layers()` lists them in layer order, each after its parent,
as `EscapedLayer`s:

- `id`, stable; `parent`, the escaped layer it is nested in (for submenus).
- `placement`: `Anchored(rect)`, in window points for a layer of the window and
  in host coordinates for one nested in a layer; or `Edge(edge)`.
- `rect`, `painted`, `floating` (the content of every layer that takes input)
  and `scrims`, in host coordinates.
- `dismiss_on_outside_press` (dismissable, and blocking everything or passing),
  `keyboard` (traps focus), and `blocks_owner` (blocks everything). A layer that
  blocks only its parent leaves the owner's view live; the document itself
  blocks it there and never asks it to close on a press outside.

Each frame's `FrameOutput::escaped` carries an `EscapedOutput` per layer: that
record and a `WindowOutput` (transparent clear colour) painted in host units
from the layer's top-left at `pixels_per_point = window raster / to_host.scale`
(the program window's density, since a zoomed window's raster counts its
zoom), ready for a surface of its own. `composite_layers` draws a window and
its layers as the host would; `Stepped::snapshot_layers` paints with it.

### Docking and windows

`dock::Docking` (crate `beui-dock`) is the workspace layout: panes split from one another, a tab
bar on each pane, and tabs that can be dragged between panes or out into windows floating over the
dock. A `Docking` only declares its dock; a `dock_host::styled::DockHost` above it lays the dock out
and draws it where the `Docking` is, and owns the tree, the dragging and the keyboard
(`dock_host::unstyled::DockHost` underneath paints nothing). An editor's frame already has one
(be-sdk-beui), so an editor writes only the `Docking`; a program, a test or a shell that needs the
host's own settings or its `DockHostLayout` puts one around its `Docking`. A `DockHost` hosts one
`Docking`; attributes that place it (`@sizing`, `@test_id`, `@node_ref`) go on the `DockHost`, whose
node is the dock's. The demo in `crates/beui-demo` is laid out in one; its Docking and Dock bars
pages show most of what follows.

```rust
let layout = DockingLayout::new();
let host = DockHostLayout::new();
view! {
    <DockHost @sizing=ItemSize::Percent(100.0) layout=host drag_modifier={Some(Modifiers::LOGO)}>
        <Docking layout>…</Docking>
    </DockHost>
}
```

`DockHostLayout` is the host's side, keyed by `TabId` (`DockingLayout::tab_of` turns a key into
one): the area the dock lays windows out in (`area`), what covers it (`set_covered`,
`over_bars_rect`), its windows (`window_rect`, `move_window`, the window manager below) and the
switch. `DockHost` takes the settings of how the dock behaves (`drag_modifier`, below) and an `api`
(`ComponentRef<DockApi>`, `DockApi::state`) for tests.

#### Declaring a dock

The children describe the starting layout and the full set of tabs; a `DockingLayout<K>` holds
where everything is now, and is what the caller saves and restores. `K` is a `DockKey`: a tab's key,
whose `key()` names it as a stable string (implemented for `TabId`, `u64`, `String` and
`&'static str`; implement it for your own type). A dock declared to a host across apps sends
`key()` as the tab's key.

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

- `DockSplit` takes two children; `fraction` is the first one's share. A split follows the code's
  `fraction` until the user moves it.
- `DockPane` holds `DockTab`s and `DockGroup`s (a tab holding a tree of its own; `pinned` keeps its
  tabs in it), names its starting `active` tab, can start with its tabs in a sidebar (`vertical`,
  `sidebar_width`) or icons only (`icons_only`), can be `locked` (see "Locked areas") and can be
  `bare` (no tab bar: it holds one tab and takes no tab dropped onto it, only a split at its edge;
  a joined dock's single pane uses it so its group shows no second tab strip).
- `DockWindow` floats one tree at a `rect`.
- A tab's optional `icon` is an icon-font glyph (empty for none), drawn by the tab bars, the drag
  preview and the stacked bar.
- A `see_through` tab shows something drawn under the dock (the inspector's Program tab shows the
  program): neither its pane nor a stacked dock fills behind it, its pane's chrome learns it from
  `DockChromeHandle::see_through`, and it stays on the main surface (`DockState::admits` refuses it
  a window). `provide_dock_shows_through(memo)` makes every dock built below show through the same
  way while it is true, except in floating windows; be-sdk-beui's frame chrome
  (`Editor::placed_content`) provides it for an editor keeping to its card on its parent's stage, so
  the parent's stage shows around the card.

Every container has an `id`, saved with the layout, so that a layout saved by older code still finds
its panes. Test ids:

- A tab whose key is a `TabId` or `u64` names its `TabId` too, which its close button and switcher
  row carry (`dock.tab.<tab>.close`; a group's close button carries its hosting tab's).
- A pane's bar is named after the pane's `id`: `dock.grip.<id>`, and under `dock.<id>` its More
  button (`dock.<id>.menu`, items `dock.<id>.menu.<item>`), pinned actions
  (`dock.<id>.pin.<action>`) and close button (`dock.<id>.close`), so two panes showing the same
  actions stay apart. A pane the user made has no `id` and the bare prefix `dock`, as has the one
  bar of a stacked dock (`dock.menu`, `dock.pin.<action>`).

A pane or window holding one tab shows only that tab's close button, and the tab shrinks to fit its
bar, ellipsising its title; several tabs scroll. A window is at least `MIN_WINDOW_SIZE.x` wide,
whatever its tab asks for (`DockTabControl::resize_window`), so a tiny program's window is framed
wide enough for its bar.

#### Reconciling, saving and restoring

The declared children are the full set of tabs: a tab the code stops declaring is closed, and a tab
it starts declaring is placed and shown. Closing is a request: `on_close` gives the tab its close
button, and the tab goes when the caller stops declaring it, or stays (the workspace keeps a
program's window until the program quits). A new tab joins the pane it was declared in, after the
tab declared before it; if the user closed that pane, it joins the nearest declared tab still open,
then a window the code declared it in, and failing both floats in a window of its own. A pane with
an `empty` view is never closed: emptied, it shows that view, and dragging its grip onto another
pane moves its tabs and leaves it behind. A pane, split or window the code adds is placed beside its
declared sibling when the saved layout lacks it.

A tab that stops being declared for a moment (a `Show` around it that flips, a `ForEach` rebuilt
from new keys) is closed and loses its place, so keep a tab's declaration in one stable place.

`DockingLayout` is a `Copy` handle owned by the scope that made it; `OwnedDockingLayout::new()` makes
one outside a scope (a host, or a test before it builds) and derefs to it. It offers:

- `snapshot` (the `DockingSnapshot` to save), `restore`, and `reset` (back to the layout the children
  describe). A snapshot has no serialized form of its own: to save one keyed by `TabId`, save
  be-sdk-beui's `SavedDockLayout::of(snapshot)`, the app protocol's `DockLayout` (the form app docks
  are saved in, with the order tabs were shown and the visit history); `SavedDockLayout::snapshot`
  turns it back.
- Restore before the effects of the write that declares its tabs run (writes made together, with no
  `flush()` between, qualify): a tab not declared when the dock reconciles is closed.
- `show`, `focused`, `shown` (for a stacked dock), `mode()`, and `state()`, the plain `DockState`
  value underneath, from which `find`, `all_tabs`, `focused_tab` and `surface_tabs` answer.

Saved with the layout: the tree, `recent_tabs` order, history, homes and unlocked places, bars and
their placements, and compact sizes. Not saved: a switch in progress, fullscreen, window snaps and
dock dialogs.

#### Documents in tabs

A tab's key names the tab, not what it shows, so a tab that shows something else keeps its key and
with it its place, focus and history. Master/detail views, and lists that open what they name into
tabs (the demo's catalog, be-launcher's pull requests), hold their tabs in a
`DockDocuments<K, T, I>`: slots of content `T`, made with `new(layout, key, identity)`, where `key`
turns a `DockSlot` into the layout's key and `identity` tells one document from another.

- `open(content, beside)` shows the document if it is open; otherwise it puts it in the slot shown
  last, unless `beside` (Ctrl or Cmd held) asks for a slot of its own. It answers
  `DockOpened::Shown`, `Replaced` (with what the slot held) or `Added`.
- `refresh(content)` updates an open document in place, `shown` and `find` name slots, `close`
  empties one.
- `DockDocumentTabs` declares a tab per slot, handing the caller a `DockDocument`;
  `DockDocumentBody` rebuilds its body only when the slot's document changes, so new content for the
  same document keeps the body's state:

```rust
<DockDocumentTabs documents>|document: DockDocument<Tab, PullRequest, u64>|
    <DockTab id={document.key()} title={title_of(document.content())} on_close={move || document.close()}>
        <DockDocumentBody document>|pull_request: ReadSignal<PullRequest>|
            <Detail pull_request />
        </DockDocumentBody>
    </DockTab>
</DockDocumentTabs>
```

#### Panels and focus

A tab's panel is built the first time the tab is shown and belongs to the dock, not the pane: the
pane holds a `Portal` pointed at it, so the panel keeps its nodes, scroll position, caret and state
when hidden, dragged to another pane, floated into a window, or moved between tiled and stacked
modes. A panel no pane shows is laid out by nobody, costs nothing, and is not read by a screen
reader. Closing the tab removes it.

Pressing a pane's or window's chrome (bar, tab, grip) gives the keyboard to the tab it shows: the
bar is an `Interactive` with `press_focus=false` (guides/beui_keyboard.md), and the dock
(`beui-dock-host/src/unstyled/keyboard.rs`) puts the focus in the panel where it was when that tab last
had it, or on its first focusable (`Document::focus_into`, which waits a layout for a panel still
being built). Picking a tab with the pointer does the same; the arrow keys leave the focus in the
tab bar. When a pane stops showing a tab whose panel has the focus, the dock remembers where it was
and gives the keyboard to the tab shown in its place.

- `DockTabControl::keyboard` changes each time the dock gives a tab the keyboard, so content whose
  keyboard lives in another document (a program's window) can take it there.
- `DockTabControl::brought_forward` changes each time a tab in a floating window comes to the front
  (opened, shown, switched to, or left in front by a close; not while the dock is built or
  restored), without moving the focus.
- A block placed in a tab (`ChildBlock`, `Subregion`) follows both and asks the host for the
  keyboard (`focus_child`, guides/adding_an_app.md), which is how a block window the desktop opens or
  brings forward takes the keys.

Keys: Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of the focused pane, registered with `on_shortcut` so
they arrive even from a text input in a panel. The bar between two panes is a tab stop with a
`Splitter` role that the arrows move. The tab bar is a `Choice` inside a `Scroll` running the bar's
way, so the arrows, Home and End walk it and scroll the tab they reach into view.

#### Tiled, compact and stacked

Tiled, the styled `Docking` lays panes edge to edge, divided by a one-point line
(`splitter_thickness`) that takes presses a few points either side (`splitter_reach`); only floating
windows and cards have a border, rounded corners and a focus outline. A split keeps each side at
least `MIN_PANE_LENGTH` where there is room, and a window is drawn no larger than the dock, so a
layout made on a wide screen stays usable on a phone.

The dock picks its mode from its own size; `DockingLayout::mode()` reads it (that is how the
workspace knows it is on a phone), and a joined dock reads the mode of the dock it joined
(dock-hosted's `mode`). The unstyled `Docking` is compact below `stack_below` wide, or below
`short_below` tall and `short_narrower_than` wide; the styled one sets these to `NARROW_WIDTH`,
`SHORT_HEIGHT` and `PHONE_LANDSCAPE_WIDTH` (a phone either way up, not a short desktop window).

- **Stacked** (`DockMode::Stacked`): a compact dock whose own tree has no bare stage pane draws the
  same state as one screen, the focused tab filling the dock edge to edge, with no tab bars,
  splitters or windows. Everything else in the state is kept, so widening tiles it again.
  `stacked_tab` is the tab it shows: the focused one, or the one shown last when the focused pane is
  empty. `recent_tabs` lists tabs from the one shown last. A stacked dock ignores locks and keeps
  them for when it tiles. The workspace stacks its dock on a phone.
- **Gathered** (`DockMode::Compact`): a tree with a bare stage pane (`DockState::stage_of`: a bare
  pane among others, an editor's stage), whether the dock's own or a joined group a stacked dock
  shows (the workspace's editor). The stage keeps its pane, and every other pane's tabs, in declared
  order, share one tabbed pane (`dock.compact`): below the stage when the tree is taller than wide,
  at 40% of its height (`COMPACT_SHARE`), or beside it when wider, as wide as the tree's fixed split
  (`SplitSize::First/Second`) or `COMPACT_SIDE`. Its splitter (`dock.compact.splitter`) is the only
  thing that resizes the stage. Tapping the gathered tab on show folds the pane to its tab strip;
  tapping a tab or `DockState::show` (so `DockingLayout::show` and `DockRequest::Show`) unfolds it to
  that tab. Dragged sizes, the fold and the gathered tab are a `DockCompact` per stage tab
  (`DockState::compacts`), saved apart from the tiled splits (`DockSpec::compact`, dock-layout's
  `compact`) so switching sizes mixes neither. A stacked dock's switcher lists such a tree by its
  stage alone (`DockState::switchable`).

The stacked screen's bar is the dock's, not the caller's: the tab's icon and title; a square, always
shown, counting the open tabs that can be closed, the shown one included (so it reads 0 while only
tabs like Files are open), which opens a sheet to show or close them (a card per closable tab from
the one shown last, then a row per tab without a close button, such as the workspace's Files and
Settings, in layout order); and a back button while `DockState::back` has somewhere to go, which the
back gesture also follows.

#### Tab menus and pinned actions

What a tab adds to the dock's bars is its menu, which the panel hands over with `dock_menu(items)`,
a `Memo<Vec<MenuNode>>`. A tiled pane or window offers the shown tab's menu behind a More button
just before its close button, at the end of its tab bar or the top of its sidebar; the stacked bar
offers it too.

- `.pin(Pin::Icon)` and `.pin(Pin::Label)` move an action out of More into the bar. A tiled tab bar
  shows the icon pins, a divider, then label pins as labelled buttons (the workspace's undo and redo,
  then Share); a sidebar shows every pin as an icon; the stacked bar shows only icon pins and keeps
  label pins in its menu.
- `.pin(Pin::Title)` belongs to the tab's name: the stacked bar draws the name as a button with a
  chevron that runs it (the workspace's Rename) and leaves it out of its menu; tiled bars keep it in
  theirs.
- A pinned action built with `.checked(..)` is a toggle that looks pressed while on.

Pinning puts a tab's few most-used controls in reach without giving the dock a node; the rest stays
in the menu, and the tab keeps its page for its own toolbar. A menu is a list of actions rather than
a node, so a dock laying out tabs built elsewhere can draw it from rows it was sent, with actions
made by `ActionBuilder::detached` (not registered for shortcuts or the palette), and pass the pick
back.

Splitting a menu between a bar and More is `unstyled::ActionBar`'s, usable by any toolbar: given the
`menu`, whether label pins fit (`labels`) and whether the caller shows the title pin itself (`title`;
`unstyled::title_action` finds it), its handle holds the `icons` and `labels` to pin and the
`overflow` for More. `styled::ActionBar` draws it, with `ActionLabels::Shown`, `Icons` or `Overflow`
for the label pins, a `direction` and a button `size`, naming its parts `<test_prefix>.pin.<id>` and
`<test_prefix>.menu`; the dock's bars are styled action bars. A tiled bar is 40 points with its rule,
its tabs 30; its controls are `ButtonSize::Toolbar` icon-only buttons (30 square), the labelled pins
compact `Button`s of the same height, and a tab's own close button stays compact. The stacked bar
uses regular-sized buttons.

#### History and back

Every change of the focused tab is a `DockVisit` in `DockState::history`: the tab it came from, the
tab it went to, and whether that tab was `opened` by it (first shown since it joined the dock).
`back` is the last visit to the tab on show. Going back undoes it: a tab the visit opened is closed
through its `on_close` when it has one, unless the `DockTab` says `back_closes=false` (be-sdk-shell's
`ProgramTab` does, since closing a program's window ends the program; the app protocol's `dock-tab`
carries it as `back-closes`). Closing the tab on show, in either mode, shows the tab its last visit
came from; otherwise `go_back` shows that tab and drops the visit and every one after it. A tab that
leaves the dock hands its visits to the tab it was reached from.

#### The switcher

The switcher walks `recent_tabs` across every pane and window, as Alt+Tab walks a desktop's windows.
`DockHostLayout::begin_switch(backwards)` chooses the tab shown before the current one (backwards: the
one shown longest ago), `step_switch` moves the choice along and wraps, `commit_switch` shows it
(activating it in its pane and raising its window), and `cancel_switch` leaves everything as it was;
`switching` and `switch_choice` read it back, and `DockState` has the same calls and `switch()`.
While a switch lasts, an overlay in the middle of the document lists the tabs with icon and title,
the chosen one highlighted, drawn by the `switch` render prop (`DockSwitchHandle`, a row per tab
whose `pick` commits it); nothing else changes until the commit.

Escape cancels it with any modifiers held and wherever the focus is (a program's window included):
the overlay holds no focus, so the dock listens for Escape at the window while it is open and asks
the overlay to close (`Document::dismiss_overlay`), as a press outside or back does. A dock in an app
only hears a key a program has the keyboard for when it intercepts the chord, so the desktop app also
binds Alt+Escape to `dock.switch.cancel`, intercepting it. The dock binds no other key:
`dock.switch.next` and `previous` (in `window_actions`) begin or step the switch, and commit it at
once unless Alt, Ctrl or Super is held, in which case letting go of all of them commits it. Whoever
binds them decides the keys, as the desktop app does with Alt+Tab; the demo binds Alt+Q its own way.

#### Surfaces and windows

Each surface (the main one, and one per window) lays its tree out over its rectangle, so panes and
the bars between them are canvas items at computed rectangles rather than nested boxes.
`layout_surface` is that calculation on its own, which drop targets and tests resolve against. A
window is a floating overlay: it paints above the dock, takes the pointer where it actually is, and
shuts out the document underneath.

A dock window is a `Window` (see "Windows and window hosts") sized to its rect plus the reach of its
resize grips, which lie wholly outside it in a ring `HANDLE_REACH` wide, each corner's grip running
`GRIP_CORNER` along both edges. Neither the grips nor the bar between two panes (which takes presses
only in the gap it draws) reach over a pane's or window's content, which gets every press up to its
edge. Windows resize from any of their eight grips and are raised by whatever takes the focus inside
them.

- Where nothing opens windows, the dock moves, clamps, springs back and resizes the `Window`'s
  overlay itself.
- Where a host opens windows (`opens_windows`), each dock window is a real window (space's panels,
  or be-host's windows on the desktop for the desktop app's dock, guides/desktop.md, Displays): a
  press on its bar calls `begin_move()` and a grip `begin_resize(edge)` (as does a secondary drag
  with `drag_modifier`), so the host moves and resizes it. A size set from inside
  (`DockTabControl::resize_window`) changes the rect and so the `Window`'s `size`, which asks the
  host with `WindowRequest::Resize`. Either way the dock writes the host's `Resized` sizes into the
  rect (less the grips), keeps each `Placed` report's `HostPlacement`, and hands it back as
  `WindowPlacement::Host` when the window opens again (after the dock was stacked, say). A window
  with no kept placement (floated from a tab's menu, or dropped on the main window where no pane
  takes it) opens `Over` the dock's window at the corner it would be drawn at there, grips included.
  A tab or pane dropped where no window is (`Draggable`'s `on_drop_outside`) floats into a new
  window opened `AtPointer`; without a host that drop keeps its old meaning.

Each window holds its own drop target; targets are resolved in the window the `DragPoint` names, in
that window's coordinates (`stacked_at` takes the window), so a tab dragged from one window onto
another's bar or pane lands there.

Stacking: `DockState::windows` keeps windows bottom to top, and a saved layout keeps that order. A
window opens on top, and the focus moving into one puts it on top, so the focused window is the
topmost. `Document::layers()` follows: a window raises its layer and those above it when created (so
a restored layout stacks as saved, whatever order its windows are built in), and a press on a window,
the focus moving into one, or showing a tab of the focused window (`DockState::show`, so a host can
bring back a window something covered) raises its layer, over a fullscreen tab too unless the focus
is in that tab. A drop target is looked for in the window whose layer is on top at the pointer, never
in one covered there by a non-dock layer (a menu, a dialog, the fullscreen tab).

Snapping: `DockHostLayout::snap_window(tab, DockSnap, area)` gives a floating window the left or
right half of `area` (in the dock's window points, as `window_rect`) or all of it (`DockSnap::Fill`,
maximized), remembering its rect before the first snap; `unsnap_window` restores that rect, and
`window_snap` reactively reports the snap for as long as the window stays at the snapped rect (moved
or resized away, it is unsnapped with nothing to go back to).

#### The window manager

The dock is the window manager of its floating windows, for any shell
(`beui-dock-host/src/unstyled/hosting/manager.rs`, `beui-dock-state/src/hidden.rs`), all on
`DockHostLayout` and keyed by `TabId`.

- Areas: `DockHostLayout::set_areas` gives it the rects of the displays it spans, in the same
  points; with none, its own `area()` is its one area. A window belongs to the area it overlaps
  most (`area_of_window`, `holding_area`), `snap_in_area` snaps it within that area, and
  `move_to_area`/`step_area` move it to another keeping its place in proportion, inside the area,
  and keep a snapped window snapped there.
- Hiding: a floating window can be hidden (`hide_window`, `is_hidden`,
  `DockState::hide_window`/`show_window`). It keeps its place but is not drawn, its tabs are not
  placed (`is_shown` is false for them, a tracked read on `DockHostLayout` too, so its dialogs go
  with it), the focus goes to the tab shown before it, and the switcher skips it; showing or
  focusing any of its tabs (`show`, `DockState::focus`) shows it again. Hidden windows are not part
  of the saved layout. `show_desktop` hides every shown window, and pressed again while none has
  been shown since (`shows_desktop`), brings back what it hid; anything shown meanwhile ends it, so
  the next press hides again.
- Groups: a `DockTab`'s `group` (an optional string: which tabs are one app's) is what
  `next_of_group` cycles: the shown windows' tabs of the same group, in tab order.
- Closing: `close_tab` closes a tab through its own `on_close`, so the shell decides what closing
  means.
- Actions: `DockHostLayout::window_actions(disabled)` returns `DockWindowActions`, a set of
  `ActionBuilder`s with ids, labels and icons but no keys, each enabled only when it has something
  to do and never while `disabled` (the shell's memo: the desktop's lock screen) is true:
  `dock.window.close`, `hide`, `snap_left`, `snap_right` (snapping the other way restores),
  `maximize`, `restore`, `fullscreen` (the focused window's tab full screen over its area),
  `show_desktop`, `next_area`, `previous_area`, `next_of_group`, and `dock.switch.next`, `previous`
  and `cancel` (The switcher). They act on the focused window: the focused tab, or the dock dialog
  with the focus (whose `close` closes the dialog), when its window floats and is shown. The shell
  chooses which to bind and to which chords; the demo's Window manager page puts them in a menu.

#### Dragging tabs, panes and sidebars

Dragging a tab picks a drop target from what is under the pointer: a tab bar inserts it between the
tabs there, the middle of a pane joins that pane, and an edge splits it. Holding Alt while dropping
floats the tab into a window, as "Pop out into a window" on a tab's menu does. A window holds one
pane, so a tab dropped anywhere in it joins rather than splits, and that pane's bar is the window's
title bar: anywhere on it that is not a tab drags the window.

Every pane and window wears the same bar: a grip, the tabs, and a close-all button shown only when
every tab in it can close. A middle click closes a tab. A finger picks a tab up by dragging it out of
its bar, across the way the bar scrolls; sliding along the bar scrolls it. Dragging a docked pane's
grip carries the whole pane (`DockState::drop_leaf`), with the same drop targets as a tab.

The grip's menu closes every tab in the pane, ticks "Locked" for the lock the pane is in, offers the
shown tab's menu below its own rows, and turns on icons only (`set_icons_only`):
`DockTabHandle::icons_only` asks the tab face for its icon alone, which the styled dock draws as a
square with the title in a tooltip (the title's first letter where a tab has no icon). Right-clicking
a grip moves the pane's tabs into a sidebar beside its body (`DockState::set_vertical`), whose edge
drags or arrows to a new width (`set_sidebar_width`); a window in that mode has no title bar, only
the sidebar with the grip and close button at its top. A sidebar of icons puts its More menu and
close button below its tabs, in a column. Dragging the sidebar edge (`DockState::drag_sidebar`) left past
`MIN_SIDEBAR_WIDTH` turns icons only on (width `icon_sidebar_width`); dragging right from icons only
turns it off at `MIN_SIDEBAR_WIDTH`, and the sidebar slides to follow the pointer.

`DockHost`'s `drag_modifier` (none unless set) makes the dock a window manager while those modifiers are held:
a primary drag anywhere in a floating window (content, bar or tabs) moves it, and a secondary drag
resizes it from the nearest edge or corner (by thirds of the window). On a docked pane a secondary
drag moves the nearest split and a primary drag is swallowed. Each window and docked pane claims
those presses (a `Claim` with those modifiers), so neither tabs nor content see them. The code is
`beui-dock-host/src/unstyled/modifier_drag.rs`; the desktop app sets Super, and the demo's Docking page
has a switch for Alt.

#### Groups

A place in a tab bar holds an `Entry`: a `Tab` or a `Group`. A group is a tab holding a dock tree of
its own, shown in the pane's body (one pane with a second tab bar, or split panes), and the same
drops work inside it. Dropping a tab onto the middle of another tab groups the two (onto a group, it
joins it); the outer edge of a pane showing a group still splits the outer pane, so the group's drop
zones sit inside a thin band that belongs to its parent. A tab's menu offers "Group with next tab"
and "Split with next tab"; a group's menu ungroups it, closes every tab in it, or floats it into a
window. The tree tidies itself after every change: a group left with one tab turns back into that
tab, and a tab bar holding only a group takes the group's tabs or split in its place.

- Reading: `entries`, `active_entry`, `locate`, `group_tabs`, `tree_leaves`, `surface_of`,
  `is_nested`, `group_title`; `layout_tree` lays out a group's tree as `layout_surface` does a
  surface's. `find`, `all_tabs`, `surface_tabs` and `show` look through groups, and showing a tab
  inside one selects the group in every bar above it.
- Changing: `drop_entry`, `group_with_next`, `split_with_next`, `ungroup`.
- `tree` reads a surface's or group's layout as a `DockTree`, which names tabs but no leaf, split or
  group ids, so it can be compared, sent and rebuilt: `from_tree` makes a state from one, and
  `set_tree` replaces a tree in place, moving in any of its tabs that were elsewhere and keeping the
  focused tab focused.

A pinned group (`insert_pinned_group`) is never tidied away: it stays a group with one tab or none,
cannot be ungrouped or closed from its menu, and admits only the tabs whose home it is (the tabs it
was given), locked or not. While locked, its tabs are also held in it: they can be rearranged inside
it and the group moved with all of them, but a held tab cannot be dragged, popped or carried out
with its pane. `unpin` makes it an ordinary group.

A drag the state would refuse (`admits`, `admits_leaf`) shows no drop marker; over a pane it shows
the `refused` face (the styled dock's is a lock), and letting go does nothing.

#### Locked areas

A `locked` pane makes an area: the pane and every pane split off it for a tab that lives there,
including the groups inside them (`area_of(leaf)` names a leaf's area, the locked pane's `id`).
Every tab declared in the pane, including ones a `ForEach` adds later, lives in that area:
`DockState::home(tab)` answers a `DockHome`, `Area(id)` or `Group(id)` (a pinned group's).

- An area or pinned group is locked unless the user unlocked it (`is_locked(place)`,
  `set_locked(place, locked)`). The lock belongs to the place, not its tabs. Each pane's grip in it
  shows a lock and ticks "Locked" in its menu, which toggles the innermost place the pane is in
  (`lock_of(leaf)`).
- While its place is locked, a tab at home is held (`is_tab_locked`): it can move, split and group
  anywhere inside, the area growing with its splits and shrinking as they empty, but a drop outside
  is refused, it cannot float, and "Pop out into a window" is disabled.
- A locked area keeps out every tab that lives elsewhere and is held, or lives nowhere: only its own
  tabs and tabs of an unlocked place go in, and a pane split off it for one of those is not part of
  it. A group's tab inside another area of its group is not at home.
- A tab away from home is free, so locking a place again holds only the tabs in it; a tab dropped
  back home is held again.
- Seeding and reconciling move a tab just given an area as its home into it while the area is
  locked; `spec` and `from_spec` keep the homes and the unlocked places.

#### Nested and hosted docks

A `Docking` built inside a `DockTab` of another `Docking` joins it (the first one in that tab does,
found through a context the outer dock provides beside `use_dock_tab()`'s) and draws nothing. The
outer dock puts what its children declare in a pinned group keyed `joined:<tab>` in place of that
tab (titled, iconed and closed as the tab), with its pane, split and group keys prefixed
`joined:<tab>/`, so its locked panes stay areas inside the group, and its `DockWindow`s become
windows of the outer dock. Unlocking the group from a grip of its panes lets its tabs go anywhere in
the outer dock.

- A group of one bare pane stands for its hosting tab (`stands_for(group)`, `hosted_by(tab)`,
  `standing_tabs(entry)`): its tab wears the host's title, icon and close, its menu is the bare tab's
  followed by the host's (`menu_tab(leaf)`), and the hosting tab keeps its home, so the group is held
  by the host's place and the grip of its pane locks and unlocks it.
- The outer dock places the inner bodies, built in a scope under the inner `Docking` with the outer
  dock's `use_dock_tab()` and `dock_menu`, so they survive moving. Switching, back history,
  fullscreen and stacking are the outer dock's.
- The inner `DockingLayout` keeps working: `show`, `focused`, `shown`, `contains`, `mode` and
  fullscreen go through the outer dock; `state()` and `snapshot` are the group as the outer dock
  holds it, with `DockState::outside` naming where each tab moved out of the group went (a
  `DockOutside`: the keyed outer pane and index, or the window's rect); `restore`/`reset` rebuild the
  group and put moved-out tabs back (unlocked) when those panes exist.
- On the outer layout, a focused or shown inner tab counts as its hosting tab (`focused`, `shown`),
  `show` of the hosting tab shows its group, and `contains` holds it while the group stands.
- A tab joins when its body is first built, so a hosting tab not yet shown is an ordinary tab until
  then. When the inner `Docking` goes, the group turns back into the tab and every inner tab closes,
  moved-out ones too.
- Each layer saves its own layout: the outer `snapshot` keeps only the hosting tab, so the inner
  dock's owner saves the inner `snapshot` (be-sdk-beui does this for a dock that joins another app's,
  guides/adding_an_app.md) and restores it before or after it joins (a restore made before is grafted
  in once the group stands). An outer `restore` keeps a joined inner dock's layout as it is.
- A `DockDialog` whose parent tab hosts a group hangs from the group's shown tab. The inner dock's
  `DockDialog`s are declared too (`DockDeclaredDialog`); the outer dock presents them over their
  parent's place, building their bodies like tab bodies and handing a close back to the inner
  `on_close`. A pane's `empty` view shows in the outer dock as a tab body does
  (`DockBodies::empty`), and the inner `focus` picks the group's tab when the group first takes the
  hosting tab's place.
- A `DockTab`'s `picture` reaches an outer dock in the same tree; across apps it is not carried (a
  `dock-tab` has no texture; only the shell's own window tabs use pictures).

Every dock crosses the same seam, a `DockHostLink` (`beui-dock/src/link.rs`), plain data both ways
(`DockDeclaration`, `DockHosted`, `DockRequest`, `DockHostEvent`, `DockBodies`). `DockState::spec`
and `from_spec` read and build a state exactly as the `DockSpec` seeding takes (bars, homes and
`outside` included; `DockTree` keeps a pane's `bare` too).

- A `Docking` always declares through one: the `DockJoin` of the tab body it is built in, or else
  the `DockHostSlot` context above it, and it panics if it finds neither. A `DockHost` provides a
  slot whose link draws the dock in place of the `Docking` that claims it (`DockHostLink::view`); a
  platform's slot (`DockHostSlot::new`) draws nothing there and hosts the dock elsewhere.
- A `DockHost` hides any `DockJoin` around it (`DockJoin::none`), so a `Docking` under it stands
  alone even inside another dock's tab, as the demo's Window manager page does with its floating
  windows.
- A host builds the tab bodies itself from `DockBodies`, providing `DockTabMenu` and a
  `DockTabControl` in each body's scope so `dock_menu` and `use_dock_tab()` work there:
  `DockTabControl::local` and `DockBarControl::local` take the `DockTabDriver` and `DockBarDriver` a
  `DockHost` implements, and a platform hands out `DockTabControl::hosted(tab, hosted, request)`
  instead; `DockTabControl::is_hosted` tells such a body apart, and `claim_body` lets one child in
  it claim the tab.
- A `DockHost` given `bars` (or inside one that was, which hands its link on) is joined by bars
  only: it declares the dock's owned bars to that link once a `Docking` is hosted, places only their
  bodies, and while the link's `hosted` is `Some` draws none of them (`bars_away`), keeps its tab
  bodies clear of that hosted state's covers, and reports the bars where the link has them, so
  `DockingLayout::bar_placement` and `place_bar` read and place them there.
- be-sdk-beui provides a platform slot in an editor's frame whose dock joins another app's, and
  wraps any other editor's content in a `DockHost` with the frame's bars link
  (guides/adding_an_app.md). Its `ChildBlock` in a tab body claims that tab's `DockJoin`
  (`use_dock_join()`, `DockJoin::host`, `claim`) for the child editor's dock.
- A link's `withdraw` takes back only the declaration: the bodies `place` gave and the `listen`
  callback stay with the link until the claiming scope ends, so a dock that stops declaring and
  declares again (a child editor's `Docking` torn down and rebuilt) is hosted again with the same
  bodies.
- Nesting happens in the client: a `Docking` inside a tab body of another splices the inner dock in
  as `joined:<tab>` and declares its tabs, dialogs, bars, group titles and see-through tabs as its
  own, naming the tabs hosting a group (`DockDeclaration::hosts`, which keep their panels alive) and
  passing its requests up, so a host only ever sees one declaration.
- A `Docking` hosted by a `DockHost` (`DockHostSlot::is_local`) claims the `DockKeeper` context if
  one is provided above it (one per keeper; a dock nested in it joins it and claims none) and hands
  it a `DockKept`: its layout (`snapshot`'s state, as a memo), the keys of its tabs and bars, and a
  restore that keeps tab ids, so the keeper saves that dock as it would a joined one. be-sdk-beui
  provides one in every editor's frame; an owner that saves its dock itself provides
  `DockKeeper::none()` above it, as be-sdk-shell's `Workspace::new` does.

#### Dock dialogs

A `DockDialog`, declared beside the dock's other children, is a dialog attached to a tab:
`<DockDialog id parent title size modal on_close>content</DockDialog>` hangs from the tab named by
`parent` (or from another dialog, so dialogs stack) as the same anchored card as the shared
`Dialog`: its content at `size` under a header from the `dialog_chrome` render prop (styled: a
bordered card with a title row and close button, `dock.dialog.<tab>` naming its body and
`dock.dialog.<tab>.header` its header), placed `Popover` from the parent's handle with a beak. A
tab's handle is its entry in its strip (`DockTabControl::handle()`); a dialog's is its header. The
overlay hangs from the parent's panel (`OverlayAnchor::Pinned`), so it escapes its pane as a layer,
blocks only that panel, and rises with the parent's window; in space it is shown in the parent's
window. With no handle (a fullscreen tab) it is centred on the panel with no beak. Dragging the
header moves the card and drops the beak until it closes.

- It is not part of the `DockState`: never a tab, never dragged into the dock, not saved; a key
  declared as both a dialog and a tab is only a dialog. It shows exactly while its parent tab does
  (`DockState::is_shown`). Its body is built once when declared and kept while its tab is hidden.
- Focus: a new dialog takes it; the dock tracks which dialog has it beside the focused tab. Pressing
  a dialog focuses it, pressing the parent's pane or tab bar takes it back, and a closing dialog
  returns it to its parent. The content's `use_dock_tab()` is a `DockTabControl` for the dialog,
  whose `focused` follows that and whose `show` focuses it. Escape or back with the focus in a dialog
  closes it through `on_close`.
- `modal` makes the dialog's layer `BlocksParent`, dimmed by `dialog_scrim` over the parent's panel
  (styled: the `Scrim` colour role), with corners `dialog_radius` when the parent is a dialog
  (styled: the card's radius token; `Overlay`'s `scrim_radius`, carried to a host as
  `Scrim::radius`). A press on the parent's panel is shut out and focuses and flashes the innermost
  modal dialog (the handle's `attention`; `DockTabControl::attention` for content in another
  document to take the keyboard itself), unless `dismiss_on_outside_press` makes it close the
  dialog. The focus cannot move into the panel, and while the parent's tab is focused the focus goes
  to its innermost modal dialog. While a tab has a modal dialog (directly or via its dialogs), its
  label shows a dot (`DockTabHandle::waiting`, styled `dock.tab.<tab>.waiting`), which matters once
  another tab hides the dialog.

A small form opened from a tab body's own control is not a dock dialog but the shared `Dialog`, which
escapes its pane as its own layer (in an app, an anchored view of its owner) and finds its tab
through `use_frame_anchor()`, provided in every tab body (`DockTabControl::anchor()`: the tab's strip
entry, none while fullscreen). Both look alike; a dock dialog is for content the dock must hold (a
program's dialog, be-sdk-shell's `WindowDialog`).

#### Tool bars

A `DockBar`, declared beside the dock's other children, is a tool bar belonging to a tab:
`<DockBar id parent placement>body</DockBar>` (`placement` is where it starts). Its owner draws all
of it, grip included. It shows only while its tab is shown, wherever the tab goes, since its
placement is relative to whatever pane shows the tab. A `DockBarPlacement` is:

- `Over(edge, align)`: floating over the tab's body at a `DockEdge` and a `DockAlign` along it
  (start, centre or end, so corners too), `bar_margin` in from it, the body keeping its full size.
  Bars at different alignments of one edge sit side by side, centred ones moving aside only as far as
  needed; bars at one alignment stack away from the edge, nearest first. The bars floating over an
  edge are kept clear together (`keep_clear` on their list).
- `Beside(edge, align)`: docked in a band along the whole length of that edge of the pane, which the
  body gives up room for and every bar beside that edge shares (as thick as its thickest bar; bars
  sit by alignment as floating ones do, and bars at one alignment line up along the edge from its
  start). The styled Docking draws the band with `bar_band` (its fill and one line facing the body).
- `Window { edge, origin }`: popped out into a dock window sized to the bar, `origin` relative to the
  dock. Where the host shows windows, that window asks for `WindowLevel::Above` while `bar_level` is
  `Raised` (a `Window`'s `level` maps any level above `Normal` to it), and reopens at `Normal` when a
  fullscreen window covers the bars. In a stacked dock a popped-out bar floats over the body at its
  edge. The menu docks it again.

Order: a joined layer's bars come after those of the layers around it (a tab hosting a joined dock
lends its bars to that dock's first pane, so the outer editor's bar precedes the inner one's), and
one layer's bars follow the order kept in `DockState`, which starts as declaration order and changes
with drops.

A hidden bar takes no room and draws nothing while its body stays built (as with a hidden tab, so a
joined app's bar region keeps running). The dock offers no way to show it again, so an owner that
lets a bar be hidden needs to offer showing it.

State, saved with the layout: `DockState`'s `bars`, `bars_of(owner)`, `bar_owner`, `bar_placement`,
`set_bar_placement`, `place_bar_at(bar, placement, slot)`, `is_bar_hidden`, `set_bar_hidden`,
`pane_bars(tab)`, `bar_tab(bar)`, `bar_depth(bar)`, `is_bar_shown` (false for a hidden bar).
`DockingLayout::bar_placement`, `place_bar`, `is_bar_hidden`, `hide_bar` and `show_bar` work by key.

The body's `use_dock_bar()` is a `DockBarControl`: `placement()`, `edge()`, `direction()` (how to lay
items out, across for top and bottom edges) and `is_hidden()` are tracked reads; `place` and
`pop_out` move it, `hide` and `show` toggle it, `menu()` is the radio menu of every placement with a
last row that hides the bar, and `begin_move(press)` starts a drag, called from the handle's
`on_drag` once past the drag threshold with the press that went down, then with each move.

The styled `DockBarGrip` (`dock.bar.<bar>.grip`) is such a handle: it drags the bar and opens its menu
when clicked, pressed with Enter, secondary-clicked or long-pressed, never on the release of a press
that dragged it. Its rows are `grip.dock.bar.` plus `DockBarPlacement::name()` (`over.top`,
`beside.left`, `window`; in the submenus `grip.dock.bar.over.ends` and `grip.dock.bar.beside.ends`,
the ends of each edge such as `over.top.left`, `beside.bottom.right`, the submenu row ticked while the
bar is at one of its ends), then `grip.dock.bar.hide`. A bar's body draws on the styled
`DockBarSurface`, a `ToolPill` along the bar's direction: a pill while floating or popped out, the
band's fill while docked (a joined bar's body is cut out of the root's band, so it draws the fill
itself).

Moving a bar: the dock carries the drag, lifting the bar into a layer of its own
(`LayerPolicy::Inert`, at the bars' level) that follows the pointer from where it was grabbed; the
root carries an app's bar the same way (`DockRequest::MoveBar`). A popped-out bar lifts out of its
window too whenever the dock can map that window into its own (`window_point`, which needs the
window's layer space, as be-host gives its apps' windows); a local bar's window stays open, empty,
until the lift ends, so its press keeps steering. A bar in an OS window of a native host stays in it,
its grip asking the host to move the window (`WindowHandle::begin_move`). The dock reads every point
in its own window, so a press begun in another layer still steers.

Each move resolves where the bar would land if let go: the pane's outer band docks it beside that
edge; the next band in (or a bar already floating there) floats it over that edge at the start,
centre or end by the pointer's third of the edge; further in it lands nowhere while moving, and let
go at rest floats over the nearest edge; away from the pane it pops out where let go. Among existing
bars at a placement, the slot is chosen by the pointer against their middles. The layout previews
the landing through a preview `DockState` (read by `bars_at`, the bands and the bar windows while
lifted), opening a gap over `GAP_SECONDS` and growing a new band from nothing; while it would dock,
the lifted bar's `placement()` is the landing place, so its body already lies along that edge. Let
go at `THROW_SPEED` or more (the pointer's `Velocity`), it is thrown: it floats over the edge its
path runs into and never docks. The placement is saved at once and the bar springs into its gap
(`Spring`, from the pointer's velocity); under `Motion::Instant` it lands at once.

Joined bars: a bar declared by a `Docking` that joins another is declared up
(`DockDeclaration::bars`) and drawn by it. A platform that builds a joined bar's body itself provides
`DockBarControl::hosted(bar, hosted, request)` there (`is_hosted` tells it apart): its reads follow
the hosted state, and `place`, `pop_out`, `hide`, `show` and `begin_move` become
`DockRequest::PlaceBar`, `HideBar` and `MoveBar`. A `DockBarMove` carries the pointer in the body's
own coordinates as they were when the press began (through `to_host` of the body's window then and
now) and a `DockDragStage`; the root carries it as if its own grip were dragged, from where the body
was when the carry began (a popped-out bar from where its window is), so a body moving under the
pointer while messages are in flight does not run away from it. A press held in one window that the
pointer carries over another of the same program reports its position in that other window
(`PointerPress::window`); `begin_move` maps it back with `window_point(pos, from, to)` through the
windows' layer spaces (`to_host(window)`). Such a handle calls `end_move` when its press ends, which
drops the bar (a local control ignores it). `DockingLayout::place_bar` of a joined dock asks the root
the same way.

Dock bars: a `DockBar` without a `parent` belongs to the dock itself (`DockState::bars_of(None)`,
`is_dock_bar`). It shows whichever tab is shown (`is_bar_shown` always true, `bar_tab` none) and
wraps the whole dock: `Beside` sits in a band along the dock's edge that every pane and the floating
windows' area give up room for (the dock's `rect`, which windows open in and are clamped to, is what
is left; a fullscreen tab and the stacked/tiled switch still use the whole rect), `Over` floats over
the dock's edge above its floating windows, and `Window` pops out as any bar does (in a stacked dock
it floats over the dock's edge). The dock's bars lie outside every pane's bars; drag targets, slots
and menu work as for a tab's bar, measured against the dock.

- Each run of dock bars floating over one place is its own layer, `Pass` and anchored
  `Placement::Over` (so in an app it escapes its frame and draws over the host's windows), at
  `LayerLevel::Raised` like every popped-out bar's window: floating dock windows pass under them,
  menus and modals over them. While a fullscreen tab is in front they drop to `Normal` and it covers
  them (unless it is fullscreen over an area clear of the dock, such as another display), as they do
  while the owner says something outside covers the dock (`DockHostLayout::set_covered`).
- `DockHostLayout::over_bars_rect(edge, align)` is where each floating run is shown, in the dock's
  coordinates, so the owner can keep toasts and the like clear of it. A tab's bars floating over its
  pane stay in that pane, under the windows covering it.
- Floating dock windows (in a dock that draws its own) never cover a band: on an edge with a band a
  window is kept inside the area the bands leave, shrinking to fit, as on the dock's top and left; on
  an edge without one it may hang past the dock as long as its title strip and its left 96 points
  stay in view, the rule be-host's `desktop::reachable` keeps for windows it shows itself.
- A joining `Docking` declares its dock bars up with no owner (`DockDeclaredBar::owner` is `None`);
  the root gives them to the hosting tab, so they behave as that tab's bars (lent to the joined dock's
  first pane), and the joined dock sees them as its own again (`subtree`/`graft` map the host to
  none). A `DockHost` that only declares its bars up (its `bars`) keeps and draws its dock bars
  itself, so an app whose dock is the root (the desktop's) draws its own. Across the app protocol a
  docked app's dock bars cross with no `owner` too (be-sdk-api PROTOCOL.md, "Docks cross apps").

Bars by key: a dock that joins none (the root, or a standalone dock) places bars by key: a bar's own
`DockBar` id's `key()`, or for a joined bar the `kind` it was declared with (`DockDeclaredBar::kind`:
in-process, the declaring dock's own `key()`; across apps, be-sdk-beui declares `{block type}/{key}`,
as saved layouts are keyed `{block type}/frame`). So every canvas's `tools` bar shares one key,
whichever app instance declared it, while another editor's `tools` bar does not. Placing, dragging,
popping out or docking one bar places every bar of its key the same way
(`DockState::share_bar_placement`), and a later bar adopts its key's placement
(`adopt_bar_placements`). The popped-out bars of a key share one dock window
(`DockState::bar_windows(key)`): it shows the bar of the declarer whose tab was focused last and is
shown (`followed_bar`, by `recent_tabs`), and hides while no such tab is shown. Pressing or focusing
that window focuses no tab, so it keeps following the same one. Every bar of the key stays laid out
in the window, the unfollowed ones clipped to nothing and taking no pointer, so a joined app's bar
region keeps running and switching tabs shows it at once. The window takes the size of the bar it
shows and keeps its top-left corner, where a bar's grip comes first.

#### Fullscreen and notices

One tab at a time can be fullscreen: `DockState::set_fullscreen` with a `DockFullscreen` naming the
tab and an area (a rect in document coordinates, or `None` for the screen it opens on),
`DockingLayout::enter_fullscreen` and `leave_fullscreen` (or `DockHostLayout`'s) from outside, or the `DockTabControl` from
`use_dock_tab()` inside. The panel moves into a floating overlay over that area, wrapped in
`DockHost`'s `fullscreen` render prop (styled: the theme's background), covering the tab bars, other panes, the
dock's windows (even one the focus moves into) and anything else there, and nothing under it hears
the pointer; the rest of the document stays live. Leaving puts the panel back without rebuilding it.
The pane it left draws the `away` render prop (styled: a note and a Leave full screen button), seen
where the fullscreen area does not cover it, such as another screen.

- Showing or focusing the fullscreen tab (`DockingLayout::show`, the switcher, a press on its content)
  raises the overlay above the floating windows, its own pane's window included.
- Where a host opens windows (`opens_windows`), the panel goes into a `Window` over the area instead,
  so the host stacks it among its other windows: a window opened or focused later comes in front,
  and focusing the fullscreen tab raises it again and gives it the keyboard.
- The dock binds no key to leave; whoever enters fullscreen offers the way out (the demo's Escape).
  `DockTabControl::offer_fullscreen` adds Full screen / Leave full screen to the tab's right-click
  menu, which suits content that has its own way out.

`DockTabControl::show_notice(notice)`, a `Memo<Option<String>>` kept until the content's scope ends,
is what the content needs said about it right now: while `Some`, the tab, the dialog's header and the
stacked bar show it in place of the title, in the accent colour on an accent-soft fill
(`DockTabHandle::notice`, `DockDialogHandle::notice`, `DockStackHandle::notice`).

### Progress

`styled::Progress` is a progress bar over `unstyled::Progress`. `value` is `Some(fraction)` for known
progress and `None` (the default) for unknown: the accessibility node is busy and a segment sweeps
along the bar, read from the frame clock through the handle's `phase`. It fills its width unless
given one; `INLINE_PROGRESS_WIDTH` suits one beside a label or button. The sweep runs only while the
bar is laid out and motion is animated: behind a false `Show`, in a hidden tab, or under reduced
motion it asks for no frames and rests in the middle. It uses `component_placed` (`measure(node)`
being `Some`), which any component that should only work while on screen can use.

### Drag and drop

Moving something within a document is `unstyled::Draggable` on the thing moved and
`unstyled::DropTarget` on each place it may land. Nothing else wires them: every document keeps one
drag board, a draggable puts its payload on it, and targets whose payload type matches hear about it.

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

`Draggable`:

- The payload is a `Prop<Option<P>>`; a source that may not move right now (a read-only block, a
  disabled slot) binds a memo answering `None`.
- A press becomes a drag only once the pointer travels `threshold`; one that never does is reported
  through `on_click`, so a clickable, draggable row does not select itself at the end of a drag.
  `on_drag_change` says when a drag starts and ends (for a source that dims itself while carried).
- Nested draggables (a card in its pile) set `capture_presses`, so a press reaches the innermost, as
  `unstyled::Button` wins a press; of overlapping ones only the topmost hears it.
- The `preview` is shown beside the pointer in an inert overlay at `LayerLevel::Top`, above every
  window. Where windows open (`Context::opens_windows`), it is shown in the window the pointer is
  over (the overlay's `shown_in`, the board's `DragPoint::window`), so a drag between windows (a tab
  between space's panels) shows its preview there. It follows the pointer the document saw: the
  board is fed the pointer each frame even where a floating overlay covers the source, so a drag over
  a floating window keeps going.
- A finger drag waits for `TOUCH_DRAG_THRESHOLD` and for the catcher to hold it rather than scroll,
  so a Draggable in a scrolling list takes finger drags only where it sets `touch_drags`; its preview
  sits above the finger.

`DropTarget`:

- It is registered by the rectangle its content was laid out at; only its on-screen part counts.
- The target under the pointer that accepts the payload takes the drop (`accepts` filters by value; a
  target of another payload type never sees the drag). Of the targets under the pointer the one
  painted last wins (innermost in the tree, or in the higher layer, whatever order they were built
  in), so a slot inside a column inside a sidebar can each take its own drop.
- `DropHandle` carries `carrying` (true for every target that would accept the drag) and `over` (true
  for the one that would take it now). `on_over` reports payload and pointer as it moves, for targets
  that work out where inside themselves the drop lands (the dock's tab bars and split zones, a
  timeline's insertion point) and for live reordering (a filmstrip).
- The drop is handed over before the source hears `on_drag_change(false)`, so the source can keep
  what it set up until the drop has used it.

A drag between editors (a block from the file tree to another editor) crosses apps, so it goes
through the host with `Editor::drag` and `accept_drag` (guides/adding_an_app.md). The host carries it
on its own document's board: an app's drag request puts it there with `Board::begin`, each app region
is a `DropTarget` for it, and the board drops whatever it carries when the primary button or finger
is let go, wherever the press was, so a drag no `Draggable` began still ends; a cancelled touch ends
it without a drop.

### Pan and zoom

`unstyled::PanZoom` turns wheel, trackpad and middle-button gestures over a rectangle into camera
movement. Its `view` is a `Binding<PanZoomView>` (the world point at the viewport's middle and a
scale) that gestures write; pass your own signal to own the camera, or leave the prop out:

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

Scroll pans, Shift+scroll pans sideways, middle-button or two-finger drag pans, and Ctrl+scroll, a
trackpad pinch or a two-finger pinch zooms around the pointer or the point between the fingers. As a
tab stop, the arrows pan, `+` and `-` zoom around the viewport's middle, and `0` resets the scale to
one. The handle carries the camera as a `CanvasView` for a `<Canvas>`, the `scale` for interactive
content that should grow with the zoom (content that only draws is a `CanvasItem scales=true`
instead, guides/pan_and_zoom.md), whether a pan is in progress, and whether it has focus (for a focus
ring). Since the view is the caller's, a toolbar button, a fit command or a host syncing several
editors writes the same signal; the camera an editor is handed (guides/pan_and_zoom.md) reaches a
beui editor the same way. Momentum, snapping and clamping to content are the caller's; PanZoom only
applies `min_scale` and `max_scale`.

### Picking a file

`beui::reactive::create_file_picker(picked)` gives a component a `FilePicker`:
`open(FileFilter::new(name, extensions, mime_types))` asks the platform for one file, `picking()` is
true until the answer arrives (a button takes it as `disabled`), and `picked` gets the `PickedFile`
(name and bytes) or why it could not be read; a cancelled pick only clears `picking`.
`pick_file(filter, picked)` is the one-shot form underneath, handed the raw `FilePick`.

Nothing blocks while the dialog is open: the document queues the request, `Context` hands it to the
runner in `FrameOutput::file_picks`, and the runner passes each to `Platform::pick_file` with a
`FilePickAnswer` the platform sends when the person is done (winit from a thread running the native
dialog, the web runner from an `<input type=file>`, Android from `BeuiActivity`, an app through its
host; a platform without a picker answers that it has none). The document delivers the answer at the
start of a later frame, inside its reactive scope, so `picked` can write signals like an event
handler. A host that runs beui itself handles the requests in `file_picks` the same way.

### Forms and text fields

On the HTML core a plain single-line `TextInput` is a real `<input>`, so the browser edits it: IME,
selection, its context menu, autofill and password managers. Unstyled `TextInput` switches on `CORE`
and builds `<input>` (through the [platform escape](#the-platform-escape), wrapped in an
`Interactive` for focus, `focused` and hover) when it is not `multiline`, its style has no `prose`,
and it does not `select_on_focus`. Everything else (every `TextArea`, anything rich: marks, syntax
colouring, glyphs, completion) is beui's own on both cores, as is every field natively. The value
stays bound both ways, Enter calls `on_submit`, and styled draws its frame, fill and focus ring
around the `<input>` as around beui's own field: the `<input>` draws no box, takes the field's font,
colour, caret colour and padding, and its placeholder and selection colours through
`--beui-placeholder` and `--beui-selection`. A new kind of single-line field is a variant of
`InputKind` rather than a component of its own; a field needing something an `<input>` cannot express
stays beui's own on both cores ([why](../decisions/beui_html_text_input_is_a_native_input.md)).

`TextInput` (unstyled and styled) takes:

- `kind: InputKind`: `Text` (default), `Password` (masked on both cores), `Email`, `Search`, `Url`,
  `Tel`; the `<input type>` on HTML; natively only `Password` changes anything.
- `name` and `autocomplete` (strings in the HTML vocabulary: `username`, `email`,
  `current-password`, `new-password`, `nickname`, `one-time-code`, `off`, ...): the form name and
  autofill hint on HTML, inert natively.

`Form` (`beui::unstyled::Form`, re-exported by styled) takes `action` (the URL a browser posts to),
`hidden` (`Vec<(&'static str, String)>`, hidden fields' names and values) and `on_submit`. On HTML it
is a `<form method="post" action novalidate>` holding the hidden `<input>`s and its content, so a
server-rendered page posts natively before and without wasm; once running, `on_submit` turns a submit
into a call. Natively it is no node: Enter in any field inside it and its submit buttons call
`on_submit`. A submit button is `Button submit=true` (unstyled and styled), a
`<button type="submit">` on HTML. A navigation link is `href` on unstyled `Button` (so styled
`Button` and `Link`), an `<a href>` on HTML that navigates before wasm loads; natively `href` is inert
and `on_click` navigates.

```rust
view! {
    <Form action="/auth/login" hidden={vec![("csrf", csrf), ("code", code)]}>
        <List spacing=Space::Md>
            <TextInput value={email} label="Email" kind=InputKind::Email
                name="email" autocomplete="username" />
            <TextInput value={password} label="Password" kind=InputKind::Password
                name="password" autocomplete="current-password" />
            <Button label="Sign in" variant=ButtonVariant::Primary submit=true />
            <Link label="Forgot your password?" href={Some("/auth/forgot".to_owned())} />
        </List>
    </Form>
}
```

`render_to_string` writes each `<input>` with its `type`, `name`, `autocomplete`, `placeholder` and
escaped `value`, and `hydrate` claims them in place, keeping whatever was typed before wasm loaded.

### The platform escape

`beui::escape::html_escape(build)` puts an element of the platform's own into the tree: `build` gets
an `&Escape` and returns a `beui_html::Node` made with beui-html's `view!` and `html::*` elements
(`beui::escape::beui_html`), with their attributes and events; the result is a beui `NodeId` like any
other, placed and measured like a beui node and decorated by an `Interactive` around it. It renders
into server-rendered pages and hydrates as beui-html does.

- `escape.embed(child)` slots a beui `Child` into those nodes.
- `escape.style(property, value)` sets the root's inline CSS. The root takes no `style` attribute of
  its own, because the core owns it (a parent's layout writes to it too).
- `Escape::color` and `Escape::font` spell a beui `Color` or `FontId` in CSS.
- `beui_html::Node::number`, `set_number` and `call` read and write a live element's properties
  (`scrollTop`) and call its methods (`focus`).

It exists only on the HTML core and panics natively, so call it under `if CORE == Core::Html`, in
unstyled or styled, for a component whose HTML form is the browser's own element (a field, a form, a
link, a scroller), with a beui version in the other branch keeping the same public API. That is what
a `CORE` switch is for: not to work around a primitive one core lacks, and the cores gain no
knowledge of fields, forms, links or scrolling ([why](../decisions/beui_platform_escape_only_for_browser_elements.md)).

### Hold the pointer

A control that looks around rather than points (a first-person scene, a modeller's orbit) is
`unstyled::PointerLock`. It is a tab stop that takes the pointer when pressed; while it holds it the
pointer stops, is hidden, and reports how far it moved instead of where it is:

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

`locked` is a `Binding<bool>`, like a `Toggle`'s `checked`. It lets go on Escape, on losing focus and
when removed; keys it does not use go to `on_key` (for a control that also walks with the keyboard).

beui asks rather than holds the pointer itself: `PointerLock` sets `pointer_locked` on its
`Interactive`, the document publishes whether any `Interactive` asks as
`WindowRequests::pointer_locked`, and whoever shows the document does it: the winit runner locks and
hides the cursor, and an app region passes it to the host in its `WindowRequests`
(guides/adding_an_app.md). Platform behaviour:

- A platform reads raw, unaccelerated motion (winit's `DeviceEvent::PointerMotion`, libinput's
  unaccelerated deltas under drm) only while `Runner::window()`, the state it was shown, holds the
  pointer, and meanwhile sends no absolute `PointerMoved` (winit, web and drm alike).
- winit lets the grab go when its window loses focus and retakes it on focus only if the shown state
  still asks, since X11's and macOS's grabs outlive focus.
- A browser drops its lock itself on blur and on Escape (which the page never sees), and refuses one
  asked for without a user gesture; the web platform (`lock::Lock`) reports either, while the shown
  state still asks, as `Input::PointerUnlocked`. The document answers by clearing focus, as for losing
  the keyboard, so a `PointerLock` lets go and the next press takes it again; a space host lets its
  grab go, as on blur. If the shown state still asks after that, the next mouse press asks the browser
  again from inside its gesture and reaches nothing, nor does its release.

Motion arrives as `Event::PointerMotion`, a delta with no position, delivered to the focused node
while the pointer is held and nowhere otherwise. So only the control holding the pointer sees motion,
and a window that loses the keyboard gives the pointer back.

### Draw with the gpu

A viewport no arrangement of nodes can produce (a 3D scene, a ray tracer, a map) is a `Drawing` node
whose callback paints a gpu `Drawing`: `draw_gpu(drawing)` builds one that paints a `Shape::Drawing`
over the node's rectangle. The renderer runs the `Draw` behind it where the shape sits in paint
order, so nodes written after it still paint over it.

```rust
view! {
    <Drawing
        draw={Prop::dynamic(move || draw_gpu(drawing.get()))}
        @sizing=ItemSize::Percent(100.0)
    />
}
```

`Draw` has a gpu frame's two halves. `prepare` gets the device, queue and an encoder, for passes of its
own (rendering into its textures, writing its buffers). `paint` gets the render pass beui is painting
the document into, and draws there. Both get a `DrawAt`: the rectangle and clip in physical pixels,
the whole target's size, the scale, the target's texture format (what a pipeline built on the first
frame is built against), and `renderer`, a number the same for every frame one renderer draws and
different for any other.

- Key gpu resources shared between drawings by `renderer`, not by the device: wgpu devices of two
  instances can compare equal. The color wheel (beui-components-styled's `color_wheel/gpu.rs`) shares
  one pipeline that way.
- Leave the pass's viewport and scissor as you found them, since beui keeps painting into the same
  pass: position what you draw from the given rectangle, as beui's own shader does, rather than by
  setting a viewport.

To show a texture, use `beui::native::texture_quad` instead of writing a pipeline: `TextureDraw` is a
`Draw` filling its rect with one texture, and `Quad` sets the source rect, a rotation about the
centre, opacity, `corner_radius` (in pixels, rounding the whole texture's corners, so a source rect
that cuts the texture keeps its cut edges square), `opaque` (alpha read as 1) and `srgb_encoded`
(sRGB values in a unorm texture, decoded when the target encodes). A host drawing several textures in
one `Draw` binds them with `Pipelines` and prepares them with `Quads`; one painting outside a `Draw`
(the DRM output) calls `Pipelines::paint`. `Pipelines::erasing` draws the same quads by clearing what
they cover, which is how the browser's app host cuts the holes its app canvases show through.

`beui::native::drawing` wraps a `Draw` in a `Drawing`, and two drawings are the same shape when they
are the same `Rc`. That decides whether the frame changed: a drawing whose contents moved is a new
`Drawing`, and a frame showing the same thing keeps the one it had, which leaves an idle editor idle.
The gpu resources belong to the `Draw`, not the drawing: keep them in an `Rc<RefCell<Option<..>>>`
the drawing clones and build them on the first `prepare` from the format `DrawAt` names; a headless
test, which never builds a `Renderer`, then never opens a device. The `be_app_scene_3d` editor is the
worked example.

`Draw`, `DrawAt`, `drawing` and `painted_drawing` live in beui-draw, which holds only them over
beui-core and wgpu, so code that draws with the gpu does not build against the renderer; programs
reach them as `beui::native`. A crate under beui that draws with the gpu depends on beui-draw behind a
feature that beui turns on outside the HTML core, as beui-components-styled's `gpu` does.

## Imports

Program code imports `beui::reactive::*` for the framework and `beui::styled::*`
for components, and names anything else (`beui::unstyled::X`,
`beui::styled::theme::X`) on its own line. `beui::primitives` is for beui's own
crates, its tests and beui-demo's base pages; the source lint rejects it
anywhere else, since a primitive takes raw colours and pixels where a styled
component takes tokens.

## Use beui in a standalone program

A standalone program depends on beui-host as well as beui. It builds its
document once and implements `Program`:

```rust
use beui::reactive::*;
use beui::styled::*;
use beui::{Program, Context, Document, NodeId, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui_host::native::run(CounterProgram::new())
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

struct CounterProgram {
    document: Document,
}

impl CounterProgram {
    fn new() -> Self {
        Self {
            document: build(|| view! { <Root title="Counter"><Counter /></Root> }),
        }
    }
}

impl Program for CounterProgram {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
```

The demo runs with `./scripts/buck run //crates/beui-demo:native`.

### The `Root` and the window's appearance

`build` takes a closure returning a `Root`, which is not a node: every document
built with it has exactly one `Root` at the top, holding one child, and a node
in its place does not compile. `primitives::Root` takes:

- `clear_color`: what the window is cleared to wherever nothing paints, so a
  program needs no frame filling the window.
- an optional `title`.
- `system_bars` (`SystemBars::Light` or `Dark`): what is behind Android's status
  and navigation bars, whose icons turn dark over `Light`.

`styled::Root` takes a `ColorRole` for `clear_color`, defaulting to
`Background`, and picks `system_bars` from the theme's kind unless given. Every
prop is reactive: each time the document is shown it hands its appearance
(`beui_core::window::Appearance`) to the context, in `WindowState`, and the
runner clears with its colour, labels the accessibility tree with its title and
passes it to `Platform::show_window`. winit retitles the window; the web
platform retitles the tab and colours the page's body; Android follows
`system_bars`; DRM, headless and apps ignore the title.

In an app, `BeuiRoot::View` returns the editor's own `Root`, whose `clear_color`
the app's surface is cleared to (the theme's background unless the editor names
another role); the app protocol carries no title, and be-sdk-beui puts the
frame's chrome around the root's child. A preview, drawn over something of the
host's, is rooted in a transparent `Root`; an app dialog (a creation dialog,
artifact settings) in `SurfaceRaised`, the card it fills. Holes are cut from the
clear colour, not left by it: a child editor's `Embed` punches its rectangle, and
a painting with holes records the colour as its first primitive so they cut it.

Documents drawn over another one (the inspector, the software keyboard) are
built with `Document::new`, `enter` and `set_root` and have no `Root`, so they
leave the window's appearance alone.

### Features and platforms

- No features: the document, components, layout, input model and painting
  output; enough for headless logic tests.
- `render`: adds the wgpu renderer without creating a window. Embedded hosts use
  this level.
- `window` (the default): adds the native runner and enables `render`. On the
  desktop it is winit's. On Android it is beui's own
  (`crates/beui-platform-android`, with its Java in its `android` directory).
- `web`: enables `render` and adds the browser runner,
  `beui_host::native::run_web(element_id, options, layers, program)`, which draws
  with wgpu into the element if it is a canvas and into a canvas it adds
  otherwise. `crates/beui-web-demo` is the demo in a browser:
  `./scripts/buck run //crates/beui-web-demo:web` serves it on
  http://127.0.0.1:8070.

On Android the program's activity extends `com.be3.beui.BeuiActivity`, whose
`BeuiView` is the surface, takes touches, keys and the soft keyboard's input
connection, and hosts AccessKit. It is a view so that input reaches it through
the view system: TalkBack's touch exploration arrives as hover events on it,
which NativeActivity's input queue never delivers
([decision](../decisions/beui_android_uses_its_own_view.md)). The library defines
`#[unsafe(no_mangle)] fn android_main(program: beui_host::native::AndroidApp)`,
which beui calls on a thread of its own, and which calls
`beui_host::native::run_with`. The activity draws edge to edge under transparent
status and navigation bars: the program fills the safe area inside them, the bars
show its clear colour, and their icons turn dark when its `Root`'s `system_bars`
is `Light`.

The web runner draws into its canvas with WebGPU, or WebGL where the browser
has no WebGPU. It reads the keyboard through a hidden text area, so pasting and
composition behave like any other input on the page. A press on the canvas
focuses that text area, and so does a text field taking beui's focus any other
way (the keyboard, a script, `drive click`), so the browser's keys reach it.
Like the desktop runner, it asks the browser for a frame only when an event
arrived, something asked for a repaint, or a `Waker` was woken.

The soft keyboard takes its room out of the window rather than covering it: on
Android its inset joins the safe area, and on the web the pages' viewport meta
asks for `interactive-widget=resizes-content`. When the rectangle a document is
shown in changes size while the focus takes text, the document scrolls the
focused field into what is left, through every scroll it sits in.

### Running: `RunOptions` and the rest of `Program`

`beui_host::native::run_with` takes `RunOptions`; `run` takes only the program
and uses the defaults. Both load wgpu. Besides the program id and starting size,
`RunOptions` carries:

- `device_extensions`: the Vulkan extensions wgpu's device is opened with
  (be-host's import Wayland clients' buffers).
- `platforms`: platforms the native runners offer beside their own, such as
  `beui_platform_drm::Drm` (drives the displays and input devices itself), for
  `BEUI_PLATFORM` to pick by name. `beui_runner::environment::choose` takes every
  name a program offers; the first, the native one, is the default.
  `beui_host::native::Headless` draws offscreen with no window or display server.

The rest of `Program` is optional:

- `setup(&Setup)` runs once, after the gpu exists and before the first frame.
  `Setup` hands over a `Waker` and whatever the renderer and runner provide:
  `setup.get::<beui_host::native::GpuSetup>()` is the wgpu device, queue and
  surface format, for a program that paints with the gpu itself through
  `draw_gpu`; on the desktop `setup.get::<Arc<beui_host::native::winit::window::Window>>()`
  is the window. `Waker::wake` can be called from any thread and asks the runner
  for another frame: it is how work finishing elsewhere is pushed to the UI.
- `close_requested` is asked when the window is closed, and can refuse by
  returning `false` (to ask about unsaved work first, then call
  `Context::close_window`).
- `exiting` runs once on the way out; the runner drops the program right after
  it, while the GPU device and the platform's displays are still there, and no
  frame runs after it. Anything the program keeps in a `thread_local!` has to be
  taken out here: the main thread's thread-locals are dropped only by `exit()`
  after `main` returns, in no set order, after the GPU and the rest of the
  runner are gone, and dropping GPU or reactive state there aborts or crashes
  the process. be-host's `Shell::exiting` takes down the Wayland compositor, the
  app runtimes and their worker threads, and the displays this way.
- `lifecycle(Lifecycle)` hears the program leave and come back to the screen
  (not the keyboard): `Background` when the window is occluded or minimised
  (winit), the activity pauses (Android `onPause`) or the tab is hidden or left
  (`visibilitychange`, `pagehide`); `Foreground` when it returns; `Focused` when
  the window gains focus; `NetworkChanged` when the browser goes `online` or
  Android's default network becomes available. The process may be killed
  without another frame after `Background`, so that is where unsaved work is put
  somewhere durable, without blocking (Android gives `onPause` little time).
  Platforms call `Runner::lifecycle`; the document's own focus still comes as
  `Event::Focus`.
- `busy`: whether work the program is waiting on (a server request, an app's
  frame) will still change what it shows. Only a driver reads it.
- `environment`: the variables a program needs to open its windows inside a
  program that hosts programs (be-host's Wayland compositor). Only a driver reads
  it, for `drive env` and `drive exec`.

From inside a frame the program asks the window for things through the
`Context`: `set_fullscreen`, `set_ime_area` for an input it draws itself,
`set_zoom_factor`, `close_window`, and `with_input` to show a document only the
input meant for it, which is how the inspector keeps the program's keys while
its own panel has the keyboard (guides/beui_keyboard.md, One input path).

A runner hands the document its events in batches, one per frame. A press that
follows typing waits for the next frame, so text typed before a click reaches
the field it was typed into, however many events arrive between two frames.

### Drivers

A driver is a layer, `beui_layer_drive::Drive`; guides/running/drive.md (How it
works) covers its taps, its session and its dialogs. The native run functions
stack it where `BEUI_LAYERS` names `drive` and serve its commands on
`BEUI_DRIVE_SOCKET`, and pick their platform and renderer from `BEUI_PLATFORM`
and `BEUI_RENDERER` (`beui_runner::environment`; `environment::drive_taps` works
a stack's taps out of `BEUI_LAYERS`, so `drive,space,drive` is
`Drive::taps(inbox, &[Some("space"), None])`). be-host's web build stacks one that
its `automate` export feeds (`run_web` takes the layers). What a driver relies on
from beui:

- `Context::keep_accessibility`, which a bare `Context` and a headless app keep
  by default and other runners only for a driver.
- Input commands become `Incoming`s already in points, skipping the platforms'
  intake (`beui_core::input::typing` names the keys); a tap under another layer
  sends physical pixels (`Input::scaled`), as a platform does.
- It steps a stopped clock in `now`, and answers in `framed` once a frame asks
  for no other and the program is not `busy`, reading the program's
  `FrameOutput` (actions, paintings, overflows).
- It acts on the window only through the `PlatformServices` below its lowest
  tap: `resize` asks `PlatformServices::resize`, which only a headless window
  answers, and `state` reads the size, scale and fullscreen the window last
  reported. It keeps the focus `focus` and `blur` gave.

The headless platform's window is `beui_platform_headless::Simulation`
(`beui_host::native::Simulation`), a `PlatformServices` like any other: it keeps
a clipboard, grows to its screen for fullscreen, and reports its size, scale and
fullscreen through its `WindowReports`. beui's own tests drive one the same way,
with a renderer that draws nothing. `Renderer::capture` reads the presented frame
back as a `beui_runner::renderer::Capture` for a screenshot.

### Unseen windows

A window nobody can see is `Runner::set_occluded`: its frames still run (layout,
paint, accessibility, the driver), but the runner hands each one's output to
`Renderer::note` instead of `prepare` and presents nothing. `note` keeps only
what the frame damaged, so the wgpu `Target` draws the union of every unseen
frame's damage into its retained frame the next time it draws; a renderer
without `note` prepares as usual. The headless platform is always occluded, so
it draws only for a driver's `shot`.

`Context::seen` tells the program whether the frame it is running will be
drawn. A `shot` makes an occluded window seen until it is taken, and it is taken
in the first frame whose program says through `Program::drawing` that it has
nothing still to draw. be-host passes this to its apps: each `DrawFrame` carries
`seen`; an unseen app runs its frames (layout, its description, children)
without encoding or presenting them and keeps what they damaged, except its
previews, which another editor shows as pictures and which it paints either way.
When the host is seen again it asks each app that drew unseen for a frame, which
paints all of it, and `drawing` holds the shot until those frames have arrived.

### Windows and window hosts

A program has one `Document`, one reactive graph and one focus, but may show
several top-level windows.

**Output.** A frame's `FrameOutput` holds one `WindowOutput` per window in
`windows`: its `id` (`beui::WindowId`; `WindowId::MAIN` is the window the program
starts in, `WindowId::next()` makes a new one), its `size` in points, its display
`layers`, its `WindowState` (cursor, IME area, appearance), its own `damage`,
`moved` and `changed`, and `pixels_per_point`, the scale it was drawn for, which
a host draws it at. `FrameOutput::main()` is the main window,
`FrameOutput::window(id)` any other. What happens once for the whole program
(copies, pastes, file picks, closing, actions, accessibility) stays on
`FrameOutput`. A renderer, `painting` and `quads` each take one `WindowOutput`;
`painting::overflows` reads every window.

**Requests.** The program asks its host for windows with
`Context::request_window`, which collects `WindowRequest`s into
`FrameOutput::window_requests`:

- `Open { window, placement, size, role }`. The `WindowRole` (the `Window`
  component's `role`) is one of:
  - `Normal`.
  - `Keyboard`, for an on-screen keyboard: the host never focuses it and places
    it by the field it types into.
  - `Popup`, a menu of the window it is placed over: the host never focuses it,
    and closes it (reporting `Closed`) when a press lands outside it and outside
    the popups opened over it, keeping that press from what is under it as a
    `BlocksAll` menu does. be-host does so for every app window it overlays,
    anywhere in its own window, and beui-layer-space for every panel, a press in
    any other panel or none closing it.
  - `Overlay`, the window an `Overlay` opens to show itself in: the host keeps it
    in front of the window it is placed over, never focuses it and lets presses
    through wherever it paints nothing; the program closes it itself.

  A hosted window whose role changes is opened again with the new one.
- `BeginMove(window)`, `BeginResize { window, edge }`: the host then moves or
  resizes it, as an OS does after winit's `drag_window`.
- `Raise(window)`: sent whenever `Document::raise_overlay` raises a hosted
  window, as a dock does when one of its windows takes the focus, so the host
  stacks its windows as the document does. The host keeps the windows opened
  over a window (its dialogs and popups) in front of it. A frame's requests for
  windows already open go before the `Open`s of new ones, so a window opened in
  that frame ends up in front.
- `Resize { window, size }`: the program wants another size; the host answers
  with the size it gives.
- `Close(window)`.
- `Place { window, placement }` (`WindowHandle::place`): move an open window,
  keeping its size, to where the placement says, as `Open` would have put it.
  Hosts with nowhere to put it (a space) ignore it.

A `WindowPlacement` is `AtPointer`, `Over { window, at }` (its top-left corner
at `at` in that window's points, where the program would draw it inside that
window), or `Host(HostPlacement)`: a value only the host understands (a screen
position, a 3D pose), which the program stores and hands back unchanged.

**Reports.** The host answers with `WindowReport`s pushed as
`Incoming::Window`, which the runner applies before the frame lays out:

- `Resized { window, size }` in points.
- `Placed { window, placement }`.
- `Moved { window, origin }`: where the window's corner is, in points, in a
  space all the host's windows share (as beui-platform-drm's desktop is).
- `Rastered { window, pixels_per_point }` (below).
- `Closed(window)`.
- `Focused { window, focused }`: the host's window focus, as a window manager
  moves it on a click or to a window it opens.

`Context::window_size`, `Context::window_placement` and `Context::window_origin`
read them (`beui::window::window_origin` in a component).
`beui::set_work_area(window, area)` keeps on the document the part of a window
its windows should stay in (`Document::work_area`); a root `Docking` whose
windows the host opens sets it to its area inside its dock-level bands, and the
app platform reports it to the host as the view's `work-area`.

**Hosting.** `Context::window_hosting` says which windows a host opens, as a
`WindowHosting`:

- `None`: every platform on its own; a window is shown inside the main one
  instead.
- `Any`: a layer opens every window, as space does. `Context::opens_windows` is
  true only for this.
- `Placed`: the platform opens only the windows it placed itself, a `Window`
  opened with a `WindowPlacement::Host` the platform handed out through a handle
  in `Setup` (as be-sdk-beui-platform-api's `Views` and its
  `views.placement(id)` do). Every other `Window` is an overlay inside the
  window it was opened in, so an app's docked windows stay inside its region.

A platform may move between them: the app platform hosts every window while its
host shows the windows of one of its views (guides/adding_an_app.md), and a
`Window` follows, opening as a real window or an overlay from the frame the
hosting changes. A platform with no main window of its own (be-sdk-beui-platform,
beui-platform-drm) sets `Context::set_shows_main(false)`: the document then
paints neither the root nor anything opened in the main window, so the root just
holds the program's `Window`s.

#### The `Window` component

A program makes a window with the `Window` component (`beui::Window`,
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

Where a host opens windows it is a real window: its content is laid out from its
own corner, `(0, 0)`, at its size (the size the host last reported, else `size`)
and painted into its own `WindowOutput`. The document asks the host to `Open` it,
with `placement` and `size`, when it opens and to `Close` it when it closes or
leaves the tree. When `size` changes while it is open, to something other than
the size the host last reported, the document asks with `Resize`; the window
keeps the reported size until the host answers, so a program that follows
`Resized` reports in its `size` never fights the host. `on_report` hears the
host's `Resized`, `Placed` (the `HostPlacement` to store and hand back as
`placement` next time), `Rastered` and `Closed`, after which the window stays
shut until the program closes and opens it again.

Elsewhere it is an `Overlay` at `at` (`Placement::At`, `LayerPolicy::Pass`, no
focus trap) holding its content at `size`, so a program looks the same either
way. A `Popup` one is too: closing it on a press outside is then left to the
program, as be-host-wayland's popups need.

`WindowHandle` (also `use_window()` inside the content) carries the `id`, the
shown `size` and `is_window()`, which says which of the two it is
(`real_window()` is the hosted window it is drawn in: its own, or the one around
the overlay). `begin_move()`, `begin_resize(edge)` and `close()` ask the host and
return whether it took the request: false means the window is an overlay and
the caller moves or resizes it itself. Overlays opened inside a window (menus,
tooltips) are laid out inside that window and paint into it.

#### Inside the document

The window is an `OverlayNode` carrying a `WindowSpec`
(`Document::set_overlay_window`), and each `Layer` names its `window`: the
window's own layer and everything opened from inside it.
`Document::window_layers(id)` is one window's layers with the window's own at
index 0, which hit testing, press claims and forwarding use, so a press in one
window never reaches another's nodes at the same coordinates.
`Document::window_of` names the window a node is in, `window_rect` its rectangle
and `open_windows` the windows the host shows.

**Focus.** Focus, key routing and tab order are one across every window, except
that a host's `Focused { window, focused: false }` takes the focus from a node
inside that window (remembered per window) and `focused: true` gives it back if
nothing else has taken it since. A window with nothing to give back first lets
its `on_report` hear the `Focused` and choose: a dock window the host focuses
(`window_focused` in `beui-dock-host/src/unstyled.rs`) focuses its pane and, unless
the keyboard is already inside it, hands it to the shown tab (`give_keyboard`:
the node that tab last had, else its first focusable). So a tab floated into a
window the host focuses on opening takes the keyboard with it, while the main
window keeps the remaining tab's focus for when it is focused again. If the
keyboard is still not inside the window after that, the document focuses its
first focusable in tab order (`focus_into`, which waits for the content if it is
not built yet), so a plain `Window` the host focuses on opening gets the keys
too. The main window gets no such fallback.

Where the platform opens only the windows it placed (`Placed`), `Focused {
window, focused: true }` instead moves the focus into that window, keeping the
focus it takes from another window for when that one is focused again and giving
back the one this window last had, and focuses nothing else; `focused: false`
leaves the focus where it is. A `Resized` report for the window holding the
focus reveals a focused text field the way a resized main window does.

**Keys and actions.** A key listener at `KeyAt::Window` and an action,
registered inside a `Window`, belong to the hosted window that holds it
(`Document::host_window_of`): they hear a key only while the key's window is
theirs (`Document::key_window`: the window of the focused node, else the one the
host focused last) or while no window is known, so two app views each keep their
own Escape and shortcuts. `Context::request_action_in(window, id)` runs that
window's action of that id, and the actions the document describes come once
more per window, as an `ActionGroup` whose `window` names it.

**Damage and scrolling.** Every window's rectangle starts at the origin, so
damage is told apart by window: the paint cache files what each root's nodes
damage under the window it paints into (`PaintCache::paint_into`,
`damage::WindowRegions`), and each window gets only its own, clipped to its size
(the whole window when it opens or resizes). A window resizing or animating
leaves the others' paintings untouched. Scroll copies work the same way: each
`paint::Move` names its window, the document settles each window's moves against
that window's painting and damage alone, and each `WindowOutput` carries its own
`moved`.

**Scale.** Each window is drawn at its own pixels per point: a host reports one
with `WindowReport::Rastered` (the main window included), and until it does a
window uses the context's. It is the window's raster, not its layout: every
window is laid out at the context's scale. The document lays out and paints
each window's tree, and everything opened inside it, with
`Context::window_painter(window)`, whose `PainterState::raster` is that window's
pixels per point over the context's, so its text is rasterized for the size it is
drawn at without moving, as a zoomed canvas's is (`Document::viewport_window`
names the window being laid out, since the overlays above a node are out of the
arena then). A change of scale lays nothing out: it repaints the window's tree at
the new raster and damages the window's output whole.

**Window state.** The cursor goes to the window the pointer is over, the IME
area to the window holding the focus, and the rest of `WindowState` stays on the
main window, except the appearance: a `Root` built inside a hosted window sets
that window's clear colour, title and system bars
(`Document::update_appearance(Some(window), ..)`), and a window without one takes
the main window's. A `Root` inside a `Window` that is an overlay sets nothing,
since an overlay has no clear of its own. A window that draws its own rounded or
inset chrome, as the dock's floating windows and window-presented dialogs do,
sets `Color::TRANSPARENT` so its host shows what is behind its corners and
margins.

**Accessibility.** The accessibility tree is one tree, with each window's nodes
in its own coordinates; `FrameOutput::window_accessibility_tree` is the part of
it in one window (its nodes, rooted at the window, with the focus when the focus
is there), and `FrameOutput::window_overflows` names the window of each overflow.

**What a host asks per window.**

- `overlay_rects(window)`: where the overlays opened in it take input, scrims
  included.
- `overlay_painted_rects`: only what they paint: their content, and a scrim that
  dims unless the host draws scrims.
- `overlay_scrims`: the scrims that dim, each a `Scrim` with its colour and
  corner radius. Where `Context::hosts_scrims` (set by the app platform), the
  document paints no scrim and its host draws `overlay_scrims` between what it
  shows under the window and the overlays' content, so the host's own children
  under a dimming overlay are dimmed with the rest rather than hidden.
- `floating_rects(window)`, `overlay_layers(window)`, `drawing_rects(window)`.
- `press_claims(window)`: the nodes in it that claim presses.
- `measure_node(ctx, node, available)`: measures a node with the painter of its
  window.

#### Pointer input across windows

`Incoming::Input` carries a `window` (`Incoming::in_window`; every constructor
starts with the main window), in that window's own coordinates. The intake turns
a change of window into `Event::PointerWindow`, so every `PointerEvent` and
`InputState::pointer` carry the window too.

A host names the window under the pointer even while a press made in another
window holds it, and does not send `PointerLeft` when the pointer only crosses
into another window: the node holding the press goes on getting the drag, and its
`PointerPress` names the window the pointer is over and the position in that
window (`local` is then measured from the holder's rectangle in its own window,
so it means little; a click needs the release in the window the press began in).

A forwarding catcher (an app region, a Wayland window) is handed those events in
its own window's coordinates instead, once the host has reported both windows'
`Moved` origins: the position moves by the difference between the origins and the
event names the catcher's window, so a drag begun on one display goes on smoothly
over the next. Each pointer event a catcher is handed also names the node the
pointer is over (`Forwarded::Pointer`'s `over`, an `Over`: the node and the
pointer's position from that node's corner in the node's window), which is how a
host hands a held press on to the region it is over: `Replay::replay(input, zoom,
aim)` asks `aim` where each pointer event goes:

- `Aim::Own`: this catcher, from its corner.
- `Aim::Other { node, at }`: replayed at `at`, with `Replayed::over` naming the
  node.
- `Aim::Away`: over nothing of its own; while a button is held, `PointerLeft`
  once, then the catcher's own coordinates.

**Dragging out of every window.** The drag board looks for drop targets only in
the window the `DragPoint` names. A host signals that the pointer, holding a
button, is over no window by sending `PointerLeft` (moves and the release then
keep going to the window the press began in, outside its rect). The intake turns
that into `Event::PointerOutside`, which sets `Pointer::outside` until the next
press. While it is set and the pointer is outside its window's reported size, the
board (`Board::set_outside`, each frame) drops nothing: the target the pointer
left hears `over(None)` and the drop calls the `Outside` callback given to
`Board::begin` with the last point (inside or out, so a drag that leaves on its
first move still has one), which `Draggable` passes on as `on_drop_outside`.
Unless the host opens every window (`Context::opens_windows`) the flag is
ignored, so dragging out of a platform window still drops at the last point
inside.

`be_lib_ui_types::Input` has no window: it is the app protocol's input too, and
an app region is always one window.

#### Testing windows

`testing::Stepped::hosting_windows` (be-sdk-test's
`DocumentTest::hosting_windows`) hosts windows with a host that opens every
window asked for at its size and reports `Resized` and `Placed` back the next
frame. `frame_in(window, inputs)` sends input to a window,
`take_window_requests` reads what the program asked, `report_window` reports as
a host would, `windows()` lists the open windows and each one's latest
`WindowOutput`, and `Stepped::snapshot_window(name, id)` takes a window's
painting.

`Stepped::hosted(context, program, window, layer, setup)` runs under a real
window-hosting layer instead, such as `beui_layer_space::Space`, stacked over its
drive layer, with the `Setup` it is given (space wants `GpuSetup` and the
`Context`), so a test can paint the scene that reaches the platform
(`Stepped::painting`, `drive paint in platform`), as be-host-wayland's space test
does.

### Layers

A `beui_runner::Layer` is middleware between a platform and the program's
runner. `Launch::layers` is the stack, the layer nearest the platform first; the
native run functions build it from `BEUI_LAYERS` (`choose_platform` and `stack`
in `crates/beui-host/src/runner.rs`). Input goes up the stack, from the platform
through each layer to the program; the program's output and its calls on the
platform go down it. Every hook defaults to changing nothing:

- `attach(&Context)`, once, as the runner is made: a layer that opens the
  program's windows sets `WindowHosting::Any` here.
- `setup(&Setup)`, after the renderers load and whenever the GPU is replaced:
  `setup.get::<beui_host::native::GpuSetup>()` is the wgpu device, queue and
  surface format, `setup.get::<Context>()` the program's context, and
  `setup.waker` wakes the platform.
- `pace() -> Option<Instant>`, before everything else in a frame: a layer that
  paces frames (an XR headset) blocks for the next one and returns its predicted
  display time, which becomes the frame's time (unless the clock is stopped;
  never earlier than the last). While a layer paces, the runner turns the
  window's vsync off (`Renderer::set_vsync`) and keeps framing while the platform
  window has no size (minimised), at its last size and unseen.
- `now(Instant) -> Instant`, bottom first, before the frame: the frame's time.
  The runner reads the context's clock once (`Context::stop_clock` and
  `advance_clock` stop and step it), and the layers, the program and every later
  hook see the time the stack settled on (`LayerView::now`, `Context::now`).
- `input(&mut Vec<Incoming>, &mut LayerFrame)`, bottom first, with everything
  that arrived for the frame before the program sees it: a layer may rewrite,
  drop or add entries, name each pointer event's window in that window's
  coordinates, and push `WindowReport`s. Right after it the runner asks
  `keyboard_attached()`, top first: the first `Some` overrides whether the
  platform reports a keyboard attached (see The on-screen keyboard); `None` (the
  default) leaves it to the layers below and the platform.
- `output(&mut Shown, &FrameOutput, &mut LayerFrame)`, top first, after the
  program's frame. `Shown` is what the layers above left of the program's
  windows and window requests (the program's own at the top); a layer may read,
  replace or drop them. What reaches the platform is what the bottom layer left:
  `Runner::shown_window(id)` reads it, and the runner hands its main window to
  `show_window` and the renderers. The `FrameOutput` is the program's own, which
  `Runner::output()` keeps.
- `framed(&mut Frame, &FrameOutput, &mut LayerFrame)`, top first, once the frame
  is prepared: a layer may ask for the next frame sooner (`repaint`,
  `repaint_after`) or read whether one is due.
- `wants_sight()` keeps an occluded window's frames drawn; `wants_capture()` asks
  the renderers for the pixels of the frame last presented, which
  `captured(Option<Capture>)` hands over.
- `up(window, at)` and `down(window, at)` map a point in points between the
  windows below the layer and the windows above it, `None` where it reaches no
  window. `LayerFrame::below` maps through the layers under one to the platform
  (`to_platform`, `from_platform`) and `LayerFrame::above` through those over it
  to the program (`to_program`, `from_program`).
- The services: `copy`, `copy_image`, `paste`, `pick_file`, `save_file`,
  `delivers_paste_shortcut`, `shows_keyboard`, `show_window`, `resize`,
  `set_fullscreen` and `publish_accessibility`, each handed the
  `PlatformServices` below it (`LayerFrame::below` is the same thing), which it
  forwards to by default, so a layer may answer a request itself or change it on
  the way down.

`LayerFrame` also carries the `LayerView` (the platform window's size in
physical pixels, the pixels per point and the frame's time), the `Context`, the
`Program` (for `busy`, `drawing`, `environment`, `close_requested` and
`request_painting`), the time the frame started, and `close()`, which ends the
program after the frame as a closed window does. An `Rc<RefCell<L>>` of a layer
is a layer too, so a test or a bench keeps a handle on one it stacked (`InSpace`
in crates/beui-host/src/tests.rs), and two layers that share state through such a
handle can sit at different heights in one stack.

With a layer that opens windows, the main window's size is the one the layer
reports, not the platform window's, and the program gets no safe area. A wgpu
layer draws each window into a texture of its own and composites them through a
`Drawing` (`beui::native::drawing`, whose `Draw::prepare` has the device, queue
and an encoder) in a `display::Layer::Shape(Shape::Drawing { .. })` covering the
window it returns. beui-platform-drm under such a layer shows the main window on
its first display.

### Displays under beui-platform-drm

beui-platform-drm shows no main window and opens only the windows it placed
(`WindowHosting::Placed`): one per display, macOS-like. It hands the program a
`beui_platform_drm::Displays` handle through `Setup` (`setup.get::<Displays>()`):

- `displays()` lists each display on the desktop as a `Display`: its `id` (stable
  while the monitor is plugged in), `name`, `position` and `size` in the
  desktop's points, and its own `scale` (pixels per point). Each `size` is the
  physical mode over the scale.
- `display(id)` is one of them.
- `watch(f)` hears every change (a display plugged in or out, a new mode,
  rotation or scale) before the frame it reaches lays out.
- `placement(id)` is the `WindowPlacement` to open that display's `Window` with.

`Displays::new` makes a handle with displays of a test's own, and `Displays::set`
changes them.

The program opens a `Window` per display at the display's `size`, so it lays out
at the right size in the frame that first shows it; the platform matches the
window's `Open` to the display in that same frame and draws its output there, and
reports the window's `Resized`, `Moved` (the display's position) and `Scaled`
(the display's scale; a window opened on a display at another scale than the
first is drawn crisp from its second frame). It reports no placement for any
window, so a dock never reopens one, and any other `Window` is an overlay in the
window it was opened in. A display with no window shows its clear colour. The
main window is the first display's size at its scale, which is also the
context's.

**Arrangement.** `DisplayConfig` arranges the displays (`plan.rs`, then
`layout::arrange`):

- `uses` puts each on the desktop (the default), shows another display's picture
  on it (`DisplayUse::Mirror`: the platform draws that display's window on it
  too, at the scale that fits it, from its top-left corner), or turns it off
  (darkened as blanked screens are, keeping its connector and CRTC). A display
  mirroring one that is not on the desktop is on the desktop itself, and with
  none left on the desktop the main one is (or else the first found), so a
  session always has one. Mirrors and displays turned off are not in
  `displays()` and take no pointer.
- `main` names the first display, which `displays()` lists first, then the
  others on the desktop in the order they were found.
- `places` puts each display's top-left corner in the desktop's points.
  be-lib-geometry's `touching::arrange` keeps it there while it shares an edge
  with a display placed before it and overlaps none, and otherwise moves it to
  the nearest place that does (`settle`); a display with no place goes right of
  the rest along their bottom edges. The arrangement is then moved so its
  top-left corner is the origin.
- `scales`: each display's scale is what this names for its monitor id, else
  `BEUI_SCALE`, else one that suits the display's physical size and its mode
  (`density.rs`), which `Monitor::default_scale` reports. `Monitor`s list every
  connected display, those off and mirroring included, in the same order.

Pointer input goes to the window of the display under the pointer, in that
window's points (`Units::Points`), and a press focuses the window it lands in
(`WindowReport::Focused`); a press held in one window goes on as the pointer
crosses into another, by the rules in Pointer input across windows.

### Space: the VR simulator layer

`crates/beui-layer-space` is a `Layer` that shows a program's windows as panels
in a first-person 3D scene: a VR simulator that runs on any wgpu adapter, and
the scene a headset shows (see Headsets). `BEUI_LAYERS=space` stacks it over any
native or headless (`:dev`) run. It opens every window (`WindowHosting::Any`,
`Context::opens_windows`), so every `Window` is a real window here; rewrites the
platform's input in `input`; composites the windows into the one main window it
leaves in `Shown`; asks for frames while it animates in `framed`; and maps points
between the platform window and its panels (`up` through the ray at a point,
`down` by projecting a panel's point). Its inherent `Space::input` and
`Space::frame` take a `LayerView` directly, which its tests call. No platform
knows about it, and all its 3D (`glam`) stays in the crate.

**Panels and windows**

- Panels: `Space::panels` is a list of `Panel { window, pose, size,
  metres_per_point, bend }` keyed by `WindowId` (`Panel::new` starts flat at
  1 / `POINTS_PER_METRE`). A panel is `size * metres_per_point` metres
  (`Panel::metres`); `bend` is the angle its width wraps round a vertical
  cylinder whose axis is in front of it, so its sides come toward the viewer (0
  is flat, `MOST_BEND` half a cylinder). `Panel::world`, `offset` and
  `intersect` work on that surface, flat or bent, and on its extension past the
  window's edges. `frame_at(pos)` is the surface's point and turn at a window
  point, and `anchor(pos, frame)` places a reshaped panel so that point stays
  put, which keeps a resized or scaled bent panel's fixed corner in place. On
  the first frame the main window's panel opens at the platform window's size in
  points, at eye height straight ahead, far enough to fill 80% of the view's
  width, and space reports that size (`WindowReport::Resized`).
- Windows: `Space::request` handles each frame's `WindowRequest`s.
  - `Open` adds a panel at the size asked for and reports `Resized` and
    `Placed` the next frame. Placements: `Host` decodes pose, scale and bend
    (`Panel::placed`; one another host wrote, or one it cannot read, opens at
    the pointer instead). `Over { window, at }` puts its top-left corner on that
    panel's point `at`, at that panel's scale, facing the same way,
    `OVER_NEARER` (3 cm) in front of it plus another `OVER_NEARER` for each
    panel already there. `AtPointer` puts the window's point `OPEN_GRAB`
    (24, 16) on the cursor's ray, `OPEN_NEARER` in front of the panel the ray
    hits or `OPEN_DISTANCE` (1 m) away on a miss, turned to face the viewer
    (`Panel::face`, see Carry).
  - A window that opens takes the focus (see Focus). One opened again keeps its
    panel and leaves the focus alone; it takes its new role and size, and moves
    where an `Over` placement puts it (otherwise keeping its top-left corner).
  - `WindowRole::Keyboard` (`Space::keyboards`) ignores its placement and opens
    as `Over` would on the panel holding the focused field (the window whose
    output carries the IME area), centred `KEYBOARD_GAP` (12) points below the
    field, or over the bottom of the focused panel when there is no field. It
    never takes the focus, however it is opened, pressed or carried, so its keys
    type into the field; a hand's grip carries it like any panel.
  - `WindowRole::Popup` opens where its placement says and never takes the
    focus, so its owner keeps it (a Wayland popup's grab ends when its toplevel
    loses the keyboard). A mouse press or trigger pull anywhere but on a popup or
    a popup placed `Over` it (chrome included) closes it, reporting `Closed`,
    and goes no further, nor does its release (`Space::dismiss_popups`).
  - `Resize` sets the size, keeping the top-left corner, and reports `Resized`
    the next frame. `Close` removes the panel and its texture; the main
    window's panel does not close.
- Focus: focus follows clicks, not the cursor, as on a desktop
  (`Space::focused`, the main window's panel at first). A press on a panel's
  content or chrome focuses it, as do starting to carry it and opening it, so a
  window popped out of the dock gets the keys at once; a press on no panel and
  no chrome focuses none, and then key presses and text reach no window.
  Windows reopened at saved placements at startup open in the order the program
  asks, so the last one has the focus. A change is reported as
  `WindowReport::Focused` (old window `false`, new `true`), so the program's
  document lets go of, and later restores, the keyboard focus inside that window
  (a window just opened gets its first focusable, or the dock's tab, see
  Windows and window hosts). Panels have no stacking order, only depth, and only
  a carry moves one: a `WindowRequest::Raise` for an unfocused panel that
  another hides even in part (`Space::hidden`) flashes its outline instead,
  drawn through everything and fading over `FLASH_SECONDS` (1 s). Closing the
  focused panel focuses the main one.

**Input**

- Grab: while its window has focus the host holds the pointer
  (`Space::grabbed`, asking for `pointer_locked` all that time), so the mouse
  sends only `PointerMotion`. Losing focus (`Input::Focus(false)`), the platform
  dropping the lock (`Input::PointerUnlocked`: a browser on Escape) or holding
  Escape for `hold::LET_GO_AFTER` (0.6 s, frame clock) lets it go; the next
  press into the window grabs it again and reaches nothing, nor does its
  release. Escape's press reaches the program at once, since programs use a tap
  of it; space asks for a frame at the deadline (`repaint_after`), and once the
  hold lets go Escape's repeats stop reaching the program but its release still
  does, so the program's key state stays whole. While a window under the cursor
  asks for pointer lock itself, motion goes to it and the cursor stays put.
- Pointer: the cursor is a point in the world (`Space::cursor`). Every press,
  move and hover goes where the ray from the eye through the cursor as drawn
  (`Space::cursor_ray`, `cursor_screen`) meets the nearest panel's front
  (`Space::pick`), and the program gets it in that window's points (on a bent
  panel, the point of the curved surface). A press's own position is ignored.
  An absolute position (a `PointerMoved` in pixels, or in a window's points from
  a driver, mapped onto that panel) puts the cursor where its ray hits.
  `PointerMotion` swings the ray about the eye around an up (`Head::swing`):
  motion across turns it about that up, keeping its height, and motion up raises
  it toward that up, short of `head::MOST_ELEVATION`, at a fixed angle per pixel
  (`Head::pixels_per_radian`).
  - Which up is a temporary setting while the right rule is found
    (`tuning::Tuning`, one rule for the simulator and one for a headset, both
    the sky by default; `tuning::Up`): the sky (gravity-aligned, which
    degenerates looking straight down), the view's up (rolls with the head,
    works at any pitch), or a blend from the sky to the view's up as the head
    pitches between two angles. Ctrl+Alt+M cycles the rule for where space is
    running, Ctrl+Alt+Left/Right moves the blend's start and Ctrl+Alt+Down/Up its
    end by 5°, none reaching the program; each change shows the rule in a label
    beside the cursor for `tuning::SHOWN_FOR`.
  - The cursor sits `CURSOR_LIFT` (1%) of the distance in front of what it hits,
    or on a miss at the last hit's distance (`OPEN_DISTANCE` before any hit).
    Turning the head and walking leave it at its point, and nothing holds it
    inside the view: it can go anywhere around the viewer, still points and
    clicks where it is, and turning or moving the mouse back finds it. During a
    carry it stays on the grabbed point.
  - Moving onto another panel just names the new window (no `PointerLeft`), so a
    drag goes on across windows. Moving off every panel sends the last window
    `PointerLeft`; while a button is held, moves and the release go to the window
    the press began in, at the point the ray meets that panel's plane outside
    its rectangle, which is how a program tells a drop on nothing (the dock
    floats a tab dropped there into a window opened `AtPointer`). Presses that
    miss reach nothing.
- Head: holding Ctrl and Shift (either side) steers. Ctrl or Shift alone reaches
  the program; when the other joins it, the program sees both released and its
  modifiers cleared. While steering, `PointerMotion` turns the head (yaw about
  the vertical, pitch clamped short of straight up or down; with the head rolled
  the motion is first turned by the roll into the level frame, so the view turns
  along the screen while yaw stays vertical and the roll stays put), W, A, S and
  D by their place walk along the floor relative to where the head faces, and Q
  and E roll it (`Head::roll`, `ROLL_PER_SECOND`), all timed from
  `LayerView::now`. `Head::rotation` is yaw, then pitch, then roll about the
  view direction; the view, cursor rays and `scale` go through it, the cursor's
  motion does not (see Pointer). No key pressed while steering reaches the
  program, nor its release, and clicks reach it without Ctrl or Shift, so a
  program in space never sees a Ctrl+Shift chord. `drive`'s keys, modifiers and
  `PointerMotion` follow the same rule (its motion is in points, scaled to
  pixels first): `drive --into=space keydown ctrl shift w` walks, and
  `drive --into=space motion 200 0` with Ctrl and Shift held turns the head.
- Pointers: the mouse and each controller's laser are pointers of their own
  (`space::Pointer`: `Mouse` or `Hand(Hand)`), each holding its own carry,
  resize, chrome gesture, hovered and captured window (`space::Held`:
  `Space::mouse`, and `held` on each `Space::lasers` entry), so both hands and
  the mouse can each carry, resize, reshape or press at once. A
  `WindowRequest::BeginMove` or `BeginResize` goes to the laser whose touch is
  down in that window, else to the mouse. Two pointers do not hold one panel,
  but both hands' grips together scale and turn it (see Lasers).
- Lasers: controllers reach space as `Incoming::Controller`
  (`beui_runner::Controller`, per `Hand::Left` or `Right`): `Pose { aim, grip }`
  (`HandPose`, a position and an `xyzw` orientation whose -Z is where it points,
  in the views' world, floor at y = 0), `Trigger` and `Squeeze` (0 to 1; press
  at `laser::PRESS` (0.6), release at `laser::LET_GO` (0.4)), `Lost` (its touch
  is cancelled and what it held let go), and `Toward { window, at }`, which a
  driver uses to aim a hand at a window's point (from `Laser::resting` beside
  the head before it has a pose). An XR host sends pose, trigger and squeeze
  each frame.
  - A laser (`laser::Laser`) is a ray from its aim pose, smoothed by a One-Euro
    filter (`laser::OneEuro`, `MIN_CUTOFF`, `BETA`, `DERIVATIVE_CUTOFF`, timed
    from `LayerView::now`; the last pose is fed again each frame, with frames
    asked for, until a still hand settles); a change of more than `laser::JUMP`
    (0.15 rad or m) is taken at once.
  - It acts as a finger, not a hovering mouse: nothing reaches the program until
    the trigger presses, which sends `Input::Touch` (a `TouchId` with
    `laser::DEVICE`, finger 0 for the left hand, 1 for the right) `Start` where
    the ray hits the nearest panel, focusing it (a press on nothing focuses
    none), then `Move` and `End`, all to the window it began in (off its panel,
    where the ray meets the extended surface). So beui's touch handling applies:
    a drag scrolls, a long press opens the context menu, two lasers on one panel
    pinch. The press freezes the ray (`Laser::frozen`) until the laser turns more
    than `laser::FREEZE` (0.02 rad) from it, so the jolt of pulling the trigger
    moves nothing. A trigger press on chrome works it as a mouse press does.
  - Squeezing the grip with the laser on a panel or its chrome carries the panel
    along the laser (`Carry::along`, `Carry::follow` each frame, the grabbed
    point at the distance it was grabbed) with the mouse carry's turn to face
    the viewer, until the grip lets go; a carry a laser's touch began (a title
    bar's `BeginMove`) ends with the trigger, and the program gets the `End`
    where it grabbed. Squeezing the other grip on the carried panel holds it with
    both (`Space::both`, `Both`): it moves and turns with the line between the
    grips (`Quat::from_rotation_arc`) and scales about its centre with their
    distance (`metres_per_point`, between `chrome::SMALLEST_SCALE` and
    `LARGEST_SCALE` of where it began), until either grip lets go, which reports
    `Placed` and leaves neither hand holding it.
  - Drawing: each laser (`Space::beams`, `render::Beam`) is depth tested after
    the chrome, a strip `BEAM_WIDTH` (4 mm) wide from the hand to where it hits a
    panel or chrome, or `laser::REACH` (5 m) on a miss, faint at the hand, blue,
    white while pressed, with a dot `DOT` times its distance from the eye across
    the hit; the paintings draw them over the panels. Outlines
    (`Space::outlines`, `render::Outline`: a carry's ghost and a raise's flash)
    are drawn after the beams with no depth test, so they show through every
    panel: a faint `OUTLINE` fill and an edge `OUTLINE_WIDTH` times its distance
    from the eye.
- Keyboard: while any laser has a ray, `Space::keyboard_attached` (its
  `Layer::keyboard_attached`) says no keyboard is attached, so tapping a field
  opens beui's on-screen keyboard as a panel (see Windows); with only mouse and
  keyboard it says nothing and no panel opens.

**Moving and reshaping panels**

- Carry: on `WindowRequest::BeginMove` (any window) the panel keeps its pose
  relative to the head, with the grabbed point on the cursor's ray, so moving
  the mouse or steering carries it. As the mouse swings the ray the panel turns
  by the ray's change of yaw about the head's up (`Carry::aim`); steering turns
  it with the head. From the grab it also turns over `TURN_SECONDS` of
  `LayerView::now` (smoothstep, `carry::Turn`) to face the viewer and then keeps
  facing them, so letting go turns it no further. Facing (`carry::facing`) points
  the panel's normal through its centre at the eye with its up along the head's
  up (`Head::up`), keeping the grabbed point on the ray, so the panel shows
  upright to the view however the head is rolled or pitched.
  - Distance: the grabbed point stays at `Carry::distance` from the eye. Scroll
    changes it by `STEP_PER_LINE` (0.95, about 5%) per line, a touchpad's pixels
    by their share of a line (`SCROLL_LINE` points), between `carry::NEAREST`
    and `FARTHEST`. The scroll does not reach the program, which sees no pointer
    moves during a carry.
  - Contact (`Carry::settle`, `contact.rs`): a carried panel does not pass
    through another. Each frame it moves along its carry's ray (the eye's or the
    laser's) to the distance nearest the one asked for at which it stays on its
    side of every panel it overlaps, `contact::CLEARANCE` (12 mm) clear, and
    turns to face the viewer there. `contact::span` finds the distances along
    the ray at which two panels (and their chrome) come that close, continuous
    as a panel slides off another. The side (`carry::Side`, `Carry::side`) is decided
    when an overlap begins and kept until it ends, so a panel popped out in front
    stays in front. A push changes it: resting against another panel, a scroll
    (or laser move) farther into it is held as `push` lines, drawn as a ghost
    outline (`Carry::ghost`) where it is being pushed to, and once the push
    reaches `PUSH_LINES` (2) and the ghost is clear of the other panel the side
    flips. Backing off takes the push back first; letting go or sliding off
    drops it. Pulling nearer through a panel in front works the same way.
    `Popup` and `Overlay` windows take no part (`contact::collides`), and a
    two-hand hold is not stopped.
  - Start and end: a program's `BeginMove` or `BeginResize` starts a carry or
    resize only while a press it was given is held (any window's, since one
    program's drag can open and carry another window; for a laser, while its
    trigger holds that window), as Wayland ties a move to its press. A request
    arriving after that press was let go is dropped (see
    [the decision](../decisions/beui_space_move_needs_a_held_press.md)). Letting
    go of the button that was down when it began (for a carry begun with none,
    as `Space::begin_carry` can, the next click) ends it: the program gets the
    release in the window that took the press, where it grabbed when that is the
    carried one; the turn finishes at once (`Carry::finish`) and the panel is
    placed once more; and the host reports `WindowReport::Placed` with the
    placement `space X Y Z QX QY QZ QW S B` (position, rotation,
    `metres_per_point`, `bend`; `Panel::placement`, read back by
    `Panel::placed`). The program keeps it in memory (the dock does) and hands it
    back when it opens the window again; placements are not kept across
    restarts (plans/vr.md).
- Resize: on `WindowRequest::BeginResize { window, edge }`, or a drag on a side
  of a panel's chrome (`resize::Resize`), the edge or corner follows where the
  cursor's ray meets the panel's surface as it was, while the opposite one stays
  put, no smaller than `resize::SMALLEST`, stopping where the panel would touch
  a panel it was clear of (`contact::newly_touching`), as the chrome's scale,
  turn, roll and bend drags do too. Each size is reported as `Resized` at once,
  so the program lays out at it the same frame; the program sees no moves
  meanwhile, and the carry's rule for letting go ends it with a `Placed`.
- Chrome (`chrome.rs`): controls the host draws and handles around every panel
  of a `Normal` or `Keyboard` window (`Panel::framed`; popups and overlays have
  none):
  - Parts: an L bracket just outside each corner; the window's title (its
    `WindowState` appearance title, set by the `Window` component's `title`) in
    a pill centred above the top edge, or in its place accent text on an accent
    pill showing the window's `notice` (the `Window` component's `notice`, which
    the dock sets from `DockTabControl::show_notice`; `chrome::Title`), cut with
    an ellipsis to fit between the top brackets; and under the bottom edge a pill
    with move (`ICON_MOVE`), rotate (`ICON_ROTATE_3D`), roll
    (`ICON_ROTATE_CW_SQUARE`) and curvature (`ICON_SPLINE`) buttons.
  - Size: they keep one size to the eye, as the cursor does. Their lengths
    (bracket arm `ARM` 22.5 mm, `THICKNESS` 3.5 mm, `GAP` 6 mm out; the pill
    `BAR` 20 mm tall, `BAR_GAP` 8 mm below, `BUTTON` 16 mm) are metres at
    `REFERENCE` (0.6 m) times `Fit::scale`, the eye's distance to the panel's
    centre over `REFERENCE` (`chrome::fitted`), which `Space` sets on each panel
    (`Panel::chrome`) every input and frame, changing it only once it is off by
    more than `KEPT` (5%), so small head moves do not repaint the chrome.
    `Fit::title` is the title's width at that reference, measured with the
    program's `Context` when the title changes.
  - Look: faint (`FAINT`) except on the focused panel, which keeps them lit
    until the focus moves. Each panel's chrome is a display list
    (`chrome::output`, icons laid out with the program's `Context` from `Setup`)
    drawn into its own texture only when its `Look` changes, at the pixels per
    metre its window's scale gives the window (`chrome::pixels_per_point`, at
    most `LARGEST_TEXTURE` texels a side), icons rasterised at that
    (`Context::rastered`), on the extended surface `LIFT` in front of the panel,
    after every panel.
  - Hits: `chrome::layout` places the parts and `chrome::part_at` finds one at
    a window point outside the window's rectangle. Hit areas leave no gap next
    to the window: each bracket's arms take the band from the window's edge out
    to the bracket (and `SLOP` beyond), the sides between brackets take that
    band too, and the buttons, slider and title take everything between them and
    the edge. Picking is by depth alone: the ray takes the chrome on a panel's
    extended surface when it is nearer than any panel (`chrome::pick`), focused
    or not, so a popped-out window's chrome takes the click over content behind
    it. A driver's point outside its window picks too. Input on chrome does not
    reach the program (the window under it last gets `PointerLeft`), the cursor
    shows what a part does, and while a program holds a button chrome is
    ignored.
  - Gestures (`Gesture`): dragging a side between brackets (`Part::Resize`, lit
    while hovered or held) resizes from that edge as `BeginResize` would
    (`Gesture::Resize`), the main window too: its size in space is its logical
    size, reported as `Resized`, while the platform window keeps its own and
    stays the mirror. Dragging the title, or pressing the move button, starts
    the carry a title bar's `BeginMove` starts (`Space::begin_carry`,
    `Gesture::Move`), ended by letting go. Dragging a bracket scales the panel
    (`Gesture::Scale`): `metres_per_point` follows how far the cursor goes along
    the diagonal from the opposite corner, which stays put, between
    `SMALLEST_SCALE` and `LARGEST_SCALE` of where it began, so the window keeps
    its points and aspect and gets no `Resized`.
  - Rotate button (`Gesture::Turn`, `chrome::Sight`, `chrome::turned`): a
    trackball about the panel's centre as seen from the eye. The aim's change
    since the drag began, measured on the screen times `TURN_GAIN` (2), turns
    the panel about the sight's up (the head's up made perpendicular to the
    line from the eye to the centre) and tilts it about the sight's right, tilt
    first, so the nearer edge follows the drag on screen, a level edge stays
    level, and the panel does not roll however the cursor circles. With the mouse,
    `PointerMotion` swings the gesture's own aim (`Gesture::Turn::aim`) while the
    cursor is put back on the button (`grab`), so it lets go on the button; an
    absolute position (an ungrabbed platform pointer, or a driver) aims directly,
    and a laser's turn follows its ray.
  - Roll button (`Gesture::Roll`, `chrome::roll_angle`, `chrome::rolled`): rolls
    the panel about its normal through its centre by the change in the cursor's
    angle around the centre on the surface as it was, like a dial's knob.
  - Curvature button: opens a slider in the pill (`Chrome::slider`); clicking or
    dragging along it sets `bend` from 0 to `MOST_BEND` about the centre.
  - Scale, roll and slider drags map the ray onto the panel as it was when they
    began; scale, turn, roll and slider drags report `Placed` when let go. A
    later carry turns the panel back to face the viewer (`carry::facing`),
    overriding rotation and roll.

**Drawing**

- Views: the scene is drawn through a list of `head::View { position,
  rotation, fov, size }`: a pose, an off-axis `head::Fov` (from its edges'
  tangents, `Fov::tangents`, or OpenXR's angles, `Fov::from_angles`;
  `Fov::projection` is right-handed with depth 0 to 1 between `NEAR` and `FAR`)
  and the target's size in pixels. `Space::views` is the pose source's: the
  simulator's one view (`Head::view`, the head over the platform window, a
  symmetric `FOV_Y`), or the tracked `head::Views { display_time, views }` an XR
  host hands `Space::set_views` each frame (`None` goes back to the simulator),
  in world space with the floor at y = 0. Tracked views move `Space::head` to
  their centre (`Head::posed`: mean position, averaged rotation), so cursor
  rays, carry, chrome and the mirror window (that centre eye at `FOV_Y`) work
  off the centre eye. `Space::draw(encoder, targets)` draws each
  `ViewTarget { view, texture, format }` (a 2D texture view of the view's size,
  such as an XR swapchain image) through its own view, a 4x multisampled pass
  resolved into the texture, after painting any changed window; the platform
  window is drawn by the `Drawing` that `Space::frame` returns.
- Sharpness (`scale`): each frame `Space` gives every panel the most pixels per
  point any view shows it at (`scale::wanted`): per view, the view's pixels per
  point at the frustum-clipped panel's nearest depth (its outline sampled along
  the top and bottom edges, so a bent panel's nearer sides count; a panel scaled
  up by its chrome gets more; `Fov::pixels_per_tangent` is the larger of across
  and down), times `SUPERSAMPLE` (1.25, so the main panel's first 80% view is
  drawn at exactly the platform's scale), rounded up to `STEPS_PER_DOUBLING` (4)
  steps per doubling, between `LEAST_SCALE` (0.5) and `MOST_SCALE` (4) and no
  larger than `LARGEST_TEXTURE` (4096) texels a side. A panel keeps its scale
  until it needs more or `KEPT_STEPS` (2) steps less, and one out of sight keeps
  its own. A new scale waits until wanted for `SETTLE` (250 ms of
  `LayerView::now`, with a frame asked for when the wait ends), and a panel being
  carried, resized or reshaped keeps its scale until the gesture ends, so walking
  about redraws a window only now and then and a scale does not change every
  frame. A new window takes its scale at once. A change is reported as
  `WindowReport::Rastered` (`Space::scale` reads it), only the window's raster:
  the program repaints at it without laying out, so text only gets sharper or
  softer. An `Embed` in the window hears it (`EmbedPlacement::raster`); be-host
  sends an app region its raster beside its scale (the protocol's `raster`,
  `ViewPlace::raster`), and be-sdk-beui-platform reports it to the region's
  window as `WindowReport::Rastered`, so app content is as sharp as the window
  without relayout.
- Panels: each window's display is drawn by its own
  `beui_renderer_wgpu::Renderer`, over the compositor's one `Resources`, into a
  mipmapped texture at its output's `pixels_per_point`, only when its output
  changed, repainting only its damage after copying its `moved` scroll inside
  the texture. A window that changes size gets a new texture but keeps its
  renderer ([decision](../decisions/beui_space_keeps_one_renderer_per_texture.md)).
  Textures are sampled with trilinear and anisotropic filtering and a
  level-of-detail bias of -0.5 (`SHARPEN` in space.wgsl), so a texture up to 1.4
  times the panel's on-screen size is read from its sharpest level. A panel is a
  strip of `render::SEGMENTS` columns bent in the vertex shader.
- Scene: a 4x multisampled pass draws the backdrop, the floor grid and the
  panels, and the `Drawing` copies it texel for texel into the platform window.
  A frame is composed again only when the head, a panel, the cursor or a
  window's output changed. The backdrop (`floor.rs`, `background_fragment`)
  fills the view with no depth, coloured by each pixel's direction alone: `SKY`
  above the horizon, `GROUND` below, blended by `floor::ground_share` (the
  floor's `floor::fade` at the distance a ray from `EYE_HEIGHT` would meet it),
  so the floor is part of the sky, not a surface. Only the grid lines are 3D, on
  y = 0 around the eye, faded with distance like the floor colour and blended
  over the backdrop with no depth test or write before any panel, so a panel
  under the floor is drawn over them and stays visible and pickable. Panels are
  depth-tested (backs grey; nearer panels hide farther ones, a carried one too).
- Paintings (`picture.rs`, `Paints`, what the crate's painting tests compare):
  the `Drawing` also paints the backdrop as a 16x16 mesh of the screen with each
  vertex's direction's colour, the grid lines within 20 m as thin quads (the
  GPU's go on to `floor::EXTENT`), then each panel's and its chrome's paintings
  placed onto the projected surface cell by cell (`picture::scene`). Cells are
  ordered by depth per pixel, as the GPU's depth test does: two overlapping
  flat cells are ordered where their planes' depth difference keeps one sign
  over the overlap (checked at its corners); where panels pass through each
  other or pairs cycle, the screen is split into quarters on the pixel grid
  until each is decided or one pixel. A tile drops everything under the last
  opaque cell covering it, so a scene with no intersections paints as one
  tile.
- Cursor: while space holds the pointer (Grab) or a program locks it
  (`Space::cursor_shown`), the host asks the platform for `CursorIcon::None` and
  draws its own in the scene, last and over everything, as a quad facing the
  head with its hotspot on the cursor's point (`Space::cursor_at`). Once the
  grab lets go it draws none and asks the platform's cursor for the icon, since
  the platform then moves the pointer.
  - Away: when the cursor's point is off the screen or behind the eye it draws a
    marker inside the edge instead (`Space::cursor_away`, `cursor::Look::Away`):
    `beui_core::icons::ICON_MOUSE` with an `ICON_ARROW_UP` turned toward the
    cursor, on the line from the view's middle toward where the cursor projects,
    `cursor::AWAY_INSET` (36) points inside the edge, just in front of what it
    hits there; the arrow's angle is kept in `AWAY_STEPS` (90) steps a turn so
    the sprite repaints only when it moves a step. It goes only when the pointer
    leaves the platform window.
  - Its size grows with distance (`Space::cursor_scale`, metres per point) so it
    covers as many pixels as it has points at any depth. It shows the icon the
    window under it, holding the press, or being carried or resized asks for
    (`Space::cursor_icon`): a resize, grab or pointing hand over chrome, an arrow
    over nothing.
  - `cursor::Sprite` paints the icon with `beui_core::drawn_cursor::paint` (the
    drawing the inspector's mouse simulation shows on a touchscreen) through a
    `Context` of its own with only the icon font. The compositor draws it into a
    texture like a window, under a `WindowId` of its own, only when its look
    (icon or marker) or scale changes; a new icon repaints into the same texture
    with the same renderer. The paintings draw its last output over everything
    (`render::Frame::sprite`).

**Headsets**

`BEUI_LAYERS=xr`, in space's place, shows space on an OpenXR headset through
`crates/beui-layer-xr` and waits for one when there is none; `BEUI_LAYERS=space`
is the simulator. `XrSpace` wraps `Space` as the layer. It builds on Linux and
Windows; elsewhere the `Link` fails at once and space stays the simulator.

- Finding a headset: `beui::window` makes a `Link` (`Link::new(program name)`:
  `RunOptions::program_id`, else the program's name, for the runtime's
  dashboard), which tries once at once and then on its own thread every `RETRY`
  (2 s): load the runtime and make an instance (`Runtime::open`), then ask for a
  headset (`xrGetSystem`), keeping the instance while the answer is
  `FORM_FACTOR_UNAVAILABLE`, so SteamVR started later or a headset plugged in
  later are found without restarting SteamVR. `Link::status()` is `Found`,
  `Missing(why)` or `Failed(why)` (the runtime could not make or share the GPU;
  it then stops looking); each change wakes the loop through the `Waker` the
  layer hands it in `setup`.
- Loading the runtime: through the `openxr` crate's `loaded` feature, the
  system's `openxr_loader` if there is one, else the active runtime's own
  library, found as the Khronos loader finds it (`XR_RUNTIME_JSON`, then
  `active_runtime.json` under the XDG config directories on Linux, the
  `ActiveRuntime` registry value on Windows) and negotiated with directly
  (`runtime.rs`), since Windows has no system loader and SteamVR ships none.
- One GPU device: the runtime picks the GPU, so the `Link` is the renderers'
  `OpenGpu` (`beui_renderer_wgpu::present`, `Link::open_device`). `OpenDevice {
  extensions, gpu }` says how a renderer opens its device: the Vulkan device
  extensions it wants where the adapter has them (be-host-dmabuf's
  `DEVICE_EXTENSIONS` on Linux and be-host-win32-capture's on Windows, which
  be-host passes, enabled through `open_vulkan_device`) and, when set, the
  `OpenGpu` that makes the instance and may make the adapter and device (`None`
  leaves them to wgpu). `Xr` makes the VkInstance and VkDevice through
  `XR_KHR_vulkan_enable2` with the extensions wgpu wants (`desired_extensions`,
  `required_device_extensions`) plus those, and hands them to wgpu
  (`Instance::from_raw`, `expose_adapter`, `device_from_raw`), so the window, the
  panels, dmabufs and the headset share one device; it then starts the session.
  The `Link` opens the device through the `Xr` it found, else a plain one, and
  reports `OpenGpu::stale` once it has found an `Xr` the device was not opened
  through. A renderer whose `OpenDevice` is stale reopens its device as after a
  lost GPU (`Renderer::recover`, `Gpu::reopen`, which asks the `OpenGpu` for a
  fresh instance); the runner hands the new device to the program
  (`Program::renderer_replaced`) and the layers, and `XrSpace::setup` takes a
  `Headset` for `Link::opened()`.
- Without a headset (none, or its session lost or not yet running), `XrSpace`
  hands space a `notice::Notice` ("Waiting for a headset" and why) drawn over
  the mirror window (`Space::set_notice`), and space is the simulator: head at
  eye height looking at the main panel, mouse and keyboard on the panels, no
  pacing and no polling. The mirror keeps the simulator's head where the headset
  left it.
- Session loss (`Headset::lost()`): `Lost::Session` (`LOSS_PENDING`, headset
  unplugged) reopens on the same device once `xrGetSystem` finds the headset,
  tried every `RETRY` (`Headset::reopen`). `Lost::Instance` (the runtime quit:
  an instance loss event, `ERROR_INSTANCE_LOST`, or `xrGetSystem` failing
  otherwise) drops the headset, session and instance (the device holds its `Xr`
  only weakly, since the Khronos loader allows one instance at a time) and sends
  the `Link` looking again (`Link::lost`), reopening the device through the next
  instance. `Lost::Ended` (`EXITING`, or swapchains or a session that could not
  be made) leaves the simulator for the rest of the run, so a runtime that
  closes the program is not answered by opening it again.
- Frames: the headset paces them. `Layer::pace`, which the runner calls before
  each frame, blocks in `xrWaitFrame` and returns the predicted display time,
  which becomes `LayerView::now` and the frame clock (unless the clock is
  stopped; never earlier than the last). While a layer paces, the runner turns
  the window's vsync off (`Renderer::set_vsync`, `AutoNoVsync`), the layer asks
  for the next frame at once, and the runner keeps framing it while the platform
  window has no size (minimised), at the window's last size, unseen. Input
  locates the eyes in a `LOCAL_FLOOR` space (`STAGE`, else `LOCAL` lowered by eye
  height) for `Space::set_views`; just before drawing the frame locates them
  again for the same display time (`Headset::relocate`, which the projection
  layer then names) and hands those over, draws them into each eye's swapchain
  image (one swapchain per eye, sRGB where offered, imported with
  `create_texture_from_hal`), submits, releases the images and ends the frame
  with a projection layer. A frame begun and not submitted is ended empty at the
  next `pace`. While the session is idle the layer polls the runtime every
  `IDLE_POLL` (100 ms).
- Composition layers: panels whose order to the eye is plain are also shown as
  OpenXR composition layers, sampled once by the runtime's compositor, so text
  stays sharp. `Space::layers(&layers::Allowance { most, cylinders, largest })`
  picks them (`layers::choose`): panels facing the eye, not held by a carry,
  resize or gesture, no bigger than the runtime's swapchains, bent ones only
  where `XR_KHR_composition_layer_cylinder` is enabled, those that look biggest
  first, up to the runtime's layer count less the projection layer (and
  `MOST_LAYERS`, 8, for memory), and none whose order to another chosen one is
  unclear: two that can overlap on screen must lie wholly behind or in front of
  each other's planes, else the second stays in the eye buffer. Each
  `PanelLayer` (pose, metres, bend, texture pixels and format, and a `version`
  the compositor bumps on each repaint) becomes a `beui_layer_xr::Quad`.
  `Headset::submit` keeps a swapchain per layered window (dropped three frames
  after it stops being layered, once the GPU is done with it) and acquires an
  image only when its version changed, so an idle panel costs no copy.
  `Space::draw`'s `LayerTarget`s have the compositor copy the panel's texture in
  (`fill`) and draw each layered panel's front as a hole, cleared to transparent
  with depth, so nearer panels, chrome, beams and the cursor still cover it. The
  layers go under the projection layer, farthest first, as quads or cylinders
  (posed at the cylinder's axis, `central_angle` = bend), and the projection
  layer blends by its alpha (`BLEND_TEXTURE_SOURCE_ALPHA`). The mirror window
  draws every panel itself. `Headset::layered` counts the last frame's layers.
- Mouse and keyboard work as in the simulator: the mirror window keeps the
  keyboard focus and the pointer, and the cursor's screen is the mirror (the
  centre eye at `FOV_Y`, inside a headset's field, so the away marker does not
  sit in the periphery). Tracked views are in the tracking space, and
  `Space::body` (`head::Body { position, yaw }`) places it in the world
  (`Views::placed`, and each hand's poses through `Body::place`). While views are
  tracked, Ctrl+Shift steering moves the body instead of the head: mouse motion
  across turns it about the eyes (motion up and down, Q and E do nothing), and
  W, A, S and D walk it the way the head faces. The body stays while the headset
  is away.
- Controllers come from an action set (`controllers.rs`) attached when the
  session starts: per hand, the aim and grip poses, trigger and squeeze, bound
  for the simple controller, Index, Touch, Vive and Windows Mixed Reality
  profiles, read at the predicted display time in the eyes' space and pushed into
  the frame's input as `Incoming::Controller` (see Lasers), with `Lost` once when
  a hand's aim stops being tracked.
- Tests: `beui-layer-xr`'s tests run Monado's service with its null compositor
  on lavapipe (`//buck/sysroot:amd64-xr`) and have it show frames, the main
  panel as a quad layer; one starts the `Link` before the service, then stops and
  starts the service again, reopening the device as the runner would
  (`Gpu::stale`, `Gpu::reopen`, `XrSpace::setup`). They share `XDG_RUNTIME_DIR`,
  so each holds the tests' `MONADO` lock.

### The on-screen keyboard

beui has an on-screen keyboard of its own, modelled on Android's Unexpected
Keyboard, for a screen with no keyboard attached (a login session on a tablet)
and for typing with a pointer.

Behaviour: each key types its middle when tapped and what its corners show when
dragged toward them; a few keys also take the four straight directions (the
arrows on Compose, the cursor slider on Space, word selection on Backspace).
Circling up, right and down types the key's capital. Shift, Ctrl, Alt, Meta, Fn,
superscript and subscript latch for one key when tapped, lock on a long press,
and apply to every key pressed while held; Shift's corner is caps lock. Fn and
the two scripts change the labels as well as what keys type. Dragging left from
Backspace selects a word per step (Ctrl+Shift+Left) and deletes the selection on
release; dragging along Space moves the caret a letter per step. Held keys
repeat. Compose combines the next keys through X11's compose sequences; while it
composes each key shows what it would do next (`compose::preview`, held per slot
by `KeyCapHandle::previews`): a letter that finishes a sequence shows its
result, one that continues stays as it is, and one that leads nowhere fades, as
do keys that take no part (Esc, the modifiers, the layers and Compose stay); a
key with nothing left on it fades whole.

It is three pieces:

- `beui::native::keyboard::unstyled` (crate `beui-keyboard`): the layouts as
  Rust data (`layout.rs`: `QWERTY`, `GREEK`, `NUMPAD`, built with `KeyDef`'s
  const builders; there is no user configuration), the Fn, superscript and
  subscript tables (`tables.rs`), the compose table (`compose.rs`, generated
  from X.org's `en_US.UTF-8/Compose`), and `Engine`, the pure gesture and
  modifier state machine its tests drive (`engine/tests`). The
  `SoftwareKeyboard` component lays the keys out in rows of `Percent` weights
  inside one forwarding `Interactive` with `press_focus=false`, so every finger
  reaches the engine as its own pointer and pressing a key does not move the
  focus. It hands each key's appearance to a `cap` render function and the emoji
  layer to `emoji`.
- `beui::native::keyboard::styled::SoftwareKeyboard`: the keys painted with
  theme tokens under a separator, a bar with the close button, and the emoji
  panel (`emojis` grouped as the `emojis` crate groups them, leaving out what the
  fonts cannot draw). `install_keyboard` puts a `KeyboardWindow` on a document
  as its `KeyboardHost`, which `beui_host::native::build` does for every
  document.
- `beui_core::software_keyboard`: whether it shows and where, below.

Where it shows: a context shows one only once `Context::enable_keyboard` has
been called, which the runner does on its first frame unless
`Platform::shows_keyboard` says the platform has its own (Android, the web, an
app's host); a platform's own keyboard ignores beui's. `Document::show` (and the
inspector, around the program it shows) calls `show_with_keyboard`, which takes
the keyboard's height off the bottom of the window the focused field is in (the
host's last focused window when nothing is focused, else the first open one when
the main window is not shown), as the mouse simulation's strip is taken: that
window's content and overlays are laid out above it, and the keyboard document
paints into that window (`Context::draw_into`) and hears the pointer in its
coordinates (`InputState::seen_from`). It covers `DEFAULT_SHARE` of that
window's height until `Context::set_keyboard_share` says otherwise (at most
`MAX_SHARE`; be-host sets it from the display settings' on-screen keyboard
height), and its keys stretch to fill it. The keyboard is a document of its own,
shown after the program, with the pointer only while a press began on it or the
pointer is over it, and with no keys. Where windows open
(`Context::opens_windows`, a host such as space) it takes nothing off the
screen: `KeyboardHost::show` gets a `WindowId` the context keeps for it, and
`KeyboardWindow` shows the keys in a `Window` of that id with
`WindowRole::Keyboard` (`keyboard::WINDOW_SIZE`), which the core closes with
`WindowRequest::Close` when the keyboard closes. The pointer counts as on it
while its window is the one the pointer is in. Its window reports go only to the
document holding that window (`Context::take_window_reports` takes those a
filter keeps), so the keyboard's document hears its own `Rastered`.

Opening and closing: `toggle_keyboard()` (or `Context::toggle_keyboard`) is the
one way to ask for an on-screen keyboard, whatever shows it; it counts up
`WindowRequests::keyboard_toggles`. Where beui's keyboard is enabled it opens it
until closed; otherwise the platform answers in `Platform::show_window`: Android
shows or hides its keyboard, the web focuses or blurs its hidden textarea, and an
app passes it to its host as `WindowRequests::keyboard_toggles`, where be-host
toggles its own. The desktop's Quick settings Keyboard tile, the inspector's Sim
tab button and the mouse simulation's keyboard button call it. While the platform
reports no keyboard attached (`WindowReports::keyboard_attached`, which only
`beui-platform-drm` sends, from libinput devices that have an A key; every other
platform counts as attached, and a layer may override it, as space does while
controllers are active), it opens whenever the focused field's
`ImeArea::keyboard` turns `Shown` or its `asks` changes, and closes when the focus
leaves for something that types nothing, except where it is a window of its own
(`Context::keyboard_in_a_window`, which only space sets): there it stays open
until its close button or a toggle closes it, since the person placed it. Its
close button (`close_keyboard()`) closes it until a field asks again.

Typing: the engine makes `Input::Key`s with real modifier presses and no
physical place (`evdev` is `None`; `beui_core::input::typing`, which `drive type`
and `drive key` use too, so a Wayland program gets the key that makes each
character in its own layout, see guides/desktop/linux.md) and `Input::Text` for
what no key types. `Context::type_events` puts them in `FrameOutput::typed`, and
the runner pushes them as the next frame's input, so a beui field, an app and a
Wayland program all take them as from a keyboard. Paste as plain text asks for a
`PasteRequest::Typed`, whose answer the runner types out key by key. The Inputs
page of beui-demo shows the keyboard in place, and the inspector's Sim tab opens
it over the inspected program.

On the HTML core the keyboard shows only where a view places it (the Inputs page
of beui-demo); no host opens it for a field. Its forwarding `Interactive` hears
the pointer through `on_forward`, `type_events` dispatches each key at the
focused element as a `KeyboardEvent` and commits what no key handler took, and
text goes through the focused field's IME agent; `toggle_keyboard` and
`close_keyboard` show and hide the browser's own (`navigator.virtualKeyboard`).

### The inspector

Ctrl+Shift+I in a standalone beui window opens the node, component,
accessibility and performance inspector. Its document is a
`Docking` in a `dock_host::styled::DockHost` that fills the window, with the inspected program as one
of its tabs: a Program tab in a pane split off the left of the tools (Beui,
Comp, A11y, Perf and Sim, 340 points wide), whose body is an `Embed` the
program's document is laid out in. The tools can be dragged beside the program,
under it, beside one another or into floating windows, and the splitter sizes the
program. A window narrower than `NARROW_WIDTH`, or a phone-sized one shorter than
`SHORT_HEIGHT`, stacks the dock, so program and tools take turns behind the
stacked bar; picking shows the Program tab, and a pick returns to the tool shown
last. The tree tools pin a Pick toggle (`inspector.pick`, an icon pin, checked
while picking) to the dock's bar and every tool pins Close (`inspector.close`),
so the inspector has no header of its own. Each tree tool keeps its own rows, and
rows are collected only for the tree tools on show.

Drawing order: `InspectorTools` shows the inspector's document first, so the
Program tab's rectangle is this frame's, sets its painted layers aside
(`Context::set_aside`), shows the program in that rectangle with the highlights
over it, then puts the inspector's layers back on top (`Context::put_back`), so
its menus and windows stay over the program. The Program tab is therefore
`see_through` (see Docking and windows), and the pointer is kept from the program
while it is over one of the inspector's overlays or a press that began on one is
held.

Which document's inspector: the chords (Ctrl+Shift+I, C, M and F) belong to the
innermost document with the keyboard. While the focus is on a node that forwards
keys (`Document::forwards_keys`, such as be-host's app region), the host leaves
them to the app it forwards to, so only the focused app's inspector opens. A
document whose main window is not shown (`Context::shows_main` false: an app,
whose views are windows of its own, or the DRM session) cannot hold its program
in the Program tab, so its inspector is built without one: the tools are drawn
over the right edge of the window that has the keyboard
(`software_keyboard::typing_window`), after that window's own painting and with
its input, and the pointer over them is kept from the program.

Tools and keys: the Comp tab lists the `#[component]`s that built the tree rather
than its base nodes: every component records its name against the node it
returns, so components returning the same node nest there, outermost first.
Ctrl+Shift+C enables node picking. Ctrl+Shift+F moves keyboard focus into the
panel and back out, and Escape inside the panel returns focus to the document,
so the inspector is reachable without a mouse. Tree rows select and expand
together: clicking a row, or Enter or Space on it, selects its node and opens or
closes its children, and the arrow keys walk the tree.

The Sim tab converts one input device into the other, inside `Document::show`,
so it works the same in a standalone window and in a beui app:

- "Emulate touch with mouse" turns mouse presses into touch events.
- "Simulate mouse with touch" turns the shown rectangle into a trackpad and
  paints a cursor the document reacts to, in the shape of the frame's
  `CursorIcon`. One finger moves the cursor, two scroll smoothly. A tap presses
  the primary button where it lands and holds it until the double-tap timer runs
  out, making a click. A press within that time keeps the button down: moving
  drags from where the tap landed, and lifting in place locks the button, so the
  drag carries on across touches until a single tap releases it. A stroke that
  moved the cursor is not a tap. A strip along the bottom holds the left, middle
  and right buttons and a keyboard toggle: a button stays held while its finger
  is down, another finger can work the trackpad meanwhile, and swiping up or down
  on the middle button scrolls a wheel tick at a time. The keyboard button calls
  `toggle_keyboard()` (see The on-screen keyboard); fingers on beui's keyboard
  pass through to it as touches. The strip takes its room out of the window:
  `Document::show` asks the simulation how tall it is, trims that off the bottom
  and lays the inspector out in the rest, as a phone's keyboard pushes a page up,
  and beui's keyboard opens above it. The bar is opaque, since it covers no live
  content.

### Responsive design mode

The Sim tab's "Responsive design mode", or Ctrl+Shift+M, lays the document out
in a screen of a chosen width and height in points, like a browser's responsive
design mode. A toolbar above the program picks a device preset, types the width
and height, rotates the screen and sets its zoom: Fit shrinks it to the Program
tab but does not grow it, and a percentage draws that many real points per
simulated point. The screen sits centred at the top of a backdrop, with handles
on its right edge, bottom edge and corner; a drag holds the scale it started at,
so the handle stays under the pointer, and the screen refits on release. A screen
drawn larger than its room follows the pointer: the point under the pointer is
the point at the same fraction of the simulated screen, so moving across the room
looks around the whole screen. The toolbar is a document of its own spanning the
toolbar and the room below, so its selects can open over the program, which gets
no pointer while one is open. The state lives in the `Context`, so the screen
stays simulated after the inspector closes.

`Document::show` does it with `Context::scaled` and `Context::clipped`, laying the
document out at a rectangle chosen so that a real point is the document point
times the scale; panning moves where the document is laid out rather than adding
an offset. Every mapping stays a pure scale, which a program that reads input or
places surfaces outside `Document::show` relies on: `Context::screen_scale` and
`Context::screen_input` give it the scale and the frame's input in document
points, as be-host reads them for its app surfaces.

### Back gesture

The Sim tab's Back gesture slider plays Android's back gesture into the document,
so a `BackSlide` can be tried without a phone: dragging starts the gesture and
follows it, reaching the end goes back, and letting go before then cancels. It
calls `Document::back` directly, as the Android runner's events do, so it reaches
the document even while the inspector holds the keyboard.

### Filters

The Sim tab's Filters section puts a blur, a contrast reduction and colour vision
simulations over the shown rectangle, independent of each other and of the screen
reader simulation. They are a post-processing pass in `Renderer`, needing no
cooperation from the document, so whatever it paints (an app's beui pane
included) is filtered the same way.

`Context::apply_filter` takes a `Filter` (the region in points, a blur radius, a
contrast multiplier and a `ColorVision`) and wraps everything painted so far,
since the innermost `Context::clipped` or `Context::scaled` it is called in, in a
filter group (`Group::Filter`): what was painted before the call goes through the
filter, and what is painted after lands on top untouched. A second call filters
the first one's result again. The wgpu renderer draws each filter group into a
scene texture of its own, starting transparent, filters it and lays it over what
lies beneath; a filter at the bottom of the frame holds the background too, so it
filters the whole picture. The inspector calls it once a frame after the
document, the panel, the overlays and the screen reader's focus outline, and
before the reader's readout, so the outline blurs with the page while the words
stay readable; the region is the shown rectangle, so the panel holding the
sliders is not filtered.

- Blur: a dual Kawase chain, the frame halved a level at a time then tented back
  up, with the number of levels taken from the radius, so a radius of 120 points
  costs no more than 8. The radius reads like CSS `blur()`: an edge's light
  reaches about three times it. Sampling is clamped to the filter's region at
  every level, so nothing outside bleeds in.
- Contrast and colour vision run in the pass that composites the blur. The
  matrices are the Viénot, Brettel and Mollon linear-RGB approximations, applied
  in linear light; the contrast reduction pulls toward mid grey in gamma space,
  as CSS `contrast()` does.
- Damage: a filtered frame is repainted from the same damage rectangle as an
  unfiltered one. Contrast and colour vision are per-pixel; a blur spreads light,
  so `Prepared::widen` grows the damage by the chain's reach (the sum of what
  every pass can move a sample), clipped to the filter's region, once per
  filter. The scene textures and blur chains are retained like the frame, and
  every pass is scissored to the widened rectangle, since the reach bounds what a
  change can touch at every level. Each damaged rectangle goes through the whole
  chain as a pass of its own. A filter that changed (a new radius, or switched on
  or off) damages only the region it covers: the `Context` reports the regions of
  filters that differ from the frame before, and the renderer adds those it
  prepared differently. A scroll is copied unless what it moves overlaps a
  filter's region.

### The screen reader simulation

"Simulate a screen reader" at the bottom of the Sim tab hands the document over
to what a screen reader would say. A readout takes a fixed-height strip off the
bottom of the shown rectangle (above the mouse simulation's bar when both are
on), holding the last thing said and how far through the document it is; a long
phrase is clipped, not wrapped
([decision](../decisions/beui_screen_reader_readout_is_a_fixed_strip.md)). The
highlight marking where the reader is paints over the document under the
filters, so turning the blur or contrast reduction up leaves the page's shape
with nothing to read, while the readout, outside the filtered region, stays
legible.

The simulation does not read the beui tree. It walks the AccessKit tree the
document publishes every frame, in reading order, and changes the document only
by sending `ActionRequest`s through `Context::accessibility_action`, the path a
platform screen reader uses. An item is anything with a name of its own, anything
that answers Click, Focus, Increment or `SetValue`, and any scroll area; a
control's own text is folded into its name, as a platform computes one. It speaks
the name, the role, the value when it differs from the name, then the state
(checked, expanded, selected, a slider's percentage, "dimmed" for a disabled
control). A control with nothing to name it is announced as its bare role:
"button" on its own is the bug it exposes.

While it is on, the document answers no pointer input directly; the simulation is
the only way to reach things. The reader's commands all hold Alt, and every other
key goes to whatever the document has focused. Landing on a control focuses it,
so walking to a text field and typing types into it, and the arrows, Enter and
Space do what the focused control does. When the document moves focus itself
(Tab, Shift+Tab, a dialog opening) the reader moves to that control and reads it.
Keyboard and touch work at the same time, with no mode:

- Keyboard: Alt with left/right (or up/down) walks an item at a time, Alt+Shift
  with the arrows moves between controls, Alt+Home and Alt+End jump to the ends,
  Alt+Enter or Alt+Space activates, Alt+Minus and Alt+Plus adjust, Alt+Page Up
  and Alt+Page Down scroll, and Alt+R repeats the current item. The document does
  not see these keys. Walking past either end says so and reads the end item
  again.
- Touch: dragging a finger reads what is under it, flicking left or right moves
  an item, flicking up or down adjusts a value, a double tap activates, a
  two-finger tap repeats, and a two-finger drag scrolls; "Emulate touch with
  mouse" makes these reachable from a mouse.
- The same commands are buttons in the panel, listed in the Sim tab, so it can be
  driven without either device.

Ctrl+Shift+F parks keyboard focus in the panel and Escape returns it, as usual;
otherwise the panel does not hold focus while the simulation is on, so clicking a
command button leaves the keyboard commands working.

## Write views

Import `view!`, `#[component]`, signals, and base components from
`beui::reactive`; styled controls come from `beui::styled` and behavior-only
ones from `beui::unstyled`.

`view!` has three framework attributes, usable on every tag, in their own `@`
namespace so a component can name its props whatever it likes:

- `@test_id` names a node for headless interaction tests. It takes a
  `Prop<String>`, so a row whose identity is reactive can bind a memo: the name
  follows it, and the old name stops resolving. A test id is one word. It is
  carried like the node's accessibility: the node's accessibility node holds it
  as AccessKit's `author_id` (several ids joined by spaces), and a named node
  with no role or text of its own still gets one, a `GenericContainer`. So test
  ids reach `FrameOutput::test_id_rect`, `drive` and an app's description by
  the same incremental path as the rest of the tree, and only where the tree
  shows the node (not under a closed overlay). `Document::find_test_id` finds
  the node itself.
- `@node_ref` fills a `NodeRef` when enclosing code needs the resulting
  `NodeId`.
- `@sizing` sets the child's `ItemSize` or `Sizing` among its siblings in a
  list, and only a list accepts it. Children are intrinsic by default; fixed
  children reserve a logical-point size, and percent children share the
  remaining bounded space by weight. On the single root of a `view!` it builds
  a `ListChild`, which is what the row builder of a `Dynamic`, `ForEach` or
  `Keyed` returns.

A children slot names the type of child it takes, which is what confines
`@sizing` to a list:

- `children: Children<ListChild>`: any number of children, each with an
  `ItemSize` (`List`, `Stack`).
- `children: Children<NodeId>`: any number of plain nodes (`Scroll`).
- `children: Children<CanvasItem>`: only the items a `Canvas` can place.
- `children: Child` and `children: Option<Child>`: one node.

A plain node converts into whatever a slot asks for, so
`<List><Text>"hi"</Text></List>` needs no ceremony, and `@sizing` on the child
of a slot that does not size its children is a compile error.

A `Vec` of children does not fill a slot. Children are written out as tags, and
a collection's children come from a `ForEach` over its keys inside the same
`view!`, so sizing, keying and reconciliation stay in one place:

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

Sizing:

- `Frame`'s `width` and `height` constrain a component's own size, and
  `Extent::at_most(..)` makes a box that fills the room it is given up to a
  limit. A `Frame` nested in one that carries a width is laid out within it, so
  the width needs giving only once, on the outer box.
- `@sizing` describes how a node participates among siblings in a `List`.
- `Container`, `narrower_than` and `shorter_than` give container-responsive
  state; `unstyled::Stack` and `styled::Stack` switch between a row and a column
  without rebuilding their children.
- `unstyled::FirstFit` is "this, or that when it does not fit", built on
  `measure`. It keeps all its children built and shows one through a `Portal`
  (the others paint nothing and take no input or Tab stop). Its room is the
  measured width of a zero-height `Frame` beside the shown child. It moves on
  when the shown child no longer fits the room, and goes back to an earlier
  child when the room reaches the width that child last measured at, so in a
  row with `.shrink()` it narrows and widens with the row. It learns a hidden
  child's width only by trying it, so one that shrinks while hidden comes back
  when the room next changes. Each try takes a layout, all before the frame
  paints. `styled::Tabs` and `styled::SegmentedControl` use it to become a
  `Select` of the same options, sharing one `selected`, when their row does not
  fit.

### One layout per frame

A frame runs in fixed phases: input, then layout, then paint. Input is
dispatched against the rects the previous frame painted, which is what the
reader was looking at when they clicked. Events are dispatched once, in the
order they arrived, pointer and keyboard interleaved, and the effects an
event's handlers woke run after that event and before the next one, so a key
typed just after a click reaches whatever the click focused.
`Document::on_interacted` runs a callback once the frame's input has been
dispatched and before anything is laid out, for a program that wants work
started by that input under way while the document lays out.

Effects do not flush inside the dispatch of one event or inside the layout
walk, with one exception: a node being laid out may build its own subtree,
and the effects that build owns run on the spot. `Document::build_within(node,
..)` does it: it lends the walk's `&mut Document` to the build (so its
`with_document` calls go through that reference) and keeps what the build
invalidates from marking the node or its ancestors, which are mid-layout. The
build flushes with `ScopeContext::flush_owned`, which runs only the effects of
that scope and those under it; a write to a signal outside the subtree wakes
its effects once the walk has finished.

The node that builds during layout is `VirtualList`: as the walk reaches each
row its window shows that it does not have yet, it recycles a released row or
builds one, measures it and carries on, so one walk fills the window. A list
laid out at the length it measured follows that length as its rows measure,
rather than stopping at its estimate, and rows that measure shorter at the end
pull in rows above the window to take their place. When its length changed it
asks with `Document::invalidate_after_layout` for the next layout to measure it
again.

Layout is a function of the tree and the constraints it is offered, so a new
window size lays the tree out again from its root and every node offered the
space it had keeps its measurements, placement and paint; only a new scale, new
fonts or a new context lay out everything.

Everything else layout decides is told to the tree after the walk: a
`Scroll`'s position, `component_placed`, `measure` and `measure_text`, and
`Document::on_laid_out`, the callback be-host waits for its apps in. A node with
something to report queues it with `Document::report_after_layout`, and one
whose own size changed while it was laid out with
`Document::invalidate_after_layout`. The reports run together; whatever they
changed is laid out again (walking only the changed nodes and their ancestors)
until nothing changes, so a frame is never painted from a layout the tree no
longer has. Nodes whose scopes ended during a layout, its reports or
`on_laid_out` leave the tree as soon as that step returns, before the next
layout, and what leaving changes (the focus moving off a removed node, say) is
laid out again in the same frame. An open overlay whose anchor moved is placed
again after the walk, but only one the walk placed: an overlay under a culled
ancestor is not placed at all.

A frame lays out at most eight times. When the reports after the seventh still
change something, the eighth layout holds its reports back for the next frame,
the frame is painted from it and asks for another, and debug builds log what
was still changing. Tests fail there instead: beui-core's own, and every test
run through `be-sdk-test`'s `DocumentTest` or `BeuiTest`, which turn on
`beui::native::fail_unsettled_layout`.

Each extra layout costs the frame, so where a report can drive painting rather
than layout, prefer that: a scrollbar's thumb is painted where the position says
rather than laid out, which keeps scrolling to one layout a frame. Reports suit
what depends on how other nodes ended up, such as `TreeReveal`'s button
appearing once the selected row has scrolled out of view.

### Measurement

Where a node was laid out is asked one way on every core
(`beui-view/src/measure.rs`, re-exported by `beui::reactive`):

```rust
pub fn measure(node: NodeId) -> ReadSignal<Option<Measured>>;
pub fn measure_text(node: NodeId) -> TextGeometrySignal; // Get<Option<TextGeometry>>

pub struct Measured {
    pub rect: Rect,
    pub size: Vec2,
}
```

Both are `None` until the node is first laid out, and again once it is
unplaced; `measure_text` stays `None` for a node that is not a `Text`.
`Measured::rect` is in the window's coordinates, the space `PointerPress::pos`
is in, for hit tests and placing things over the node. `Measured::size` is the
size the node was laid out at in its own units, before any zoom, for content
that builds from its own size, so a `Container` in a zoomed `CanvasItem
scales=true` answers the same at every zoom. Each node has one signal per
function, made on the first call and dropped with the node, so a handler may
call `measure(node).get_untracked()` as often as it likes.

`TextGeometry` is a `Text`'s last layout: `bounds()`, `caret_rect(index,
width)`, `index_at(pos)`, `inline_at(pos)`, `lines()` (each a `LineBox` of its
byte range, its rect and its baseline's y, in the window), and `scale()`, the
scale of the space it sits in. Natively it is a snapshot; on HTML its queries
read the DOM. Offsets are UTF-8 bytes of the text's content, counted the same
way on every core: a string or `Span` holds its bytes; a `glyphs` entry is
drawn over the bytes it names and counts as them (a core may draw it in their
place, but takes no room for it); an `Icon`, `Gap` or `Break` holds none; an
inline `TextItem` stands for its `text` (U+FFFC, three bytes, when it has none),
which is how a text area's list marker covers the `- ` it draws, and a
`TextItem` given `at` holds none. The test
`text_geometry_counts_utf8_bytes_and_only_the_bytes_inline_children_stand_for`
pins this down.

Text that is not in the tree is measured with `lay_out_text(text, font,
layout)` (`beui::reactive`): the `TextBlock` a `Painter` would lay out, whose
`size().x` is the width of the text on one line with `TextLayout::DEFAULT`
(rounded up to a pixel, as a `Text` measures it) and whose `line_height()` is
the font's, or `None` before the document has fonts. It answers at once on both
cores (the HTML core from the browser's font metrics), so a component can size
what it builds from text it has not built yet (a text area's line heights, a
markdown table's column widths over rows a `VirtualList` never builds). Read `use_font_revision()` (and `pixels_per_point()`) in the memo
that calls it, so it measures again when fonts load or the scale changes.

A measurement can be `None` and can arrive a frame late, because the HTML core
measures after the browser lays out; a caller that assumes otherwise breaks
there. The native core answers in the same frame: `Document::report_laid_out`
writes every signal that changed after each layout of `update_layout`'s loop,
so an effect that reacts by changing the tree is laid out again before the frame
paints, and a scroll moving a measured node only runs the effects that read it.
`beui_host::native::testing::delay_measurement(true)` (or
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

- Keeping the last answer when it turns `None` keeps a hidden panel from
  rebuilding for a size of zero.
- Content placed by the measured size, rather than only choosing a variant (the
  dock's panes, laid out from its tree's size), is best built once the first
  answer is in rather than at a guessed size: a layout at a size nobody sees
  still moves scroll anchors and reports scroll positions.
- Content that changes the layout from its own size takes one more layout per
  level of nesting, all in the same frame (up to the eight-layout cap).
- A component that only needs where on itself it was pressed reads the `local`
  position pointer events carry instead.
- `unstyled::Container` is this pattern with a context: `narrower_than` and
  `shorter_than` answer for the nearest one.

These rest on the rule container queries settle on: a component may query the
space it was given, but if it also decides that space on the axis it queries, it
reads a size it set itself and never settles. Give such a component its room
from its parent on the axis it reads.

Children placed from their parent's measured size lag a layout behind. Children
whose rects are a function of the space they are given are placed in the layout
pass itself instead: a `Canvas` given an `arrangement`
(`CanvasArrangement::new(|size| Vec<(u64, Rect)>)`, a pure function of the
canvas's laid-out size, compared by identity, so a memo hands a new one when
what it places changes) calls it each time it is laid out and puts every
`CanvasItem arranged=KEY` at the rect returned for KEY (empty if none),
ignoring its x, y, width and height. The HTML core runs it from the canvas's
measured size. beui-dock-host's tiled panes and splitters are placed this way.

### Coordinates and revealing

Pointer callbacks get positions in the coordinates of the node that hears them
as well as the document's: `PointerPress`, `ScrollGesture` and `ZoomGesture`
carry `local` and `size` beside `pos`, and a drop target's `DragPoint` is in its
own coordinates. `pos`, rects (`measure`) and text geometry (`measure_text`)
are in the window's coordinates, through every space's scale as well as its
translation; `local`, `size` and a `HitRegion` are in the node's own layout
units, so a node inside a zoomed `CanvasItem scales=true` hears a press the
same as unzoomed (guides/pan_and_zoom.md). A length of the node's own (a
padding, a radius) compared with window positions is multiplied by the space's
scale first (`TextGeometry::scale()` for a text).

Revealing something in a scroll is resolved inside the scroll's own layout:
`reveal_node` and `reveal_focus` leave a request on each enclosing `Offset`,
which lays its content out, finds the target (or its nearest placed ancestor)
and moves itself until the target is in view. A `Canvas` cannot move itself,
since its view belongs to whoever hands it one, so it answers the same request
through its `on_reveal` callback: after each layout at a new view it measures
the target (or, when that does not fit, the focused text's caret) against its
own rect less the floating overlays over it (a dock bar, the zoom bar), and
calls `on_reveal(delta)` with the least pan that brings it into view. It keeps
measuring at each new view the owner pans to, for up to a second and only while
the distance shrinks, so the owner can ease there a step at a time (the
infinite canvas pans 35% of each delta, or all of it under reduced motion)
without being panned twice by a view that also moved for another reason. Besides `reveal_node`
and a resize while text has the focus, a canvas reveals text as the focus enters
it, so text placed at a canvas's edge is moved into view; a scroll does not, as
a long text area clicked partway down would jump to its top.

A node's parent is the node whose `children` lists it; the arena learns it when
that node changes, not from the layout walk.

### Update a document from outside its events

A signal may be written from anywhere on the UI thread: an event handler, the
host's frame code before or after `show`, a callback the host makes between
frames, a future the program polls. Write it directly:

```rust
value.set(next);
```

The effects that belong to a document wait for its next frame (see Zones in
the [reactive guide](reactive.md#zones)), and the write wakes the window, so
nothing needs to ask for a frame. Keeping the signals the host writes next to
its `Document` makes them easy to find.

`with_reactive_scope(document, f)` installs a document and enters its scope
while `f` runs. It is for work that needs the document itself outside an event,
such as calling `with_document`, running a callback the user wrote as if it
were a click, or creating nodes, signals or effects in the document's scope; a
signal write never needs it.

## Use beui in an app

A beui editor is a component. It implements `be_sdk_beui::BeuiRoot`, whose
`View` and the other views the host asks for are trait components (see
Components in traits and impls), and uses `be_sdk_beui::beui_app!`. The type it
names holds no state: the framework builds the view once and keeps it. An app
module is one beui program, with one `Document` and one `Context`: every region
the host shows of every instance (a frame, a preview, artifact settings) is one
of its windows, a `Window` be-sdk-beui opens from be-sdk-beui-platform-api's
`Views` handle with `views.placement(id)` and the region's size, and its
content is the region's view, so an editor only ever writes `View(editor)`.

```rust
#[component]
pub fn Counter(editor: be_sdk_beui::Editor) -> NodeId {
    let counter = editor.block_content::<CounterContent>();
    let count = counter.field(ObjectId::ROOT, CounterModel::COUNT);
    let increment = move || counter.operate(CounterModel::add(1));
    view! { ... }
}

pub struct CounterRoot;

#[component]
impl be_sdk_beui::BeuiRoot for CounterRoot {
    #[component]
    fn View(editor: be_sdk_beui::Editor) -> Root {
        view! {
            <Root>
                <Counter editor={editor} />
            </Root>
        }
    }

    fn create_block(creation: &be_sdk_beui::Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CounterContent::default()))
    }
}

be_sdk_beui::beui_app!(CounterRoot, "../manifest.json");
```

The counter editor under `crates/be-app-example-counter` is the reference
integration. The [editor guide](adding_an_app.md) covers the manifest, creation
flow, host connection, and current beui app capability limits.

An app depends on beui, which holds no runner, renderer or fonts; the host
supplies input, fonts, clipboard integration, rendering, and the frame
rectangle:

```toml
beui = { path = "../../beui" }
```

`Editor` is everything the instance was given: the host, the block, the
`Blocks` handle it reaches the graph through, the view the host is showing the
content through, and signals for everything the host tells it:

- Block content reaches the view through a `ContentProjection`:
  `editor.block_content::<C>()` holds the content of the editor's own block,
  `project`, `field`, `ids` and `object` derive signals from it, and `operate`
  applies an edit and sends it to the host. The framework pumps every
  projection once at the top of the frame, and `read` and `revision` track, so
  a memo or effect that reads them runs again when the content changes. See the
  [reactive guide](reactive.md#blocks).
- A request the host answers (a file or block pick, a fetch) is an `Asked`
  future, and `editor.then(asked, f)` runs `f` with the answer once it arrives,
  unless the scope that asked has gone (guides/adding_an_app.md, Asking the
  host).
- What the host sends unasked is recorded by the framework and delivered at the
  top of the next frame as signals on the `Editor`: `drag()` and `files()` carry
  a drag or a file drop over the editor; `audio()`, `history(block)`,
  `histories()`, `artifacts()`, `focused_block()`, `web_view_events()` and
  `peers::<P>()` follow the host's state; `block_types()` tracks the catalog;
  `placed()` is the content rect.
- Work on another thread takes its `Waker` from `editor.woken()`, whose signal
  ticks once the work has woken the editor; an effect that reads that signal
  collects the result.

A template the manifest marks `"dialog": true` is made through
`BeuiRoot::Creation` instead: one more component, which the framework builds
into the frame's window and shows in the picker's creation dialog. It says what
the dialog makes with `creation.on_create(...)` and answers
`creation.set_ready(true)` once it has been filled in. Asking the host works
there too, through `creation.then(...)`.

An app's frame can open windows (a dock's floating window, the desktop app's).
Outside space and the desktop shell a `Window` is an overlay inside its
region's window. Where the host shows windows as its own (the screen request's
`windows`, `Region::windows`; guides/adding_an_app.md), each becomes a view
placed by the host (`HostView::Window`). be-host keeps every such window in
`compositor/desktop.rs` (`Floated`): the desktop shell draws them on the
displays (`DesktopWindows`); under space, which says `windows` for every frame
region (`host::opens_windows()`), each is a `Panel` holding an `AppView` of the
window region (`PanelWindows` in `compositor/app_view.rs`), moved and resized by
space on `BeginMove`/`BeginResize`, and closing the panel closes the program's
window. be-sdk-beui places the children laid out in a window on that
window's region (`Editor::end_frame` places only the children in the window it
is given).

## Develop an unstyled component

An unstyled component owns semantics and interaction, not appearance, and has
no appearance defaults: colours, sizes, spacing, paddings, font sizes, scrims
and the renderers for anything it must draw to work (rows, triggers, a menu's
`MenuStyle`, a field's `TextInputStyle`, a text area's `TextAreaColors`, a
sheet's `stops`) are required props the styled layer fills from theme tokens.
Defaults that draw nothing stay: zero spacing, no scrollbar, no focus ring, a
transparent shade, a pass-through panel, an empty optional decoration. So do
behaviour defaults (the drag threshold, zoom limits, a slider's range, the
tooltip delay). Theme colors, fixed visual spacing, typography and decorative
shapes stay out of this layer, so a new skin can use the control without
undoing visual decisions ([decision](../decisions/beui_unstyled_layer_has_no_appearance_defaults.md)).

Put it in `crates/beui-components-unstyled/src/<name>.rs`, declare it in
`lib.rs`, and re-export the public component, handles, state readers, and
supporting types there. Compose it from base components. For an interactive
control this normally means:

1. Model controlled values and transient interaction values with signals.
   State the user changes is a `Binding<T>` prop that the component reads and
   writes, with no copy kept in sync; see `unstyled::Toggle`.
2. Use one `Interactive` for pointer and touch interaction, with
   `focusable=true` for tab order, keyboard events, activation, and focus
   state.
3. Publish the correct AccessKit role and state with `component_accessibility`.
4. Give the caller a `Render<Handle>` or `RenderFn<Handle>` containing the
   reactive state needed to paint the control.
5. Return the root base node directly so component state and node modifiers
   attach to the node callers receive.

### Handles

State that only the component's own handlers use is captured directly. When
callers, tests or host integration need to read it, the component exposes a
typed handle: a `Copy` struct named after it (`ToggleApi`, `SelectApi`), usually
wrapping its private state, whose methods answer what may be asked
(`checked()`, `selected()`, `option_button(index)`). The component takes it as
an ordinary prop, `#[prop(default = ComponentRef::new())] api:
ComponentRef<SelectApi>`, and calls `api.fill(SelectApi(state))` while it
builds. A caller that wants the handle passes its own `ComponentRef` (`<Select
api=select ...>`, then `select.get().selected()`), so the type checks that it
fits; a styled component that wraps an unstyled one takes the unstyled one's
handle and passes it on. Inside the tree, one component reaches another's
handle through such a `ComponentRef` prop, as `TreeReveal` takes its tree's.

`fill` also records the handle against the component's node, so a test holding
only a `NodeId` (from a test id) reads it as `document.api::<SelectApi>(node)`.
A method that needs to know where nodes were laid out takes `tree: &impl
Inspect`, which is what a test holds of a built tree (`api`, `node_rect` and
`is_overlay_open`, which `Document` implements).

### Reaching the tree

The styled and unstyled crates don't name `Document`, `with_document` or
`NodeOf` outside their tests, so their components can be built on another
backend; `be-tool-fix-rust-source` reports it when they do. They reach the tree
they were built into through `beui::reactive` (`beui-view/src/nodes.rs`):

- What concerns one node is a method of the `TreeNode` trait on `NodeId`
  (`panel.focus_is_within()`, `overlay.raise_overlay()`,
  `offset.drive_offset(..)`, `text.text()`), which answers `None`, `false` or
  nothing for a node of another kind or one that is gone.
- Where nodes were laid out is `measure(node)` and `measure_text(node)` (see
  [Measurement](#measurement)).
- The rest are functions: `focused_node`, `set_focus`, `stacked_at`,
  `tree_local` for a value kept once per tree, `use_preferences()` and
  `use_preference_layers()` as signals, `use_preference_layer(source)` as a
  `Binding` that writes that layer, and `use_font_revision()`, which changes
  whenever text may measure differently.

When a component needs something the document has and these do not offer, add
it there.

### Pointer input

A catcher shows the `cursor` it is given while it is hovered or held; one given
none leaves the cursor to the catchers around it, so a catcher that only
listens (for a secondary press, a wheel) does not undo the I-beam of the text
field it wraps. The cursor is the innermost armed catcher's, else the innermost
hovered one's.

Where a catcher hears the pointer is declared ahead of time, not asked at the
moment of a press ([decision](../decisions/beui_hit_areas_are_declared.md)):
`hit_area`, `capture_presses`, `gesture_area`, `drawing` and each `Claim` of
`claims` take an `Area`: `Nowhere`, `Whole`, `Within(HitRegion)` (rectangles in
the node's own layout units, built with `HitRegion::rect`, `with` and
`without`, and allowed past the node's rectangle) or `CaretHandles`. A
component derives them as memos of its state (a `bool` converts to `Whole` or
`Nowhere`).

Each mouse button and each finger is a pointer of its own, and the document sees
their downs, moves and ups in the order they happened
(`InputState::pointer_events`), so a press, a move and a release that share a
frame are three steps.

Hit testing. A down is hit tested once (`interact::hit`): the layers of
`Document::layers()` are taken from the top, by the rules of their policies,
and in the first one the pointer lands on, the hit is the topmost `Interactive`
under it together with every `Interactive` around it, its path. The same walk
collects the layers the press is outside of, which it then asks to close. Any
other node is transparent, and so is an `Interactive` at a point outside its
`hit_area`, along with the catchers around it up to one that captures or
forwards; a passing layer whose content turns a point down lets it through to
what is beneath.

Presses. The pointer keeps what its down reached until it is let go:

- A modifier claim (`claims`) is looked for first, from the top of the tree down
  and past `hit_area`, and a claimed press reaches its claimant alone.
- Otherwise the press goes up the path from the topmost catcher and stops at the
  first whose `capture_presses` area holds it. Every catcher it reached that
  handles presses arms: it hears `on_press`, `on_drag` wherever the pointer goes
  (once in the frame of the press too), and `on_click` when it is let go over
  the catcher, or for a finger, let go without having been dragged.
- The press then focuses the innermost focusable catcher it reached, or clears
  the focus; a claimed or captured press that reached no focusable catcher
  leaves the focus where it was.
- A secondary press reaches every catcher on the path that takes one.
- A middle press is offered to each `on_pointer` on the path from the innermost
  out, stopping at a catcher that middle-clicks or forwards; the first to answer
  `true` takes the pointer and hears the rest of it alone.
- Any other pointer's down, moves, release or cancel go to every `on_pointer` on
  its path (to the claimant alone for a claimed press) until it lifts, unless one
  answers `true` to the down, which takes it the same way.

Raw input. Beside presses, clicks and hover, a catcher hears `on_pointer`, a
`PointerInput` (the pointer, its button, `Down`, `Move`, `Up` or `Cancel`,
`pos`, `local`, `size` and `scale`, whether it is `inside` the catcher, the
modifiers, the time, and `settles`), and `on_wheel`, a `WheelInput`
(`Scroll(ScrollGesture)` or `Zoom(ZoomGesture)`); each answers whether it took
the input.

Finger drags. A finger drag is settled when the finger leaves the tap slop,
along the larger of its two movements. That move goes along the path from the
innermost catcher with `settles` set to the axis: a catcher that holds drags
that way (`touch_drags`) wins it outright, else the first `on_pointer` that
answers `true` does, and every armed catcher that does not hold that drag hears
`on_cancel`. A claimed finger drag stays with its claimant. A second finger, a
long press and a lost window focus cancel the press the same way, and the winner
of the drag hears `Cancel` though the finger stays down. A second finger landing
while the first is held still is a two-finger hold: the `on_pointer`s under it
hear it as a secondary button, which a move of the first finger cancels.

Grabs. A catcher with `grabs_pointer` set hears every pointer event of the
document first, hovering moves and wheel turns included, and one it answers
`true` goes no further. Its `cursor` wins while it grabs, and a key press other
than a modifier, or `Event::PointersCancelled`, sends it `Cancel` (an Escape
that does is taken). `gestures::autoscroll` grabs while it scrolls.

Hover. Hover is the path under the mouse, worked out again every frame, so a
catcher hears `on_hover_change` when it joins or leaves the path whether the
pointer moved or the layout did. `on_pointed_change` is the stricter state, as a
platform's menus and lists have it: a catcher is pointed once the pointer moves
while it is on the path, so one that only scrolled or was laid out under a
resting pointer is hovered but not pointed. A list whose highlight follows the
pointer (`SearchList`, and menus, whose `MenuRowHandle` carries `pointed`)
follows `ButtonHandle::pointed`, so keys walking it under a resting pointer keep
the highlight; other hover visuals use `hovered`. A forwarding catcher that
appears or is moved under a resting mouse is handed a pointer move at the
mouse's position in the same frame (`forward::reforward`), and one it replaces
`Left`, so hover follows the layout. A resting mouse keeps its pixel when the zoom changes: its position is
rescaled into the new points before the frame runs.

Wheel. A wheel turn goes to the innermost catcher on the path under where the
pointer was when it turned whose `on_wheel` takes it, unless a forwarding
catcher comes first (one with `forwards_wheel=false`, such as the on-screen
keyboard, forwards only its presses and lets the wheel pass to what is under
it), and stays latched to it for `WHEEL_LATCH_TIMEOUT` while the pointer holds
still. When every catcher on the path turns an input down (a wheel turn, or a
touch drag's settling move no catcher claims), the core offers it once more
along the path with `unclaimed` set (`ScrollGesture::unclaimed`,
`PointerInput::unclaimed`).

Gestures. Gestures are recognised over raw input in `unstyled::gestures`, not
in the core:

- `scroll_wheel(axis, ..)` takes wheel turns along an axis.
- `scroll_drag(axis, ..)` takes a settled finger drag as `DragGesture`s.
- `autoscroll(..)` is a middle-click autoscroll.
- `secondary_drag(..)` is a right-button drag or a two-finger hold as
  `SecondaryDrag`s.
- `pan_zoom(on_pan, on_zoom)` is a middle drag, a two-finger pinch and pan
  (summed over the frame through `after_input`, which runs a callback once the
  frame's pointer input is dispatched) and a touchpad zoom.
- `first_of([..])` puts several on one catcher, asking each in turn until one
  takes the input.

So a vertical wheel over a horizontal strip passes through to whatever is
around it, and a drag reaches the innermost catcher that scrolls that way.
`unstyled::Scroll` is `scroll_wheel`, `chaining_scroll_drag` (`scroll_drag` with
a test that turns a settled drag down) and `autoscroll`; `PanZoom` is
`be_app_example_pan_zoom`.

Scrolling chains as in a browser. A `Scroll` turns down a wheel gesture or a
settled finger drag that starts with it already at its edge in that direction
(or with nothing to scroll), so the input goes on to the catchers around it,
such as an enclosing `Scroll`. It decides once, when the gesture starts: a wheel
turn within `WHEEL_LATCH_TIMEOUT` of the last one it took continues that
gesture, so a fling or a run of turns that reaches its end keeps going into its
rubber band and never moves the scroll around it mid-gesture. A `Scroll` takes
an unclaimed input even at its edge, so an outermost scroll still bands. On the
HTML core the browser scrolls and chains a `Scroll` itself.

Parts drawn outside the catcher. Parts a catcher draws outside its own
rectangle, such as the touch selection handles of `unstyled::TextArea`, take
their presses with a claim on `Area::CaretHandles`: the handles that the `Text`
inside the catcher paints on its carets, within reach of a finger below each
caret, while the caret is in the text's visible rectangle. Paint order is the
tree's order, so such parts are painted from a node that comes after whatever
they must cover, or from a `CanvasItem` with `clip=false`, which a canvas paints
without cutting it to its own rectangle. The handles are carets with a handle,
which `Text` paints after its text; a single-line area paints them from an
unclipped item after the text's. Where a handle sits is the same on every core:
`handle_center(caret, handle, scale)`, `HANDLE_RADIUS` and `toolbar_anchor`
(where a selection's toolbar opens clear of its handles) are values in
`beui-types`, which a component hit-tests with and a core paints from.

## Develop a styled component

Put a styled component in `crates/beui-components-styled/src/<name>.rs`,
declare it in `lib.rs`, and re-export its public API there. An interactive
styled component wraps the matching unstyled component, passes its `label` on
for the unstyled one to put in the accessibility tree, and renders the unstyled
handle with base visual primitives:

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
its text from `handle.label`, then composes `Frame` and `Text`. Keyboard and
pointer handling stay in the unstyled control. `styled/checkbox.rs` is a short,
complete example of the pair.

Read colors from the nearest theme with `styled::use_theme()`, which returns a
`ThemeStore`, one `Signal` per token. Bind a token straight to a prop with
`theme.accent`, or read tokens inside a memo that also reads interaction state,
so the control repaints when either changes; a component wakes only for the
colors it uses. Spacing, sizes, strokes and radii are the tokens in
`styled::tokens` and `Radius`; write a component's private consts in them. Add a
field to `Theme`, with a value in every built-in theme, when a color is part of
the theme rather than unique to one component.

Styled controls show keyboard focus visibly and keep labels and accessible roles
stable when visual state changes. Icons are glyphs from `beui::icons` in a
`styled::Icon` inside a `styled::Text`. The [keyboard guide](beui_keyboard.md)
records the expected behavior for each control family.

### Buttons and rows

There is one `Button`. It takes a `variant` (`Secondary` unless given), a
`ButtonSize` (`Small`, `Compact`, `Regular`, or `Toolbar`, 30 points high and
square when icon-only), and `icon_only`, which shows only the glyph and turns
the label into the tooltip. `MenuButton` and `Popover` take the same `variant`,
`size` and `icon_only` and paint the same `ButtonFace`. `MenuButton` also takes
an `image` for a picture face, a `leading` node drawn before the label in place
of a glyph (a database choice's coloured mark, a status dot), and
`placeholder=true` while its label stands in for an unset value ("Set status"),
which mutes the label; it adds a chevron to a labelled face unless
`arrow=false`. A custom face over `unstyled::Button` reads `handle.icon_only`
and wraps itself in `<Tooltip label={handle.tooltip}>`.

A pressable row paints `RowFace`: a leading picture or icon (`image`, `glyph`,
or `leading=true` to keep the slot empty so a column of rows lines up), a
`label` and `detail`, and a `trailing` `RowTrailing` (`Chevron`, `Check` or
`Radio`), or `children` in place of the label and detail. Its `RowSize` is
`Compact` for lists, `Regular` for settings (whose label wraps), `Large` for a
picker with 32-point pictures and `Touch` for 52-point rows in a sheet. A
selected row fills with `AccentSoft`, and with `AccentSoftHover` under the
pointer, so it stays visibly selected. `ListRow` (selection and activation),
`ActionRow` (an action, `Touch`), `SettingsLink` (a page of settings, with a
chevron), the menu sheet's rows and the command palette's and search list's rows
are all built on it.

A pick among pictures is a row of `TileButton`s: a square tile (`size`, `Xl` by
default) around any child, filled `AccentSoft` and ringed in the accent when
`selected`, with `outlined` for a hairline border at rest, `radio=true` for
radio semantics and `toggle=true` for a tile that reads as pressed while
`selected` (the desktop's Wi-Fi tile); without either, `selected` is only drawn,
for a tile that only acts (`description` reads after the label). The rename
card's icons and the display settings' rotation picker are radio tiles. A colour
pick is a row of `SwatchButton`s.

`Banner` is an inline bar across an editor that says what failed (an import, a
load) until the user dismisses it: its `message` is an `Option<String>`, and it
shows nothing while that is `None`. Its test ids are `{id}` and `{id}.dismiss`.
`unstyled::Banner` gives it the alert role, an assertive live region labelled
with the message.

`ToolPill` is the raised, bordered, fully rounded bar a floating set of tools
sits in, or with `band` set the flat fill of a toolbar band (`BAND_FILL`, which
beui-dock-host's band is drawn with too; a dock bar's body uses `DockBarSurface`,
which sets `band` while the bar is docked beside an edge). It lays its children
in a row, or a column with `direction=Direction::Vertical` (a dock bar on a side
edge), with arrow-key focus along it, scrolling when offered less than it needs.
Placing it (a `Floating`, a dock bar, a margin) is the caller's.

`keep_clear(node, rects)` names parts of a node (rects relative to it) that
something floats over, as floating overlays do: a `Canvas` revealing its focused
content keeps it out of both. beui-dock-host keeps an over-the-body dock bar clear
this way, and a joined editor's tab body is kept clear of the bars the root
draws over it (`DockHosted.covers`).

### Text

`styled::Text`, `styled::Span` and `styled::Icon` take the same children as the
base ones and resolve design tokens into a base `Text`'s `font` and `color`:

- `size`, a `TextSize` on a scale from `Xs` to `Xl5`; `TextSize::px` is the
  point size, for the rare layout that must measure against it.
- `weight` (`Weight::Normal`, `Weight::Bold`), `family` (`Family::Sans`,
  `Family::Mono`) and `italic`.
- `color`, a `TextColor` role (`Default`, `Muted`, `Faint`, `Accent`,
  `AccentHover`, `AccentActive`, `OnAccent`, `OnDanger`, `Danger`, `Warning`,
  `Success`) resolved through the nearest theme, so the text follows a theme
  change. `TextColor::Custom(color)` is for colors that are data rather than
  design (a user's label color, a map marker, a terminal's ANSI palette). A memo
  that picks a color by state returns a `TextColor`.
- `style`, a `TextStyle` preset: `BODY`, `CAPTION` (small and muted),
  `HEADING`, `TITLE`, `DISPLAY`, `CODE` (small and monospace) and `PARAGRAPH`
  (muted). A prop given beside a preset wins over it. `TITLE` and `HEADING` are
  headings, levels 1 and 2 (`TextStyle::heading(level)` makes any style one):
  the text is an accessibility `Heading` on both cores, `role="heading"` with
  `aria-level` on HTML. Unlike the rest of a style, a heading level is not
  inherited from a `TextStyleProvider`.
- `overflow`, as on the base `Text`: text wraps unless told otherwise, so a
  paragraph needs nothing, and a one-line label (a button's, a row's, a title in
  a bar) takes `overflow=Overflow::Ellipsis`.

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

A lone icon is a `Text` holding only an `Icon`; give it
`align=TextAlign::Center` when it is placed in a box bigger than itself.

`TextStyleProvider` gives every styled text created within it a style its own
props override, the way a disabled section mutes everything in it. It is
context, so it reaches what is created inside it, which is not always what is
laid out inside it: text a render callback builds for an overlay takes the
provider around where the callback was written. A `Span` or `Icon` written
inside a `Text` takes what it does not set from that `Text` instead.

Raw pixel sizes and `Color32`s belong to base `Text`, for text that is drawn
rather than designed: labels on a zoomable canvas, a terminal, a game board.

Fonts. Text is Inter Regular (`Inter-Regular.ttf` from Inter's release,
unmodified), with Hack for monospace and Noto Emoji as the fallback; icons are
Lucide's font. crates/beui-fonts holds all four: their bytes, which
beui-font-freetype loads and the browser rasterizer serves, and their file and
family names (`FACES`), from which the HTML core writes its `@font-face` rules;
BUCK exports each file for web bundles. Bold is synthesized from the regular
face on both cores.

Icons. The icons are Lucide's, drawn from its font
(`beui-fonts/src/assets/icons/lucide.ttf`, committed unmodified from the
`lucide-static` npm package). They live in `crates/beui-lucide`, which
`beui_core::icons` re-exports and which knows nothing of beui, so the block
catalog (`be-sdk-ui`) holds them too.

- An icon is a `Glyph`, its codepoint in the font. Only the list can make one,
  so a `glyph` or `icon` prop, a `Prop<Option<Glyph>>` (`glyph=ICON_SAVE`, or
  `None` for no icon), cannot be handed text by mistake. The app protocol, which
  knows nothing of the font, carries codepoints as text; `Glyph::from_codepoint`
  reads one back, and `Glyph::as_str` is the text a painter lays out.
- The list, `crates/beui-lucide/src/list.rs`, is one line per icon: its `ICON_*`
  constant, its Lucide name, its codepoint, and its tags from `tags.json`
  followed by the older names (aliases) it shares a codepoint with. The
  `icons!` macro in that crate turns it into the constants, `from_codepoint` and
  `ALL`, a `NamedIcon` per icon in name order.
- `named` looks a name up in `ALL`. Anything saved holds an icon by name, since
  Lucide renumbers icons now and then
  ([decision](../decisions/beui_icons_are_saved_by_name.md)); an icon picker
  lists and searches `ALL` (the workspace's appearance card).
- Search the list's tags for what an icon is for ("duplicate" finds
  `ICON_COPY_PLUS`) rather than guessing from names.
- The list is generated. To update Lucide, unpack the package (`npm pack
  lucide-static@VERSION`, then `tar xzf`) and run `./scripts/buck run
  //crates/be-tool-local:bin -- icons PACKAGE` from inside the repository, which
  rewrites the list from `icon-nodes.json`, `font/codepoints.json` and
  `tags.json` and copies in the font.

### Colours

A colour prop on a primitive (`Frame` `fill`, `Text` and `Span` `color`,
`Outline`, `SpanStyle`, `TextMark`, `TextGlyph`, `SpanBackground`, `TextCaret`,
`Overlay` `scrim`, `Root` `clear_color`) is a `beui::Color` (beui-types), not a
`Color32`. A `Color` is a literal `Color32` or a reference to a `ColorVar`, plus
a small algebra both cores can carry: `scale_alpha(f)` and `mix(other, share)`
(two unmixed colours). A `Color32` converts into a `Color`, and a `Color32`,
`Memo<Color32>` or `Signal<Color32>` passes for a `Prop<Color>`, so literal
colours can be passed as they are.

A `ColorVar` is declared with `beui::color_var!(pub SURFACE, TEXT)`, not named by
a string. Its identity is where it is declared (module path and item name), so a
variable is reached only by importing it and two crates cannot collide. Its CSS
name (`css_name()`, `--v-` and a hash of that path) is the same in every build,
so a page prerendered on the server hydrates to the same names; `name()` keeps
the readable name for the inspector and `describe()` (`var(SURFACE)`).

Whoever references a variable defines it, with a value per `ColorScheme`
(`Light`, `Dark`): `define_colors(definitions)` (`beui::reactive`) takes a
`Prop<ColorDefinitions>` (a map from `ColorVar` to `SchemeColors { light, dark
}`) for as long as the calling scope lives; later definitions of a name win. The
core and unstyled know no colour names.

- Native resolves a reference when it paints, for the scheme of the document's
  theme preference (`ColorScheme::from(ThemeKind)`: `Light` and `EPaper` are
  light), and repaints everything when the scheme or a value it already held
  changes.
- The HTML core writes `var(--NAME)` (and `color-mix()` for the algebra) and
  puts the definitions in a `<style>` inside `.beui-root`: the light values on
  `.beui-root`, the dark ones under `@media (prefers-color-scheme: dark)`, and
  both again under `.beui-root[data-scheme=light|dark]`, which the core sets
  when the program chose a theme (`beui::html::choose_theme`). So a page
  prerendered by `render_to_string` already shows the browser's scheme before
  any script runs, and hydration computes the same references and changes no
  colour.

Because the server and the browser can disagree on the scheme, code that picks a
colour by reading the theme's kind or the scheme renders a prerendered page
wrong; pick a reference whose definitions differ per scheme instead
([decision](../decisions/beui_colours_are_scheme_references.md)). Code that needs
the `Color32` itself (a `Drawing`'s painter, image tints) calls
`resolve_color(color)`, which tracks the definitions and the scheme;
`color_scheme()` reads the scheme.

`styled` is the one place theme colours live. `styled::Theme` holds the colour
tokens as `Color`s. The built-in themes, one per `ThemeKind` with literal
colours, are `Theme::DARK`, `Theme::OLED` (dark on pure black), `Theme::LIGHT`
and `Theme::EPAPER`; `Theme::of(kind)` picks one, and a theme's `kind` says
which it is. Styled components that no provider covers use the document's
theme, whose tokens are references (one variable per token and syntax colour)
defined with the light values of `Theme::LIGHT` (or `EPAPER`, when chosen) and
the dark values of `Theme::DARK` (or `OLED`), so the scheme picks between them
on either core.
`Theme::referenced` and `Theme::definitions` build those. What dims the program
under an overlay is a theme colour too: `scrim` under a dialog, sheet or dock
dialog, `scrim_soft` under the command palette and launcher, which leave the
program readable, and `scrim_strong` under `Fullscreen`.

Colours of a caller's own with a value for each built-in theme (the choice
colours in be-sdk-beui, for example) follow the theme the same way:
`define_themed_colors(definitions)` takes a `fn(light: ThemeKind, dark:
ThemeKind) -> ColorDefinitions` and defines it once per tree for the kinds the
scheme would pick, and `theme.themed(var, |kind| literal)` (on the `ThemeStore`
from `use_theme()`) gives the reference under the document's theme and the
literal for the provided theme's kind under a `ThemeProvider` (the `referenced`
field of `ThemeTokens` says which). `Theme::of(document.preferences().theme)`
reads the literal theme, for example to pick a program's clear color.

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
`beui-reactive` module. Shared modules with a small core-specific part
(`actions`, `animation`, `nodes`'s `TreeNode`, `file_picker`, `file_saver`,
`drag`, `window`) keep their logic in `src/<module>.rs` and call
`crate::core::<module>::*` for the rest. How to test a body on both cores is
under [Cores](#cores).

### The HTML core's bodies

On the HTML core a container's own behaviour lives in a `beui-core-html` module
its body hands the elements to:

- `scroll` (`Offset`): an `overflow: hidden` element moved through
  `scrollTop`/`scrollLeft`, not a transform, so the browser's own scrolls (find
  in page, `scrollIntoView`, focus) are adopted as the offset
  ([decision](../decisions/beui_html_scroll_uses_the_element_scroll_position.md)).
  It anchors on the first element under the offset and corrects by its drift,
  snaps to the end, and reveals.
- `rows`: `VirtualList`'s window and row measurement; rows are flow between two
  `data-beui-spacer` elements that anchoring skips.
- `canvas`: an inner element translated to the view's origin, with
  `--beui-scale` scaling item positions and `scales` items.
- `overlay`: a `<dialog>`: `showModal()` for `BlocksAll`, else
  `popover="manual"`, whose `::backdrop` lets the pointer through. A `BlocksAll`
  dialog inside an open `BlocksAll` one (a submenu) is such a popover too, so
  the menu beneath it stays reachable as natively. It is placed with CSS anchor
  positioning, measured with `resolve_rect` where unsupported; the scrim is on
  `::backdrop`; it handles Escape, outside presses and focus traps.
- `portal`.
- `text` (`Text`): a `div` with `white-space: pre-wrap` and a default
  `line-height` of 1.15, as native's. It keeps the order of its children and the
  bytes each stands for, so `glyphs` cut a run into a `span.beui-glyph` whose
  `::before` draws the glyph over bytes that keep their room and copy as
  themselves. `marks` are CSS Custom Highlights, one per colour, or positioned
  boxes where `CSS.highlights` is missing; `carets` (blink, flag, handles) are
  positioned boxes in an `aria-hidden` overlay at `caret_rect`; `hang` is a
  negative `text-indent` with a matching `padding-left`, measured from where the
  `hang` byte sits on the first line, so it holds only for a Text whose first
  paragraph is the one that hangs; `Break` is `::after { content: "\A" }`, which
  no browser copies. Marks, carets and hang need the browser's layout, so a
  painting from `render_to_string` shows glyphs but not them; the shared scripts
  (`editing`, `text_input`) exercise them. Its `TextGeometry` queries read the
  DOM when called (`caretPositionFromPoint`, `Range` rects), mapping UTF-8
  bytes to each text node's UTF-16 offsets, and it compares equal until the
  Text's bounds or content change.

Each registers a pass with `frame::on_layout`, which the frame loop runs (up to
`LAYOUTS_PER_FRAME`) before paint; `ResizeObserver`s call `frame::relayout()`
so a resize is corrected in the same frame. A primitive the HTML core has no
body for is an outlined pink stand-in there. unstyled and styled build on both
cores; under HTML, `Selectable` is the browser's selection, a window is its
overlay, `on_back` never fires and file pick and save go through `<input
type=file>` and a download link.

`run` and `hydrate` read the browser's preferences before they build, so
hydration starts from the scheme the prerendered page shows
([Colours](#colours)). Browser paintings render in Chromium's light scheme
unless the page chose a theme.

### Native base nodes

Base nodes are normally the only layer that mutates a `Document` directly. Put
the node in `crates/beui-core/src/base/<name>.rs` and register the module in
`base.rs`, and its component in `crates/beui-view/src/components/<name>.rs`. A
base implementation has three parts:

- A node struct containing its retained state and child ids.
- An `Element` implementation for measurement, layout, painting, interaction,
  child traversal, and inspector metadata. Its `remove_child` lets go of a child
  whose scope went while the node still held it, so a node holding one child
  keeps it in an `Option`; a `ChildHost` hands it to `remove_hosted`.
- `Document::create_*` and `Document::set_*` methods beside the node, plus a
  public `#[component]` wrapper in `beui-view`. The wrapper creates the node with
  `with_document` and binds reactive props to setters with `create_effect`.

`Document::create_*` returns a `NodeOf<XNode>`, a node id that carries the kind
of node it names, and the setters that reach the node's fields take that handle,
so a setter cannot be handed a node of another kind. `.id()` is the plain
`NodeId` for everything that takes any node. Code that only holds a `NodeId`
(from a `NodeRef`, or a walk of the tree) asks
`document.arena.kind_of::<XNode>(id)`, which answers `None` for a node of
another kind, and a query meant for such code takes a `NodeId` and answers
`None` or `false` itself, as `is_overlay_open` does.

Return children from both interaction traversal and `children`, give the node a
stable `kind` for the inspector, and add a concise `detail` when it makes the
tree easier to understand.

The `base` module itself is private. Re-export base components intended for
composition from `beui::reactive`, as `Frame`, `Text`, and `Scroll` are, and keep
primitives that exist solely to support an unstyled control crate-private.

### Measure and layout

`measure` takes `&self`; `layout` takes `&mut self` and may update the node's
own retained state, and may build its own subtree through
`Document::build_within`, which is how a `VirtualList` builds the rows its slice
of the viewport calls for. Both receive `&mut Document` and are reached through
`beui_core::layout::measure` and `beui_core::layout::layout`, which take the
element out of the arena for the duration, so a node reaches its children
through those two functions rather than calling another element's methods
directly.

`unplaced` is the other half of `layout`. The layout calls it on every node it
placed last pass and did not place this one (a child its parent now skips, and
everything under that child), so a node that keeps retained state for what is
on screen can let it go. It runs in the same pass, after the dropped rects are
gone, and it may remove the node's own children, which is how a `VirtualList`
scrolled out of view releases its rows. A node that is laid out again gets
`layout` as usual and rebuilds whatever it released.

Measurements are memoised per available size and dropped whenever the node or
something under it changes its layout, so measuring a child repeatedly within a
pass is cheap, and a `measure` that is not a pure function of the node and its
constraint returns stale answers. A memoised size also answers a smaller offer it
still fits: a node measured at a bounded width `a` to `s` answers any bounded
width between `s` and `a` with `s`, and a node measured at an unbounded width
answers an offer of exactly `s` with `s` (likewise for height). That keeps a
list from measuring a child again at the length it just measured, so `measure`
has to agree: offering a node more room than its size may change its size only
when that room is unbounded.

`min_width` answers the least width the node can be laid out at without breaking
what must not break. It defaults to nothing; `Text` answers with its longest run
that cannot be wrapped when it wraps, a node that wraps one child forwards the
child's answer (a `Frame` adding its padding, or answering its own fixed width),
and a `List` adds up a row and takes the widest of a column. A row reads it for
its `Percent` and shrinking children only, and it is memoised beside the
measurements.

`baseline` answers where the node's first line of text sits below its top when
it is offered a size, for a row aligned to `Align::Baseline`. It defaults to
none; `Text` answers from its first line, a node that wraps one child forwards
the child's answer through `beui_core::layout::baseline` (adding whatever it
shifts the child down by), and the answers are memoised beside the
measurements.

To keep layout linear, a node measures each child a bounded number of times per
pass, and anything it computes across its children is linear too: `base::share`
hands a length out by weight between minimums and maximums in linear time, and
is what a list's shrinking and percent children and a grid's fraction columns
use. `nested_lists_measure_each_node_a_bounded_number_of_times` holds a deep
tree of lists to that.

A node whose size cannot depend on its children (a `Frame` with a fixed width
and height, a `Canvas`, a canvas item) returns true from `relayout_boundary`. A
change under it stops there: its ancestors keep their placement, and it is laid
out again on its own from the rectangle and painter it was last placed with. A
node whose `measure` reads its children returns false, or its ancestors miss
the change.

### Setters

A setter reaches its node through one of three arena accessors, chosen by what
reads the field:

- `get_mut_as` for anything `measure` or `layout` reads: the node and its
  ancestors are measured and laid out again. A field `paint` reads through
  something `layout` computed, like the origin a `Text` places from its
  alignment, counts as layout.
- `paint_mut_as` for what only `paint` reads (a colour, a tint, a drawing): that
  node alone paints again and nothing is laid out.
- `touch_mut_as` for what neither reads (handlers, a cursor, a tab stop): only
  the accessibility tree hears of it.

`state_mut` is for state the pointer dispatcher keeps on an `Interactive`
(hovered, armed, panning), which nothing laid out, painted or published reads,
so no one hears of it; what that state changes reaches the screen through the
callbacks it calls. Each accessor marks the node dirty, so a setter calls it
only when the value changes.

### Coordinate spaces

Every node is laid out and painted in a coordinate space of its own, starting at
its top left corner: `layout` and `paint` are handed a rectangle at the origin,
of the node's size, and the rectangles `layout` gives its children are in those
coordinates too. A node that only moves (a row pushed down by the one above it
growing, a panel beside a splitter) keeps its layout and its recorded shapes;
its parent records where it went. A scroll's `OffsetNode` and a `Canvas` add a
content space inside their own (`Document::enter_space`, and `Painter::shifted`
when painting), which carries the scroll offset or the pan: scrolling or panning
changes only that translation, so the rows keep their rects and their shapes and
neither is laid out nor painted again.

`Document::node_rect`, `measure`, the rect `interact` is handed and a
`CanvasView`'s origin are in the document's coordinates. A node that needs to
know where it sits in them asks the painter: `painter.origin()` is where its
space starts, and `painter.clip_rect()` is what of it is visible, in its own
space. Asking either marks the node as depending on its place, so it is laid
out, or painted, again when that changes; an `Embed` reports its rectangle this
way, and a `VirtualList` realises the rows its clip shows. A `Drawing` callback
that paints in document coordinates rather than relative to the rectangle it is
handed, as `be_app_infinite_canvas` does through its camera, paints through
`painter.in_document()`.

### Painting and damage

Painting is retained per node. A node's `paint` runs again when the node
changed, when its size or the painter it is handed changed, when a child it
paints moved or was culled or uncovered, or when a repaint it asked for falls
due; otherwise the shapes it recorded last time stand, and its children are only
visited when something under them has to paint. Being laid out again is not by
itself a reason: a node whose paint reads something its own `layout` works out
says so with `Element::paints_from_layout` (an `Offset`, a `Canvas`, a
`VirtualList`), and a `Text` asks for its own repaint from `layout` only when
the lines it placed differ from last time. So `paint` has to be a function of the
node, its rectangle, the painter and the rectangles its own layout gave its
children; anything else it reads goes unnoticed when it changes. A scroll's new
offset relays out its `Offset` alone, from the place it was last given.

What a node records is a tree (`display.rs`): its own shapes, its children by id
and translation, and group nodes around them (`Group::Clip`, `Group::Scroll`,
`Group::Mask` (a fade), `Group::Transform` and `Group::Filter`), which a painter
opens (`with_clip_rect`, `shifted`, `faded` with the space of the fade that
opened it, `in_document`) rather than writing them onto each shape or child: a
recorded shape's own `clip` is always everything, and `WindowOutput::shapes`
folds the groups into it. `PainterState::within` is what each group does to the
painter inside it, so a node visited without painting again hands its children
the painters its tree says they had.

When a node paints again its new tree is set against its old one by identity:

- Its own shapes damage where they were and where they are, except a `Drawing`
  that is the same drawing in the same place, which damages only what it says it
  redrew.
- A child, matched by id, damages its old and new place when it was added,
  removed, moved or restacked, and nothing when it stayed.
- A mask whose fade changed damages its bands.
- A scroll group, matched by the space it scrolls, whose offset changed is a
  copy of what it showed rather than damage.

A repaint that paints the same thing still damages its shapes. Damage is held as
at most four rectangles (`damage::Region`), each drawn as its own pass, and two
of them merge whenever their bounds are at most twice the area they cover, so a
large change spread over nearby spots is drawn once. A `Drawing` whose content
changed in part hands its `Drawing` node `draw_gpu(Some(drawing.redrawn(region)))`
rather than a new `Drawing`: only that region, in the drawing's own coordinates,
is damaged (the app host does this with the rectangles each app frame reports it
changed).

Checking paint. beui's own tests, and every test that drives an app through
`be-sdk-test` (which turns on `beui::native::verify_paint`), paint every frame
again from scratch and fail when the retained painting differs from it or
changed outside the damage. The damage check (`paint/verify.rs`) compares the
frames node by node, matched by id, and requires every pixel a difference
touches (rounded out to whole pixels) to be damaged or, inside a copied region,
to be what the copy brought. Only windows still open are checked: a window that closes (even
one whose content `keep_alive` holds) is no longer shown, and one that opens again is shown whole.
Painting from scratch runs every paint again, so a test that
counts paints skips the ones made while `Context::is_verifying_paint` is set.
With `BEUI_OVER_REPAINT=1` in the environment (or
`beui::native::detect_over_repaint(true)` on the thread that paints) they also
report every frame whose damage is more than four times the area of the shapes
that changed, to stderr and to `beui::native::take_over_repaints`.

Rendering. A frame hands the renderer these trees, with the frame's own groups
around them (`Context::clipped`, `Context::scaled` and `Context::apply_filter`
wrap what they cover), rather than a flat list of shapes; `WindowOutput::shapes`
flattens them on demand, for tests and software rasterising. `Renderer` encodes a
node's shapes once and keeps their instances in a GPU buffer, only walks the
trees that reach the damage, and draws a scroll group through a translation and
clip of its own, so a scroll or a pan encodes nothing again.

Scroll copies. A scroll whose offset changed reports its copy as
`WindowOutput::moved`. A host that keeps its last frame (`beui_host::native::run`,
and space inside each window's panel texture) copies that region with
`Renderer::shift` and repaints only what the copy cannot supply: the exposed
rows, whatever changed, and whatever does not move with the rows (a fade's
bands, a scrollbar). One region is copied per window per frame, the largest when
several scrolls moved. A copy is kept only when its distance is whole pixels at
the window's `pixels_per_point`, so a scroll snaps its offset to that pixel
grid, and in the main window only when nothing painted outside the document (an
inspector, a filter) overlaps it. `Repainting::repaint` on the `WindowOutput`
covers the moved region for hosts that do not copy: app surfaces, the DRM
screens, and the web canvas, which repaints its whole frame whenever anything
changed.

### Interaction

Pointer input only visits a node when the pointer lies within the rects of it
and everything `children` returns under it, or when it leads to a node that
holds pointer state. A node that keeps state between events (hovered, pressed,
dragging) returns true from `engaged` until it lets go, so it still hears the
pointer leave, release or move away. Keyboard input goes to the focused node and
its ancestors rather than through this walk.

## Test and verify changes

Beui behavior is tested headlessly. For library behavior, add a test under the
relevant `tests` directory and declare it in that directory's `tests.rs`.

Every test that runs a document or a program runs it through
`beui_host::native::testing::Stepped`, the way a platform does: through the real
`Runner` under one drive layer, on the headless window's `Simulation`
(`Stepped::new`) or on a `PlatformServices` the test supplies (`Stepped::on`),
with a gpu-free `testing::Screen`, the clock stopped, and one frame interval per
frame. `testing::Showing(document)` is a `Document` as a program.

- Its gestures are `drive`'s commands, run by the same drive layer
  (crates/beui-platform-headless/src/testing.rs): `click(Target::id("wheel").at(offset))`,
  `drag`, `down`, `up`, `hover`, `scroll`, `zoom`, `pinch`, `fingers`,
  `type_text`, `key`, `key_down`, `wait` and `settle` are typed forms of them, a
  `Pos2` is a target too, and `ask(&["back", "0.5"])` sends any other. Each
  settles as `drive` does, unless the test turned that off with
  `settle_after_gestures(false)`, when it answers after the frame that took the
  input.
- `frame(inputs)` pushes the `Input`s it is handed in points
  (`Incoming::points`), paints one frame, and hands back its output, for a test
  of what a single frame does. Inputs go through the runner's intake, so a test
  sends what a platform would: an `Input::Modifiers` before a key or a press it
  means to be held with (`Harness`'s `pressed_with` sends a chord that way),
  `Input::PointerLeft` for the pointer leaving, and
  `Input::Scroll(ScrollDelta::Pixels(..))` for a scroll in points. A `Runner`
  is the only way input reaches a `Context`; there is no way to hand it an
  `Event` directly. `Context::run(frame)` paints one frame with no input, for
  drawing offscreen (the canvas's export) or a test of painting alone.
- A test that stamps samples with times pushes `Incoming::Time` through
  `runner().push` and then paints `frame(Vec::new())`.
- A failure prints the steps so far as `drive` commands
  (guides/running/drive.md), which replay against a `:dev` program.
- A `Stepped` showing a document compares paintings (guides/testing_a_gui.md, 4)
  of the latest frame, painting one first only if none has been:
  `snapshot(name)` the main window, which also fails on what it shows out of
  bounds; `snapshot_layers(name)` the main window composited with the layers
  that escaped it; and `snapshot_window(name, id)` a window it hosts. Any other
  painting goes through `beui_paint_snapshot::assert_painting(name, painting)`.

beui's own tests use `Harness::new(document)` or `Harness::sized(document,
size)` from `crates/beui-host/src/tests.rs`: a `Stepped` with settling turned
off, plus what those tests share: finding and measuring nodes, and driving the
inspector and the dock. Tests of the `Runner` itself take `testing::Screen`
rather than a renderer of their own.

For a beui block editor, use `be_sdk_test::BeuiTest`, built from an `Editor` (or
a `Creation`, for a dialog) so the test holds the same handle the component was
handed. Give every interacted node an `@test_id`, call `run` after queued
gestures, assert the resulting block state, and snapshot when the painting is
meaningful. `BeuiTest` also supports key presses, text, hover, pointer clicks,
and touch gestures. See the [GUI testing guide](testing_a_gui.md) and the
counter editor tests for examples.

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

`render` builds in a fresh reactive scope, flushes bindings, writes the doctype
and HTML, then disposes the scope; `render_fragment` omits the doctype. For a
retained tree, build inside a scope you keep and call `Node::to_html` after
updates. String children are escaped static text; `Text` binds reactive text.
`Show`, `ShowKeepAlive`, `ForEach` and the other `beui-tree` control flow
components work with HTML children.

### Elements and attributes

HTML is built through `view!` and components; low-level node construction is
private to the backend. Import `beui_html::elements as html` for ordinary HTML
elements. Their declarations in `beui-html/src/elements.rs` generate normal
components with shared global props and element-specific attributes; extend
those declarations for missing standard attributes.

- Rust keywords use raw identifiers (`r#type`, `r#for`).
- Boolean props control attribute presence; numeric attributes take numbers;
  other attributes take strings. Omitted attributes are absent, and signals of
  `Option<T>` remove or restore attributes reactively. Static props need no
  effects.
- Void elements have no children prop and reject children at compile time.
- `@data={("test-id", value)}` and `@aria={("expanded", "false")}` set
  namespaced attributes; ARIA boolean values are strings.
- `Element tag="custom-tag"`, `@attr={("custom-name", value)}` and
  `@flag={("custom-flag", enabled)}` are escape hatches for custom markup.
- All attribute names are checked and values escaped.

A multi-child `view!` produces `Children<Node>` without a wrapper. Components can
return that type, and iterators of nodes or child runs collect into it. Both HTML
renderers accept a node or a child run.

A component prop can take a view directly: `prop=<Component />`, or a run of
children with `prop=<>...</>`. Fragments `<>...</>` also work anywhere children
fit, including at the root of `view!`; they group children without a backend
node. Empty fragments `<></>` build empty children. Expressions, reusable
components and control flow work inside fragments.

`view!` and `#[component]` locate their support crate automatically, including
renamed dependencies, through `proc-macro-crate`, so a caller can depend on
`beui`, `beui-view`, `beui-html`, or `beui-tree` with no macro path setting.

The compiler grammar lives in `beui-macros/src/lib.rs`; the source formatter has
its own grammar in `be-tool-fix-rust-source/src/views.rs`, using lossless source
tokens. A change to view syntax needs both grammars and their tests, or the
formatter fails on the new syntax.

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

`<Stylesheet css={...} />` and `<Script kind="module" source={...} />` carry CSS
and JavaScript/import-map source. They preserve source text and reject a closing
tag for their container. CSS can live in separate files read with
`include_str!`.

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
parser keeps as text (`script`, `style`, `textarea`, `title`, `noscript`) get no
markers, and hydration rebuilds their content. Hydration walks the tree and the
page in serialization order, claiming elements by tag, text by content
(splitting text nodes the parser merged, creating empty ones) and markers. It
trusts the server's attributes. A mismatch warns on the console in debug builds,
builds that node fresh and removes the server nodes left unclaimed in that
element or run; it never panics.

Elements take event props `on_click`, `on_input`, `on_change`, `on_submit`,
`on_keydown`, `on_keyup`, `on_focus`, `on_blur`, `on_pointerdown`,
`on_pointermove` and `on_pointerup`, and `@on={("name", handler)}` listens to
any other event. Handlers are `Callback<beui_html::Event>`; write a closure in
braces with its argument typed, `on_click={move |_: html::Event| ...}`. `Event`
has `key`, `holds` (a modifier key), `composing`, `target_value`,
`target_checked`, `prevent_default`, `stop_propagation` and, on wasm, `raw`, so a
handler compiles for both backends. The string backend ignores listeners. The
live backend flushes after each handler; for signals set anywhere else, the host
installs a flush with `beui_reactive::wake_with`.

`web-sys` and `dom` exist only for `target_arch = "wasm32"`, so native builds
never compile them. beui's HTML core builds its elements with beui-html
([Cores](#cores)), so `./scripts/verify`'s check of the HTML core's libraries on
`wasi_html` compiles the live backend, and crates/beui-tests' `checks()` exercise
mounting, hydration and listeners in Chromium (guides/testing_a_gui.md, 7).

The web entry points are generated by `be-tool-local web-page` through its
`:block-index` and `:demo-index` Buck targets. Their scripts, import maps and
styles live under `crates/be-tool-local/src/web_page/`.

