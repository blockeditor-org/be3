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
builds rather than its siblings. Nothing checks for the attribute; a plain
function that writes a `view!` builds into its caller's scope.

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
`beui::styled`, `beui::unstyled`, `beui::Document`, ...) and assembles them,
so code outside beui names only `beui`. The rest depend on each other in one
direction:

| Crate | Owns |
| --- | --- |
| `ui-types` | the vocabulary beui shares with the plugin protocol, which depends on nothing but serde and `geometry`: `Key` (with its names for drive, `Key::from_name` and `Key::name`, its evdev code, `Key::evdev`, and what every adapter names keys with: `Key::from_typed`, the one rule for a character key, `Key::from_value` for a W3C key value, and `Key::evdev_from_code` and `Key::from_evdev` for a physical place), `Modifiers`, `KeyChord`, `PointerButton`, the touch, pen and back phases, `ImeEvent`, `CursorIcon`, `Pasted` and `PickedFile`; `Input`, the raw input every platform adapter, drive's gestures, the on-screen keyboard and the plugin protocol produce; `WindowRequests` and `ImeArea`, what an app asks of its window; the preferences (`ThemeKind`, `Motion`, `Preferences`, `PreferenceLayer`); and the display and pointer settings `beui-adapter-drm` applies and block settings store (`DisplayMode`, `Rotation`, `InputDevice`, `PointerSettings`) |
| `beui-tree` | what makes a tree of components, whatever it is made of: `#[component]` and `view!`'s runtime, `Prop`, child slots (`Children`, `Run`, `SlotChild`, `SlotHost`) and `Show`, `ShowKeepAlive`, `Dynamic`, `Keyed` and `ForEach` |
| `beui-core` | `Document` and `Context`, the retained nodes and their layout, input and its dispatch, accessibility, paint output and damage, the font and image interfaces, the icon codepoints, the `App` contract, and the `Runner` every adapter drives it through |
| `beui-font-freetype` | `FreetypeFonts`: FreeType and HarfBuzz shaping and rasterizing, and the fonts beui compiles in |
| `beui-font-browser` | `BrowserFonts`: text measured with the browser's own fonts through a canvas, for the DOM renderer; no fonts in the module |
| `beui-view` | `beui::reactive`: `beui-tree` re-exported with the rest of the framework (actions, timers, `NodeRef`, layout values such as `Align`, `Align2`, `Extent`, `Sides`, `Corners` and `Outline`, and the node functions and `Inspect` through which components reach the tree); `beui::primitives`: the base components (`Frame`, `List`, `Text`, ...) that wrap core's nodes; `beui_core::tree` is what makes a `NodeId` a child |
| `beui-components-unstyled` | `beui::unstyled` and `beui::datetime` |
| `beui-components-styled` | `beui::styled`: themes, styled controls, the styled `Frame`, `Text`, `Span` and `Icon`, and every primitive with no styled version re-exported unchanged |
| `beui-inspector` | the inspector, the simulated screen reader and the simulated mouse and keyboard |
| `beui-renderer-wgpu` | the wgpu renderer, its shaders and filters, and presenting to a surface |
| `beui-renderer-dom` | the DOM renderer: the display tree as nested absolutely positioned elements |
| `beui-adapter-winit`, `beui-adapter-android`, `beui-adapter-web` | each platform's `Adapter` and `Platform`: its event loop, window or view, input, IME, clipboard, file picker and accessibility adapter (winit's is its own `accessibility.rs` over `accesskit_unix`, `accesskit_windows` and `accesskit_macos`, since `accesskit_winit` has no release for winit 0.31) |
| `beui-adapter-drm` | Linux's displays and input devices through DRM/KMS and libinput: an `Adapter`, `Platform` and `Renderer` whose screens each keep a retained frame and are drawn when their display flips (the app's frames are paced to the flips too, by `pacing.rs`: at most one per flip, none while nothing changes), handing the flip a fence (or, without atomic fences, waiting for it in the event loop) instead of blocking on the GPU; callers run it with `beui::run_on` |
| `beui-adapter-space` | the VR simulator `WindowHost` that `BEUI_ADAPTER=space` wraps around a native or headless adapter: the app's windows as panels in a 3D scene ("Space: the VR simulator host" below) |
| `beui-adapter-plugin` | the `Adapter`, `Platform` and `Renderer` for one region of a block editor plugin, which the plugin framework drives frame by frame (guides/adding_a_plugin_editor.md); it depends on the plugin framework, so the facade does not offer it |

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
  (`Document::extension`, which components reach as `tree_local`).
- A `Drawing` holds whatever its renderer draws; `beui::drawing` makes one for
  the wgpu renderer.
- Each platform is a `beui_core::runner::Adapter`, a trait object the facade
  picks (`beui::window_adapter`, `beui::web_adapter`) and runs with a `Launch`
  (`RunOptions`, `Context`, the `App`). The adapter owns the event loop and
  turns the platform's input into `Event`s; everything one frame does is
  `beui_core::runner::Runner::frame`, shared by all of them: it runs the
  app, publishes accessibility, and tells the adapter's `Platform` (another
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
  showed before, only when they differ, so an adapter compares just the
  fields it uses; a new field is one more field there and in the adapters
  that act on it. Typing moves the platform's own copy of the field (the
  web's hidden textarea, Android's editable) even when the app turns the
  text away or a composition ends with the field unchanged, so `typed`
  counts the frames that delivered IME or text input, and the change is
  when the web and Android adapters bring that copy back to the app's
  text, comparing against what they last sent. The IME
  area it hands over is in the window's own units (winit's logical pixels,
  CSS pixels, a plugin region's points: physical pixels over the platform's
  scale, so points times the zoom), and `Runner::window` is the state last
  shown. What happens once rather than holds (a copy, a paste, a file pick,
  `set_fullscreen`, closing) has a call or field of its own.
- What the window did goes the other way, through `Runner::reports()`, a
  `WindowReports` handle an adapter may clone and keep (the web adapter
  hands one to its `fullscreenchange` listener): `resized` with its size in
  physical pixels, `scaled` with its pixels per point (winit's scale factor,
  the browser's device pixel ratio, Android's density, a plugin region's
  scale factor, the DRM displays' scale), and `fullscreen`. The runner
  applies them at the start of the next frame, resizing the renderers and
  setting `Context::set_pixels_per_point` before the app lays out, so no
  adapter resizes a renderer or passes a scale itself. A report made during
  a frame (a simulated window answering) makes that frame ask for the next.
  Fullscreen goes both ways: `Context::set_fullscreen` asks the window to
  enter or leave it, the runner passing on each request that differs from
  what the window last reported, and the window reports what it actually
  did, whether the app asked or not, which the app reads as
  `Context::fullscreen` and a component as `use_fullscreen()`. The DRM
  adapter is always fullscreen; Android and plugins say nothing.
  The clipboard is the platform's: `Context::copy_text` writes it, and
  `Context::copy_image` (`reactive::copy_image`) writes a PNG in its place
  (`Platform::copy_image`: winit through arboard, the web as a `ClipboardItem`,
  a plugin through its host's `EditorMessage::CopyImage`, the headless
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
  adapter reads images too; Android and DRM read text only.
  An `Interactive` with `clipboard_keys` (a terminal) takes Ctrl+C, Ctrl+V,
  Ctrl+X and Ctrl+A as plain keys while it or a node inside it has the focus
  (`Document::clipboard_keys`, sent as `WindowRequests::clipboard_keys`, which a
  plugin's region passes on to its own catcher). Natively that changes nothing,
  since those arrive as keys anyway; the web adapter, which decides in `keydown`
  before the app has seen the key, keeps the last flag it was shown and stops
  the browser's copy, paste and cut for them, and its inspector for Ctrl+Shift+C,
  but leaves Ctrl+Shift+V to make the browser's `paste` event, so a node that
  lets that key go gets its paste as everywhere else. beui's inspector leaves
  Ctrl+Shift+C alone too. A copy the node makes on a key, through `copy_text`,
  still reaches the web clipboard, since it is written within the key's user
  gesture. A node that takes these keys and leaves Ctrl+V unanswered gets no
  paste on the web.
  `Platform`'s methods all have defaults (no clipboard, no file picker), so a
  platform implements only what it has.
  A new platform implements those two traits and hands its window or element
  to the renderer loader the facade gives it.
- An adapter only translates: it hands `Runner::push` an `Incoming`, which is
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
  report a pinch, which the web adapter turns into `Input::Zoom` at
  `PINCH_SPEED` per pixel) or Safari's `gesturechange`, whose scale over the last one's
  is the factor; the web adapter ignores a gesture while fingers are down,
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
  `Ended`, in `beui-adapter-winit/src/wheel.rs`): beui's fling replaces them,
  and they would scroll clamped at an end instead of banding. The browser
  reports neither, so a web wheel scroll never stretches or flings. Every pointer sample is an event of its
  own: the web adapter pushes each of a `pointermove`'s
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
  the time it happened: an adapter pushes `Incoming::Time(Instant)` just after
  the event it stamps, and the runner gives it to that event's
  `PointerEvent::time`, the session it moves and the `PointerPress::time` its
  catchers hear (touch fling velocity is measured with it too). The web
  adapter reads `timeStamp` from each coalesced `PointerEvent`, Android sends
  every historical sample of a `MotionEvent` with its own event time, the DRM
  adapter reads libinput's, and winit, which reports none, stamps an event
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
  converts them with the same scale the frame lays out at and no adapter
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
  (`Event::PointersCancelled`) and stops autoscrolling. Only the adapters
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
  `Frame::wait` is how long the adapter may sleep before the next frame
  (`None` until something wakes it), and `Frame::next` the same once it
  knows whether the frame it presented asks to be presented again.
  Closing is the runner's: when the app calls `close_window` (or a driver's
  close is accepted) the runner exits after that frame and `frame` returns
  `None` with `Runner::exited` set, which every adapter answers by stopping
  its loop (Android also finishes its activity, the web adapter closes the
  tab where the browser lets a script, and a plugin region's `Frame` says
  `stopped` from then on, `Stopped::Closed` or `Stopped::Failed`, which the
  host shows in the region's place). An adapter whose window is closed from
  outside calls `Runner::os_close`, which asks the app's `close_requested`
  and exits if it agrees; the web adapter asks on `beforeunload`, and a
  refusal is the browser's leave-the-page prompt.
  A panic out of the app's `update` stops the runner rather than unwinding
  through the event loop: the app hears `exiting`, `frame` returns `None` from
  then on, `Runner::failure` says why, and the adapter ends its loop and
  returns that as an error (`Runner::finish`), so whatever owns the screen is
  let go of in order. An app that can carry on after a panic in one part
  catches it there itself.
- Every adapter draws through `beui_core::renderer::Renderer` trait objects and
  names no renderer crate: wgpu's `WindowSurface` (native windows),
  `CanvasSurface` (a browser canvas) and `DomRenderer` implement it, and the
  facade hands the runner the ones to load. A renderer is told where to draw
  with `attach` (a raw-window-handle `WindowHandle`) or is made with its element,
  and draws a frame in two steps, `prepare` and then `present`.
- `DomRenderer` builds its elements through the `Dom` trait: `WebDom` is the
  page's DOM (the crate's `web` feature, which beui's `dom` feature turns on),
  and `MemoryDom` an in-memory one that writes itself out as HTML, which is how
  tests paint through it (guides/testing_a_gui.md). It keeps an element for
  each display it has shown, keyed by `Display::document` and `Display::node`:
  every document numbers its nodes from zero, so the node alone does not
  identify a display once the inspector paints beside the app.
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
  `Portal`, `BackHandler`, `Shift`, `Turn`, `Fade` — and it stays small because most things are
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
is offered and keeping its shape, as an image does. A sized `Drawing` clips its
callback to the rectangle it gets, so a callback that draws in the units of its
`size` scales itself into that rectangle (`painter.transformed(offset, scale)`,
as the demo's Paths page does) rather than spilling past it.

Text is never one of those. A base `Text` takes concrete values and knows
nothing about the theme: a `font` (a `FontId`, 14 points proportional unless
given), a `color` and `underline`, and the props that shape the paragraph -
alignment, `wrap` (a `Wrap`: `Off`, the default, lays each line out
unbroken; `Words` and `Anywhere` below), `ellipsis`, `clip`, `line_height`,
`line_padding`, `marks`, `glyphs` and `carets`. Its content is its children: string literals and
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
terminal row drawn with `wrap=Wrap::Off` copies as one line. A newline in a
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
and wraps them itself: `Wrap::Words` breaks after whitespace, `-`, `/` or `\`,
or after a `break_after` span, and between letters when no such break fits;
`Wrap::Anywhere` breaks between any two grapheme clusters, so each line holds
as many as fit (in a monospace font at a width of `cols` advances, exactly
`cols` characters; measure the advance as `cursor_pos(_, 1).x` of one
character, since a galley's `size()` is rounded up to a pixel). Each run is as
wide as its text's advance, so spans in a monospace font stay on its grid.
It lays text out with `line_padding` above and below each line, and aligns, truncates with an
ellipsis and spaces lines by `line_height` whatever its content. It caches each
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
floats beside a position in the text. `Document::text_geometry` answers where an index is and which index is
under a point from the node's last layout, without a document installed, and
`text_bounds(node)` is a signal of where its laid-out text reaches (its natural
size, not the rect it was given), written after each layout that moves it.
`unstyled::TextArea` is built from exactly this: a row per document line in a
`VirtualList`, each a `ForEach` of the row's spans, a gutter of `Text`s beside
it, and list markers, voids, embedded widgets and the caret's anchor as
`TextItem`s.

The galleys a `Drawing` paints come from `layout_text(text, font, layout)`,
which lays text out with the shown document's fonts and answers `None` until
the document has been shown once. It is how a component measures text outside
`measure` and `paint` - to work out where a caret sits, or how wide a column
is - and the galleys it returns are cached, so asking for the same word twice
costs a hash lookup. A `FontId` carries `bold` and `italic` alongside its size
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
browser's, whose text the DOM renderer draws with CSS and scales with the
group's transform) keeps the galley as it is.

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
tokens instead: `fill` a `ColorRole` (a theme colour, or `ColorRole::Custom`
for one that is not), `radius` a `Radius` (`Xs` to `Lg`, `Full`, or
`Radius::Custom`), and `border` an `Option<Border>` (`Border::DEFAULT`,
`Border::FOCUS`, `Border::new(role)` with `.width(..)` and `.offset(..)`;
`focus_border` shows one only while the focus ring is visible). A
`ColorRole::custom(..)` wraps a colour or a signal of one.
A frame can also paint a `pointer`, a beak of that reach on its top or bottom
edge, filled and outlined with the frame so its border runs on around the beak;
it shows only while an overlay points it at its anchor (below). It paints
outside the frame's box without taking room, so give it room in a padded frame
around it: a plugin's host draws what is outside an overlay's content under the
editors it holds. The styled `Frame` takes it as a token, `pointer=true`
(`POINTER_REACH`). Frames paint no shadows: a floating surface stands apart by
its fill and border.
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
column weights, so its cells line up, and it reads as a table to assistive tech.
`Text` carries its own decoration too: `underline` is painted from the galley's
baseline, so switching it on never moves anything. `Portal` shows a subtree that belongs to
someone else: it takes a `NodeId`, lays it out and paints it where the portal
stands, and leaves it alone when the portal goes away, so a subtree can move
between places in the tree without being built again. Exactly one portal shows
a given node - claiming it takes it from the portal that had it - and the
subtree lives as long as the scope that built it. `Embed` reserves a rectangle
for something outside the document — an editor the host composites behind the
surface — publishing the rectangle and the clip it was laid out in through the
`EmbedSlot` it was given (a `Copy` handle owned by the scope that made it) and cutting that rectangle out of the surface so what
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
paints nothing while the child is off the screen, which is how a sheet slides in. `Turn`
paints its child turned clockwise by `quarter_turns` about the middle of its box, which
takes the child's size with width and height swapped for an odd number of turns, and
further by `angle` radians, which leaves the box as it is; clips inside it grow to the
upright bounds of what they clip, so nothing turned is cut off. Pointer
input and the accessibility tree still see the child unturned, so it is for pictures, such
as a preview of a turned screen or a canvas's turned text, not for controls. `VirtualList` is an ordinary box that stands for
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
outside it all close it the same way, and the styled find bar holds a
`BackHandler` while it is open, so back closes it as Escape does. The state republishes
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
plain-text buffer it owns, bound to a `value` it reads and writes, so a
fix to how text is edited lands in both. `multiline` makes it the multiline area
instead, with `line_numbers` off, where Enter starts a new line and the field's
chrome wraps the whole scrolling area rather than one line. `prose` (a `Prose`, on
`TextArea` and on `TextInputStyle`) lays the multiline area out as plain text
rather than a document: no whitespace markers and no document padding, with
the weight, horizontal alignment, padding around each line, wrapping, and where
the lines sit when they are shorter than the area that it names.
Nothing measures text off the tree: a `TextArea` reports the size its laid-out
rows reach through `on_content_size` (padding included, read from the rows'
`text_bounds`), and a caller sizes a box from that. The infinite canvas grows a
text entity this way while it is edited, and draws each text and label as a
read-only rich `TextArea` over a `LiveText`, the same area whether or not it is
being edited, with `styled::DocumentMarker` as its marker hook, so a checklist shows the styled
`Checkbox` at the size of its text (a `Checkbox`'s `size` is the `TextSize` it sits beside). Its export is the one place text is laid out away from the tree
(`TextAreaState::lay_out_prose`): it gives every marker a square the height of
a line and paints it, and each quote's bar, as a pink box, since export does
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
unstyled defaults draw from `TextAreaColors`; `styled::TextArea` draws with `DocumentLeaf`,
`DocumentMarker` and `DocumentLine`, which use theme tokens, and a caller that draws voids handles `MarkerKind::Void`
and hands every other marker to `DocumentMarker`. Copying and pasting
carries the text without its marks. A menu is never asked for as a dropdown
or as a sheet: every menu opens as a dropdown when a mouse opened it and as the
same items in a `ModalSheet` when a finger did, so the two show the same
features. A `MenuStyle` carries both looks (the styled `menu_style()`), and an
unstyled one without a sheet stays a dropdown. An item's children cascade as a
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
greatest one past which labels end in an ellipsis, is never taller than a fixed
height, and scrolls the row the keyboard is on into view. `MenuPopup` is the same menu
opened by something other than a button, such as a model holding a request: it
shows while `open` holds, under `anchor` at `placement`, as the sheet while
`sheet` holds, at least `min_width` wide, and reports `on_dismiss` for Escape or a press
outside. A host that opens one for a plugin's button (block-app's app menu)
anchors it to the button's rect, which the plugin sends from `node_anchor`, and
sets `sheet` from `last_pointer` (`Document::last_pointer`): the host routed that
press, so it knows whether a finger made it. `MenuButton` is the button that
opens a menu under itself, which is what a toolbar reaches for where `Select`
would imply the choice sticks (`IconMenuButton` is the same with an icon
button's face); `ContextMenu` is the same menu on a secondary press, and it
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
`touch_drags`, which a canvas that draws or moves things under the finger does,
or `touch_drag_axis`, which keeps finger drags along one direction only and
leaves the other to the scroll around it, as a dock tab in a scrolling tab bar
does.
A pen is `Event::Pen`: a `PenPhase` (`Move` in range, whether or not the tip is down, `Down`
and `Up` of the tip, which is `Primary`, and of its barrel buttons, and `Gone` when it leaves
range) and a `Pen` with its pressure from 0 to 1, its tilt in degrees and whether it is the
eraser end. It is `PointerId::Pen` and behaves as the mouse does: it hovers, moves the pointer,
presses and drags without a slop or a long press and never scrolls what it is dragged across,
so every control works with it unchanged (`PointerId::hovers` is true for both). Its
`PointerEvent`s carry the `Pen`, and the `PointerPress` a catcher hears carries it too, as
`pen`, sampled when it was pressed and at each drag; a finger's `PointerEvent`s carry its
`force` where the platform reports one. The DRM
adapter reads libinput's tablet tools, the web adapter pointer events whose `pointerType` is
`pen`, Android a stylus or eraser `MotionEvent`, the winit adapter winit's tablet-tool pointer
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
parent's painter: it keeps its rect, so `node_rect`, `reveal_node` and
placement signals still find it, but nothing inside it is laid out, painted or
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

`Selectable` (and `SelectableText` over it) selects across the plain `Text`s
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
reports and wraps the offset in a base `Fade`, which paints its child inside a
mask group (`Painter::faded`, `Group::Mask`): the wgpu renderer multiplies the
alpha of everything in that group by it in the shader, and the DOM renderer
masks the group's element with a gradient. Custom `Drawing`s are not faded.

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
out in place (`Element::passes_scroll_anchor`: `List`, `Frame` and `Fade`),
taking the first child that reaches past the top of the viewport each time.

The offset places its child once per layout. What the content's own layout
decides about the anchor comes back to the offset as part of that layout's
result: a `VirtualList` whose rows above the view measured differently from
their estimate reports the difference with `Document::report_anchor_shift`,
and the nearest enclosing offset sums what its child reported with how far the
anchor drifted and any reveal it was asked for. When that sum is not zero the
offset moves by it and is laid out again in the frame's next layout, where it
does not re-anchor. Anything that moves the offset from outside its layout
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
a `Drawing` that a shader paints on the wgpu renderer, behind
beui-components-styled's `gpu` feature; the DOM renderer cannot run it and
fills it with magenta.

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
prop to the handle's `open`, as the calendar, the time list and the color area
all take.

### Tooltips

`unstyled::Tooltip` shows something after the pointer has rested on its child
for a moment, and `styled::Tooltip` is the bubble version of it:

```rust
view! {
    <Tooltip label="Reveal the block being shown">
        <IconButton glyph=ICON_LOCATE_FIXED label="Reveal" on_click={find} />
    </Tooltip>
}
```

`IconButton` already wraps itself in one, so an icon-only button says what it
is without the caller doing anything: the `label` it publishes to a screen
reader is the text the bubble shows. Wrap anything else whose meaning is not
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
which is what lets a scroll fling or a spinner stop costing frames. Start a
timer from the event or effect that begins the motion rather than leaving one
running.

UI motion uses `create_animation(step, finish)`, not a repeating timer. Its scope-owned,
copyable `Animation` handle has `start()`, `stop()`, `finish()` and `running()`; starting
an active animation keeps its clock, stopping cancels without completion, and finishing
applies its terminal state exactly once. The step receives `AnimationFrame` (`now`,
`elapsed` since start, and `delta` since the previous step) and returns whether another
frame is needed. Returning false finishes it. A callback may start another animation;
disposing the scope cancels without completing it. Publish values through signals.

Use `tween(elapsed_seconds, duration_seconds, Easing)` for duration-based progress and
`Spring::advance` for a displacement and velocity relative to a target. Tweens catch up
to the frame clock after stalls; springs solve damped motion analytically rather than
clamping away elapsed time. Retarget a spring by changing its target while keeping its
current value and velocity. `Fling` supplies the shared inertia curve. Components keep
interaction decisions and semantic completion actions, not their own frame scheduling.

`Motion::Instant` finishes active animations and completes new ones without scheduling
animation frames; the default is `Animated`. It is one of the preferences (below). Each
document exposes `motion()` and the reactive `watch_motion()`. Instant mode disables inertia
and rubber-banding, keeps carets visible and spinners static, and leaves held gestures
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
- `PreferenceSource::System`: what the platform prefers. Adapters report it with
  `WindowReports::system_preferences`: winit from the window's theme and, on Linux, the
  desktop portal's `color-scheme` and `reduced-motion`; the web adapter from
  `prefers-color-scheme` and `prefers-reduced-motion`; Android from night mode and a zero
  animator scale; the DRM adapter says e-paper and instant motion on an e-paper panel; the
  plugin adapter reports the host's resolved preferences, which reach a plugin in its
  acceptance and in `Message::Preferences`; plugin tests send them with
  `BeuiTest::set_preferences`.

Set a layer with `Context::set_preference_layer(source, layer)` and read them with
`preference_layers()`. Each document mirrors them at the start of a frame (`preferences()`, and
`watch_theme()`, `watch_motion()` and `watch_canvas_touch()` for one each);
`Document::watch_preference_layers()` is that, reactively, for settings that show where a value
comes from; components read these as `use_preferences()` and `use_preference_layers()`, and
write a layer through `use_preference_layer(source)`. `styled::PreferencesSection` is a settings section over the app layer, with a
`styled::ThemeSelect` and `styled::MotionSelect` whose first option is the value inherited from
below, and notes a developer override. An app that keeps its own choices passes
`RunOptions::saving_preferences(name)`, which loads and saves the app layer in the platform's
config folder (beui-demo does); one that does not still follows the system.
`RunOptions::config_directory()` is the folder an app's own files go in: the saved
preferences' folder, else the platform's config folder joined with `app_id`, else none. block-app stores
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
content's padded edge, stands `gap` off the anchor, and points the frame's beak
at the anchor's middle on whichever side it opened.

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
  topmost layer that has an enabled `BackHandler` in it or is dismissable,
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
enabled `BackHandler` in that layer that is in the tree (not in hidden
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
them (`base/focus_scope.rs`): an open layer pushes one and closing pops it.
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

A base `BackHandler` only routes: it calls `on_back` when back completes, or
hands every phase of the gesture to `on_gesture` if it has one, and moves
nothing. The motion lives above it. `unstyled::BackSlide` wraps a
page: held, the page follows the finger across a good share of the screen;
let go, it carries on off the edge before `on_back` runs and the next page
slides in behind it, and a cancelled gesture eases it back. Given `behind`, a
function that builds the page back goes to, it builds that page under the
moving one for as long as the gesture lasts, shaded and coming in from the
side, so `on_back` swaps in what was already showing and nothing slides in
afterwards; the page that moves needs an opaque background, and a page that
must keep its state (a scroll position) is built once and handed to a `Portal`
in both places. The stacked dock shows the tab back goes to behind the tab it leaves
this way. A back with no gesture before it (a key, Escape, a press outside)
carries the page off from where it rests in the same slide-away. `styled::Dialog` and
`Fullscreen` slide away the same way and fade their scrim, and a `Sheet`
sinks with the gesture and slides on down from where it was. On Android the
runner passes `handles_back` to the activity's `setBackHandled`, and when
nothing takes it the system's own back (to the home screen) plays instead.

A `Center` overlay is never larger than its screen: content that measures taller
or wider is laid out in the screen's size, so its bottom cannot fall off a short
window. `styled::Dialog` puts its title and body in a fitted `Scroll` with
`Bleed::DIALOG` that gives up length when the screen is short, so every dialog
scrolls rather than spilling over the edge.

A dialog's buttons are props of `styled::Dialog`, never buttons in its body, so
every dialog places and draws them the same way: a footer under the scroll that
stays in view while the body scrolls. `confirm` names the action (drawn primary,
or `ButtonVariant::Danger` with `destructive`; `confirm_disabled` greys it) and
runs `on_confirm`; `cancel` names the dismissal (always secondary, alone too,
such as "Close" or "Done") and runs `on_dismiss`. An empty label
leaves its button out. Confirm is rightmost and cancel beside it, and
`extra_actions` (such as Delete in an edit form) start the row on the left.
The row wraps rather than stacking, so the buttons stay side by side until they
no longer fit. Enter that nothing in the dialog takes runs the
confirm action, unless it is disabled or destructive, so a field needs no
`on_submit` of its own. With `id`, the buttons are `{id}.confirm` and
`{id}.cancel`. `KeepChanges` draws its prompt with the same surface and footer.

A layer at `LayerLevel::Lock` that blocks everything is a lock screen
(`styled::LockScreen`): it is above every other layer, so anything opened while
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
holds. An `Interactive` given `claim_modifiers` takes every primary or secondary
press in its rectangle while all of those modifiers are held, and `claim_at`
decides the same per point. The claim is searched for from the top of the tree
down, before `capture_presses`, so the outermost claimant wins; the press, its
drag and its release reach the claimant alone, none of its descendants, and no
forwarding catcher under it unless the claimant is that catcher.
`Document::press_claims` lists the `claim_modifiers` rectangles laid out this
frame, which a plugin reports to its host. A Wayland program's window is a
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

A canvas a pen draws on is an `Interactive` with `drawing` (or `drawing_at` per
point). While the `canvas_touch` preference is `CanvasTouch::Off`, a finger that
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
`hovered`. Given `menu`, which answers a key with a `TreeMenu` (its items and
`on_select`, as a `ContextMenu` takes them) or `None`, every row with one shows
a "…" button at its end, on a desktop too, and the same menu opens from it, a
right click or a long press: a dropdown under a mouse and a sheet under a finger.
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

### Screens

A document knows the screens it is shown on: `Document::screens()`, and
`use_screens()` in a component, a signal of `Screen`s (an `id` that stays the
same while the monitor is plugged in, a `name`, and a `rect` in document
coordinates). A window, a browser tab, an Android view, a plugin's region and
an inspector panel are one screen the size of what the document is shown in;
under `block-app --session` there is one per monitor. They come from the
renderer: `Renderer::screens` hands the runner each monitor in physical
pixels (beui-adapter-drm's outputs, beui-adapter-plugin's from the monitors
the host says its region covers), the runner puts them on the `Context`
(`Context::set_screens`, which is also how a test gives a document two
screens), and the document keeps the part of each inside the rect it is shown
in. Nothing else should keep its own list of monitors.

Overlays place themselves on one screen rather than across the box around all
of them. A `Center` or `FillScreen` overlay is on the screen it opened on: its
trigger's, else where the pointer was last pressed or moved unless a key was
pressed since, else the focused node's, else the pointer's, else the first. One anchored to a node is kept on the screen
most of that node is on, one anchored to a point on that point's screen, and one
anchored to a rect (`OverlayAnchor::Rect`, in document coordinates) on the screen
most of that rect is on, placed against the rect as it would be against a node.
`styled::Toasts` takes any anchor this way, so linux-desktop stacks its toasts in the
corner of a screen above its bar. A toast goes after its `duration` unless it is `sticky` or
`danger`: an error stays until it is dismissed, so it cannot be missed.
`Fill`, `At` and `Around` use the whole document, so something meant for every
screen - `styled::KeepChanges`, `styled::LockScreen`, `styled::LevelOsd` - lays
out one card per entry of `use_screens()`, which is a single card everywhere
but a desktop with several monitors.

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
from its own width (the unstyled `Docking` stacks below its `stack_below`), and
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
keep it in theirs. A tiled bar is 40 points with its rule, its tabs 30; its controls are
`IconButtonSize::Toolbar` buttons (30 square) and the labelled pins compact
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
keyboard for when it intercepts the chord, so linux-desktop also registers
Alt+Escape as an intercepting action that cancels. The dock binds no other key: whoever binds it decides the
keys, usually a chord that begins or steps the switch while a modifier is held,
and `held_modifiers()` letting go of that modifier to commit,
as linux-desktop does with Alt+Tab and the demo with Alt+Q.

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
window: a press on its bar calls `begin_move()` and a grip `begin_resize(edge)`
(as does a secondary drag with `drag_modifier`, from the nearest edge or
corner), so the host moves and resizes it. A size set from inside, such as
`DockTabControl::resize_window`, changes the rect and so the `Window`'s `size`,
which asks the host with `WindowRequest::Resize`; either way the dock writes the size of the host's
`Resized` reports into the window's rect (less the grips) and keeps each
`Placed` report's `HostPlacement` in the `DockState`
(`window_placement`, `set_window_placement`), so it is saved with the layout,
handing it back as `WindowPlacement::Host` when the window opens again (after
the dock was stacked, say, or when a restored layout reopens it). A window opened with no placement kept (floated from a tab's
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

A tab's panel is built the first time the tab is shown and belongs to the dock
rather than to the pane showing it: the pane holds a `Portal` pointed at it, so
the panel keeps its nodes, its scroll position, its caret and its state when
the tab is hidden behind another, dragged to another pane, or floated into a
window. A panel no pane is showing is laid out by nobody, so it costs nothing
and a screen reader does not read it. Closing the tab is what removes it.

Pressing a pane's or a window's own chrome - its bar, a tab, a grip - gives the
keyboard to the tab it shows rather than taking it: the bar is an `Interactive`
with `press_focus=false` (guides/beui_keyboard.md), and the dock (`dock/keyboard.rs`) puts the focus in the
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
Each window and each docked pane claims those presses (`claim_modifiers`), so
neither the tabs nor the content see them. The code is
`dock/modifier_drag.rs`; linux-desktop sets Super, and the demo's Docking page
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
no input. The pointer
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
it to the host in its `WindowRequests` (guides/adding_a_plugin_editor.md). An
adapter reads raw motion only while `Runner::window()`, the state it was
shown, holds the pointer, and while it does sends no absolute `PointerMoved`
(winit, web and drm alike), since a held pointer's position means nothing.
The winit adapter lets the grab go itself the moment its window loses focus
and takes it back on focus only if the shown state still asks, since X11's
and macOS's grabs outlive focus. A browser drops its lock by itself, on blur
and on Escape (which the page never sees), and refuses one asked for without
a user gesture: the web adapter (its `lock::Lock`) reports either, while the
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
example. The DOM renderer cannot run a `Draw`, so it fills a drawing's
rectangle with magenta instead; a crate that draws with the gpu does so behind
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
        <List spacing=8.0>
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
winit retitles the window and the web adapter retitles the browser tab and
colours the page's body to match; Android follows `system_bars`; DRM, headless
and plugins ignore the title. A plugin pane has no title of its own and the
plugin protocol carries none: `BeuiApp::View` returns the editor's own `Root`,
whose `clear_color` the plugin's surface is cleared to (the theme's background
unless the editor names another role), and block-editor-beui puts the frame's
chrome around that root's child. The pieces drawn over something of the host's
(a preview, a creation dialog, artifact settings) are rooted in a transparent
one. Holes are cut from that colour, never left by it: a child editor's `Embed`
punches its rectangle, and a painting with holes records the colour as its first
primitive so they cut it. Documents drawn over another one (the inspector's panels, the
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
  (`crates/beui-adapter-android`, with its Java in its `android` directory): the app's
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
  placed at their translations, and each group in it (a clip, a scroll, a
  mask, a scale) is an element of its own; a display whose `Rc` is unchanged is not
  looked at, so scrolling moves the rows' elements and rewrites nothing
  inside them. Text is measured and drawn with the browser's fonts
  (`beui-font-browser`), and the browser fetches the icon font from the URL
  `icons_font` names. A `Drawing`, a `Punch` and a `Filter` draw nothing there.
  `crates/beui-web-demo` is the demo with both renderers, DOM first:
  `./scripts/buck run //crates/beui-web-demo:web` serves it on
  http://127.0.0.1:8070.

`beui::run_with` takes `RunOptions` (app id, starting size) where
`beui::run` takes only the app; both load wgpu, and
`beui::run_with_renderers` takes the `beui::WindowRenderer`s to load instead
(block-app's opens its device itself, to import Wayland clients' buffers), and
`beui::run_on` runs on an adapter of the caller's choosing, such as
`beui_adapter_drm::Drm`, which drives the displays and input devices itself, or
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
  drops the app right after it, while the GPU device and the adapter's displays
  are still there; no frame runs after it. Whatever the app keeps in a
  `thread_local!` must be taken out in `exiting`: the main thread's
  thread-locals are dropped only by `exit()` after `main` returns, in no set
  order, after the GPU and the rest of the runner are gone, and dropping GPU or
  reactive state there aborts or crashes the process (block-app's
  `Shell::exiting` takes down the Wayland compositor, the plugin runtimes and
  their worker threads, and the screens this way).
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

`RunOptions::automation` hands the runner an `automation::Inbox` of commands
from a driver (guides/running/drive.md); the native run functions fill it
from `BEUI_AUTOMATION` and pick their adapter and renderer from `BEUI_ADAPTER`,
`BEUI_SPACE_ADAPTER` and `BEUI_RENDERER` (`beui_core::runner::environment`), and
block-app's web build fills it from its `automate` export. The runner keeps the
accessibility tree (`Context::keep_accessibility`, which a bare `Context` and a
headless plugin do by default and other runners only for a driver) and the action registry's actions, takes
the next command at the start of a frame, turns input commands into `Event`s
already in points, which skip the adapters' intake (`automation/keys.rs` names
the keys), and answers once a frame
asks for no other and the app is not `busy`. The driver acts on the window
only through the `Platform` it is given, as the app does: `clipboard` copies
and pastes through it, `resize` asks `Platform::resize` (which only a
headless window answers), and `state` reads the size, scale and fullscreen
the window last reported. While a driver is attached the runner hands it the
app's file dialogs instead of the platform, so `upload` and `dismiss` answer
them in any window, and it keeps the focus `focus` and `blur` gave. The
headless adapter's window is `automation::Simulation`, a `Platform` like any
other: it keeps a clipboard, grows to its screen for fullscreen, and reports
its size, scale and fullscreen through its `WindowReports`. beui's own tests
drive one the same way, with a renderer that draws nothing. `Renderer::capture` reads the presented
frame back for a screenshot.

A window nobody can see is `Runner::set_occluded`: its frames still run (layout,
paint, accessibility, the driver), but the runner hands each one's output to
`Renderer::note` instead of `prepare` and presents nothing. `note` keeps only
what the frame damaged, so the wgpu `Target` draws the union of every unseen
frame's damage into its retained frame the next time it does draw; a renderer
without `note` prepares as usual. The headless adapter is always occluded, so it
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
so pasting and composition behave like any other input on the page. Like the
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
placement, size }`, `BeginMove(window)`, `BeginResize(window, WindowEdge)`
(the host then moves or resizes it, as an OS does after winit's
`drag_window`), `Resize { window, size }` (the app wants another size; the
host answers with the size it gives) and `Close(window)`. A `WindowPlacement` is `AtPointer`,
`Over { window, at }` (its top-left corner at `at` in that window's points, where
the app would draw it inside that window), or `Host(HostPlacement)`: a value only the host
understands (a screen position, a 3D pose), which the app stores and hands
back unchanged. The host answers with `WindowReport`s pushed as
`Incoming::Window`: `Resized { window, size }` in points, `Placed { window,
placement }`, `Closed(window)` and `Focused { window, focused }` (the host's
window focus, as a window manager moves it on a click or to a window it
opens). The runner applies them before the frame
lays out, and `Context::window_size` and `Context::window_placement` read
them. `Context::opens_windows` says whether anything can open windows; where
nothing can (every platform adapter on its own), a window is shown inside the
main one instead.

An app makes a window with the `Window` component (`beui::primitives`):

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
`Scaled` and `Closed`, after which the window stays shut until the app closes and opens it
again. Elsewhere it is an `Overlay` at `at` (`Placement::At`,
`LayerPolicy::Pass`, no focus trap) holding its content at `size`, which is
how floating windows looked before there were windows, so an app looks the
same either way. `WindowHandle` (also `use_window()` inside the content)
carries the `id`, the shown `size` and `is_window()`, which says which of the
two it is. `begin_move()`, `begin_resize(edge)` and `close()` ask the host and
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
(`window_focused` in `dock.rs`) focuses its pane and, unless the keyboard is
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
`WindowReport::Scaled { window, pixels_per_point }` (the main window
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
holding the focus, and the rest of `WindowState` stays on the main window.
The accessibility tree stays one tree, with each window's nodes in its own
coordinates.

Tests host windows with `testing::Stepped::hosting_windows` (block-ui-test's
`DocumentTest::hosting_windows`), whose host opens every window asked for at
its size and reports `Resized` and `Placed` back the next frame.
`frame_in(window, inputs)` sends input to a window, `take_window_requests`
reads what the app asked, `report_window` reports as a host would, `windows()`
lists the open windows and each one's latest `WindowOutput`, and
`DocumentTest::snapshot_window(name, id)` takes a window's painting.

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
release in the window the press began in). The drag board looks for drop
targets only in the window the `DragPoint` names. A host signals that the pointer, holding a
button, is over no window by sending `PointerLeft` (moves and the release then
keep going to the window the press began in, outside its rect): the intake
turns that into `Event::PointerOutside`, which sets `Pointer::outside` until
the next press. While it is set and the pointer is outside its window's
reported size, the board (`Board::set_outside`, each frame) drops nothing:
the target the pointer left hears `over(None)` and the drop calls the
`Outside` callback given to `Board::begin` with the last point (inside or
out, so a drag that leaves on its first move still has one), which
`Draggable` passes on as `on_drop_outside`. Where nothing opens windows the
flag is ignored, so dragging out of a platform window still drops at the last
point inside. `ui_types::Input` has no window: it is the
plugin protocol's input too, and a plugin region is always one window.

A `beui_core::runner::WindowHost` sits between one platform window and an
app with many windows, on any adapter. It goes in `Launch::host`, or wraps an
adapter as a `HostedAdapter { adapter, host }`. The runner calls it at three
points:

- `setup(&Setup)` after the renderers load and whenever the GPU is replaced;
  `setup.get::<beui::GpuSetup>()` is the wgpu device, queue and surface format,
  and `setup.get::<Context>()` the app's context, for laying out text and icons.
- `input(&mut Vec<Incoming>, &HostView)` with everything that arrived for the
  frame (the platform's input and a driver's), before the app sees it: it may
  rewrite, drop or add entries, name each pointer event's window in that
  window's coordinates, and push `WindowReport`s. `HostView` is the platform
  window's size in physical pixels, the pixels per point and the frame's time:
  the runner reads the clock once, before `input`, and the app's frame and
  `frame` see that same time (`Context::now`).
- `frame(&FrameOutput, &HostView) -> Hosted` after the app's frame: it reads
  each window's output and requests and returns the one `WindowOutput` the
  platform window shows, which the runner hands to the platform
  (`show_window`) and the renderers in place of the app's main window, with
  a `repaint_after` that keeps frames coming while the host animates.

With a host, the main window's size is the one the host reports, not the
platform window's, and the app gets no safe area. A wgpu host draws each
window into a texture of its own and composites them through a `Drawing`
(`beui::drawing`, whose `Draw::prepare` has the device, queue and an
encoder) in a `Layer::Shape(Shape::Drawing { .. })` covering the returned
window. The native run functions (`beui::run_with`, `run_on`) wrap their
adapter in a host under `BEUI_ADAPTER=space` (`adapt` in
`crates/beui/src/window.rs`).

### Space: the VR simulator host

`crates/beui-adapter-space` is a `WindowHost` that shows an app's windows as
panels in a first-person 3D scene, a VR simulator that runs on any wgpu adapter.
`BEUI_ADAPTER=space` turns it on for any native or headless (`:dev`) run: `adapt`
wraps the adapter `BEUI_SPACE_ADAPTER` names (the platform's own by default, or
`headless`) in `HostedAdapter { adapter, host: Space::new() }`.
Everything it does is in the host, so no platform adapter knows about it, and
all its 3D (`glam`) stays in the crate.

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
  view's width, and reports that size (`WindowReport::Resized`). A host made
  with `Space::keeping_main(path)` keeps the main panel's placement in that
  file instead of reporting it: it opens the panel there when the file holds
  one it can read, and writes the file whenever the panel's pose, scale or
  bend differ from what it last wrote while no carry, resize or chrome gesture
  holds it, so a gesture writes once, when it ends. `adapt` keeps it in
  `space-main-panel.txt` under `RunOptions::config_directory()`, and only on
  the platform's own adapter, so headless runs and tests never touch it.
- Windows: the runner sets `Context::opens_windows` under any host, so every
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
  none, turned to face the viewer (yaw only). A window that opens takes the
  focus (see Focus); a window asked for again keeps its panel and leaves the
  focus alone. `Resize` sets the panel's size,
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
  cursor where its ray hits; `PointerMotion` moves it from where it is drawn by
  as many screen pixels (no further than `CURSOR_MARGIN` points inside the
  edge); either saves it `CURSOR_LIFT` (1%) of the distance in front of what
  it hits, or on a miss at the last hit's distance (`OPEN_DISTANCE` before any
  hit). Turning the head and walking leave it at its point, so it slides
  across the view; one that would leave the view is drawn, and points, just
  inside the edge without moving its point, so turning back finds it where it
  was, and moving the mouse from the edge starts from where it is drawn. While
  a panel is carried the cursor stays on its grabbed point. Moving onto another
  panel just names the new window, held button or not (no `PointerLeft`), so
  a drag goes on across windows. Moving off every panel sends the last window
  `PointerLeft`; while a button is held, moves and the release then go to the
  window the press began in, at the point the ray meets that panel's plane,
  outside its rectangle. That is how an app tells a drop on nothing (the dock
  floats a tab dropped there into a window opened `AtPointer`). Presses that
  miss reach nothing.
- Head: holding Ctrl and Shift (either side) steers. Ctrl or Shift alone
  still reaches the app; when the other joins it the app sees both keys let go and its modifiers cleared. While steering
  `PointerMotion` turns the head (yaw, and
  pitch clamped short of straight up or down) and W, A, S and D, by their
  place, walk along the floor relative to where the head faces, timed from
  `HostView::now`. No key pressed while steering reaches the app, nor its
  release, and clicks reach it without Ctrl or Shift, so an app in space never
  sees a Ctrl+Shift chord. Keys, modifiers and `PointerMotion` sent by `drive`
  go through the same rule (a driver's motion is in points, scaled to pixels
  first), so `drive keydown ctrl; drive keydown shift; drive keydown w` walks
  and `drive motion 200 0` while Ctrl and Shift are held turns the head.
- Carry: on `WindowRequest::BeginMove` (for any window) the panel keeps its pose relative to the
  head, with the grabbed point on the cursor's ray at a fixed distance, so
  moving the mouse or steering carries it. As the mouse swings the ray around
  the viewer the panel turns with it, by the ray's change of yaw about the
  grabbed point (`Carry::aim`); steering turns it with the head instead. From
  the moment it is grabbed it also turns, over `TURN_SECONDS` of
  `HostView::now` (smoothstep, `carry::Turn`), from that carried rotation
  to facing the viewer from its centre (yaw only, about the grabbed point:
  `carry::facing`), and then keeps facing them, so letting go turns it no
  further. Scroll changes the distance and does
  not reach the app; the app sees no pointer moves meanwhile. While it is no
  farther than where it was picked up (`Carry::lifted`, `Space::lifted`), the
  carried panel draws over every other panel whatever their depth, and the
  cursor's ray picks it first (`Space::pick`); scrolled farther than that, depth
  orders it like any other, so pushing it away puts it behind a nearer panel.
  Letting go of the
  button that was down when it began (or, if none was, the next click) ends it:
  the app gets that release where it grabbed, the panel faces the viewer
  (yaw only; only a carry let go mid-turn still turns), and the host reports `WindowReport::Placed` with the panel's placement,
  `space X Y Z QX QY QZ QW S B`: its position, rotation, `metres_per_point`
  and `bend` (`Panel::placement`, read back by `Panel::placed`). The app keeps
  it (the dock in its saved layout) and hands it back when it opens the
  window again.
- Resize: on `WindowRequest::BeginResize(window, edge)` (`resize::Resize`)
  the cursor's ray meets the panel's surface as it was, and that edge or corner
  follows it while the opposite one stays put, no smaller than
  `resize::SMALLEST`. Each new size is reported as `Resized` at once, so the
  app lays out at it the same frame; the app sees no moves meanwhile, and the
  carry's rule for letting go ends it, with a `Placed` for the panel's new
  centre.
- Sharpness (`scale`): each frame `Space` gives every panel the pixels per
  point it shows at on screen: the frustum-clipped panel's nearest depth
  (its outline sampled along the top and bottom edges, so a bent panel's
  nearer sides count), as screen pixels per point there (so a panel scaled
  up by its chrome gets more), times `SUPERSAMPLE` (1.25, so the main
  panel's first 80% view is drawn at exactly the platform's scale), rounded
  up to a step of `STEPS_PER_DOUBLING` (4) per doubling, between
  `LEAST_SCALE` (0.5) and `MOST_SCALE` (4) and no larger than
  `LARGEST_TEXTURE` (4096) texels a side. It keeps a scale until the panel
  needs more, or `KEPT_STEPS` (2) steps less, and a panel out of sight keeps
  its own, so walking about redraws a window only now and then. A new scale
  waits until the panel has wanted it for `SETTLE` (250 ms of
  `HostView::now`, the host asking for a frame when the wait is over), and a
  panel being carried, resized or reshaped by its chrome keeps the one it has
  until the gesture ends, so a scale never changes every frame. A change is
  reported as `WindowReport::Scaled` (`Space::scale` reads it), which is
  only the window's raster: the app repaints the window at it without laying
  anything out, so its text stays where it was and only gets sharper or
  softer. A new window takes its scale at once.
- Drawing: each window's display is drawn by its own
  `beui_renderer_wgpu::Renderer` into a mipmapped texture at its output's
  `pixels_per_point`, only when that window's output changed, repainting only
  its damage after copying its `moved` scroll inside the texture. A window that
  changes size gets a new texture but keeps its renderer, whose pipelines and
  glyph atlases cost far more to build than a resize step's whole frame. It is sampled with
  trilinear and anisotropic filtering and a level-of-detail bias of -0.5
  (`SHARPEN` in space.wgsl), so a texture up to 1.4 times the panel's size
  on screen is read from its sharpest level;
  then a 4x multisampled, depth-tested pass draws a floor grid and the panels
  (their backs grey; nearer panels hide farther ones by depth, except a lifted
  carried panel, drawn last with its depth test off), and a
  `Drawing` copies it texel for texel into the platform window. A frame is composed again only
  when the head, a panel, the cursor or a window's output changed. A panel is
  a strip of `render::SEGMENTS` columns bent in the vertex shader. The
  `Drawing` also paints (`picture.rs`, `Paints`): each panel's and its
  chrome's paintings placed onto the projected surface cell by cell, far to
  near, which is what the crate's painting tests compare.
- Cursor: the host asks the platform for `CursorIcon::None` and draws its own
  in the scene, last and over everything, as a quad facing the head with its
  hotspot on the cursor's point (`Space::cursor_at`), or when that is out of
  view on the clamped ray, just in front of what it hits there. It goes only
  when the pointer leaves the platform window. Its size grows with
  distance (`Space::cursor_scale`, metres per point) so it covers as many
  pixels as it has points at any depth. It shows the icon that the window
  under it, holding the press, or being carried or resized asks for
  (`Space::cursor_icon`), a resize, grab or pointing hand over chrome, an
  arrow over nothing. `cursor::Sprite` paints that
  icon with `beui_core::drawn_cursor::paint`, the same drawing the inspector's
  mouse simulation shows on a touchscreen, through a `Context` of its own with
  only the icon font, and the compositor draws it into a texture like a window
  only when the icon or the scale changes, under a `WindowId` of its own. A new
  icon repaints into the same texture with the same renderer; building a
  renderer for each icon cost about 30 ms a switch.
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
  (`chrome.rs`), sized in metres so they keep their size however the panel
  is scaled: an L bracket just outside each corner, and under the bottom edge
  a pill with a move button (`ICON_MOVE`), a rotate button (`ICON_ROTATE_3D`),
  a roll button (`ICON_ROTATE_CW_SQUARE`) and a curvature button
  (`ICON_SPLINE`). They are faint (`FAINT`) except on the focused panel, which
  keeps them lit wherever the cursor goes until another press moves the focus.
  `chrome::layout` places the parts (each bracket's two arms, `SLOP` wider,
  and the buttons) and `chrome::part_at` finds one at a window point outside
  the window's rectangle. Picking is by depth alone: the cursor's ray takes
  the chrome on a panel's extended surface when it is nearer than any panel
  (`chrome::pick`), focused or not, so a bracket of a window popped out in
  front of another takes the click over the content behind it; the arms'
  small margin is what keeps a dock window popped out over the main one from
  covering its tab bar. A driver's point outside its window picks too. Input
  on chrome never reaches the app (the window under it last gets
  `PointerLeft`), and the cursor shows what a part does. While an app holds a
  button, chrome is ignored. Dragging a bracket scales the panel
  (`Gesture::Scale`): `metres_per_point` grows or shrinks with how far the
  cursor goes along the diagonal from the opposite corner, which stays put,
  between `SMALLEST_SCALE` and `LARGEST_SCALE` of where it began, so the
  window keeps its points, gets no `Resized` and keeps its aspect. Pressing
  the move button starts the carry a title bar's `BeginMove` starts
  (`Space::begin_carry`, `Gesture::Move` while it lasts, for the cursor and
  the held look), ended by letting go. Dragging the rotate button turns the
  panel about its centre (`Gesture::Turn`, `chrome::turned`): the ray's change
  of yaw times `TURN_GAIN` about the vertical, of pitch about the panel's own
  horizontal, measured from where the drag began, so it never rolls the panel
  however the cursor circles. Dragging the roll button rolls it about its
  normal through its centre (`Gesture::Roll`, `chrome::roll_angle`,
  `chrome::rolled`) by the change in the cursor's angle around the centre on
  the panel's surface as it was, so the button, below the centre, stays under
  the cursor like a dial's knob. Scale, turn, roll and slider drags report
  `Placed` when let go. A carry later turns the panel back to face the viewer upright
  (`carry::facing`), so it overrides rotation and roll. The curvature button
  opens a slider in the pill (`Chrome::slider`), and clicking or dragging
  along it sets `bend` from 0 to `MOST_BEND` about the panel's centre. Scale,
  roll and slider drags map the ray onto the panel as it was when they began.
  Each panel's chrome is a display list (`chrome::output`, icons laid out
  with the app's `Context`, which `Setup` provides) drawn into a texture of
  its own only when its `Look` changes, on the extended surface `LIFT` in
  front of the panel, after every unlifted panel and not at all on a lifted one.

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

- `beui::unstyled::software_keyboard`: the layouts as Rust data
  (`layout.rs`: `QWERTY`, `GREEK`, `NUMPAD`, built with `KeyDef`'s const
  builders; there is no user configuration), the Fn, superscript and subscript
  tables (`tables.rs`), the compose table (`compose.rs`, generated from X.org's
  `en_US.UTF-8/Compose`), and `Engine`, the pure gesture and modifier state
  machine its tests drive (`engine/tests`). The `SoftwareKeyboard` component
  lays the keys out in rows of `Percent` weights inside one forwarding
  `Interactive` with `press_focus=false`, so every finger reaches the engine as
  its own pointer and pressing a key never moves the focus. It hands each key's
  appearance to a `cap` render function and the emoji layer to `emoji`.
- `beui::styled::SoftwareKeyboard`: the keys painted with theme tokens under a
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
  height off the bottom of the screen the focused field is on, the way the
  mouse simulation's strip is taken: a single screen's document is laid out
  above it, and on several screens that screen is shortened in what
  `use_screens()` offers. The keyboard covers a share of that screen's
  height, `DEFAULT_SHARE` until `Context::set_keyboard_share` says otherwise
  (at most `MAX_SHARE`; block-app sets it from the display settings' on-screen
  keyboard height), and its keys stretch to fill it. A platform's own keyboard
  ignores it.
  The keyboard is a document of its own, shown after
  the app with the pointer only while a press began on it or the pointer is
  over it, and with no keys.

There is one way to ask for an on-screen keyboard whatever shows it:
`toggle_keyboard()` (or `Context::toggle_keyboard`) counts up
`WindowRequests::keyboard_toggles`. Where beui's keyboard is enabled it opens it
until it is closed; otherwise the platform answers the change in
`Platform::show_window`: Android shows or hides its keyboard, the web focuses
or blurs its hidden textarea, and a plugin passes it to its host as
`WindowRequests::keyboard_toggles`, where block-app toggles its own. The
desktop bar's keyboard button, the inspector's Sim tab button and the mouse
simulation's keyboard button all call it. While the platform reports that no keyboard is attached
(`WindowReports::keyboard_attached`, which only `beui-adapter-drm` sends, from
libinput devices that have an A key; every other platform counts as attached),
it opens whenever the focused field's `ImeArea::keyboard` turns `Shown` or its
`asks` changes, and closes when the focus leaves for something that types
nothing. Its close button (`close_keyboard()`) closes it until a field asks
again, so tapping a field brings it back. Its keys reach the app as a keyboard
would: the engine makes `Input::Key`s with evdev codes and real modifier
presses (`beui_core::app::automation::keys`, which `drive type` and `drive key`
use too) and `Input::Text` for what no key types, `Context::type_events` puts
them in `FrameOutput::typed`, and the runner pushes them as the next frame's
input, so a beui field, a plugin and a Wayland program all take them
the same way. Paste as plain text asks for a `PasteRequest::Typed`, whose
answer the runner types out key by key. The Inputs page of beui-demo shows it in place, and the
inspector's Sim tab has a button that opens it over the inspected app.

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
keyboard button calls `toggle_keyboard()`, so it opens whatever on-screen
keyboard the platform has (see The on-screen keyboard); fingers that land on
beui's keyboard pass through to it as touches rather than moving the cursor.

The strip takes its room out of the window rather than
covering it: `Document::show` asks the simulation how tall it is, trims that off
the bottom, and lays the document and the inspector panel out in what is left,
the way a phone's keyboard pushes a page up, and beui's keyboard opens above
it. Nothing is drawn over content that is still live, so the bar is opaque.

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
window size or set of screens lays the tree out again from its root and every
node offered the space it had keeps its measurements, placement and paint; only
a new scale, new fonts or a new context lay out everything. Two nodes build
during it:

- `LayoutBuilder` hands its render prop a `ReadSignal<Vec2>` of the size it is
  laid out at, on each axis its parent bounds (an axis the parent measured it
  unbounded on keeps its last value). When the walk reaches it at a new size it
  writes the signal and flushes its content's scope, then lays the content out
  in the same walk, so a builder inside the content it just built is reached
  and built in that walk too. Measuring it never rebuilds anything: a parent
  that tries several sizes is answered by the content built for the last size
  it was laid out at. If the rebuilt content measures differently, its
  ancestors are laid out again once. Use it for a component that needs the
  space it was given, such as one that wraps text or switches between a row
  and a column, and give it that space from its parent (a stretch, a fixed or
  percent size) on the axis it reads.
- `VirtualList` shows each row its window reaches that it does not have yet,
  recycling a released row or building one,
  as the walk reaches it, measures it and carries on, so one walk fills the
  window. A list laid out at the length it measured follows that length as
  its rows measure, rather than stopping at the estimate it was given, and
  rows that measure shorter at the end of the list pull in rows above the
  window to take their place. When its length changed it asks with
  `Document::invalidate_after_layout` for the next layout to measure it again.

`PlacementBuilder` hands its render prop a `ReadSignal<Rect>` of where it was
placed, in the document's coordinates, written after each layout with the
other placement reports. Use it for anything that positions content in
document coordinates (a canvas camera, an overlay anchored at a point). Moving
it does not lay its content out again; only the effects that read the rect run.
Content that needs the rect to lay itself out takes a second layout. A
component that only needs where on itself it was pressed reads the `local`
position pointer events carry instead.

The rule these rest on is the one container queries settle on: a component
may query the space it was given, but must not be the thing that decides that
space on the axis it queries.

Everything else layout decides is told to the tree after the walk: a `Scroll`'s
position, `component_placed`, `node_rect` and `node_placed` for watching other
nodes, and `Document::on_laid_out`, the callback block-app waits for its plugins
in. A node with something to report queues it with
`Document::report_after_layout`, and one whose own size changed while it was
laid out with `Document::invalidate_after_layout`. The reports run together in
one entry; whatever they changed is laid out again, which only walks the
nodes that changed and their ancestors, until nothing changes, so a frame is
never painted from a layout the tree no longer has. A frame lays out at most
four times: when the reports after the third still change something, the
fourth layout holds its own reports back for the next frame, the frame is
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
where on itself it was pressed reads those rather than its rect.

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
framework builds the view once, keeps the `Document` it produced, and shows it
every frame.

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

Nothing in an editor polls. What the host sends between frames is recorded
by the framework and delivered at the top of the next frame as signals on
the `Editor`: `replies()` ticks when the host answers a request (a file or
block pick, a fetch), and `on_reply(f)` runs `f` each time;
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

A template the manifest marks `"dialog": true` is made through `BeuiApp::Creation` instead, one
more component that the framework builds a separate document of
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
6. Return the root base node directly so component state and node modifiers
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

Pointer input takes one path. Each mouse button and each finger is a pointer
of its own, and the document sees their downs, moves and ups in the order they
happened (`InputState::pointer_events`), so a press, a move and a release that
share a frame are three steps. A down is hit tested once (`interact::hit`):
the layers of `Document::layers()` are taken from the top, by the rules of
their policies, and in the first one the pointer lands on, the hit is the
topmost `Interactive` under it together with every `Interactive` around it,
its path. The same walk collects the layers the press is outside of, which it
then asks to close. Any other node is transparent, and so is an `Interactive`
at a point its `hit_at` turns down, along with the catchers around it up to one
that captures or forwards; a passing layer whose content turns a point down
lets it through to what is beneath.

The pointer keeps what its down reached until it is let go. A modifier claim
(`claim_modifiers`, `claim_at`) is looked for first, from the top of the tree
down and past `hit_at`, and a claimed press reaches its claimant alone.
Otherwise the press goes up the path from the topmost catcher and stops at the
first that captures it (`capture_presses`, or `capture_at` at the points its
callback accepts). Every catcher it reached that
handles presses arms: it hears `on_press`, `on_drag` wherever the pointer goes
(once in the frame of the press too), and `on_click` when it is let go over the
catcher, or for a finger, let go without having been dragged. The press then
focuses the innermost focusable catcher it reached, or clears the focus;
a claimed or captured press that reached no focusable catcher leaves the focus
where it was. A
secondary press reaches every catcher on the path that takes one, and a middle
press the innermost that pans, autoscrolls or forwards.

A finger drag is settled when the finger leaves the tap slop: the innermost
catcher on its path that takes a drag that way (`touch_drags`,
`touch_drag_axis`, or `on_scroll_drag` along its `scroll_axis`) wins it,
every armed catcher that does not hold that drag hears `on_cancel`, and the
winner's `on_scroll_drag` hears the rest. A claimed finger drag stays with its
claimant. A second finger, a long press and a lost window focus cancel the
press the same way.

Hover is the path under the mouse, worked out again every frame, so a catcher
hears `on_hover_change` when it joins or leaves the path whether the pointer
moved or the layout did. The cursor is the innermost armed catcher's, else the
innermost hovered one's. A wheel turn goes to the innermost catcher on the path
under where the pointer was when it turned that takes it along its axis, unless
a forwarding catcher comes first (one with `forwards_wheel=false`, such as the
on-screen keyboard, forwards only its presses and lets the wheel pass to what is
under it), and stays latched to it for
`WHEEL_LATCH_TIMEOUT` while the pointer holds still.

Parts a catcher draws outside its own rectangle, such as the touch selection
handles of `unstyled::TextArea`, take their presses with `claim_at`. Paint
order is the tree's order, so paint such parts from a node that comes after
whatever they must cover, or from a `CanvasItem` with `clip=false`, which a
canvas paints without cutting it to its own rectangle. Those handles are carets
with a handle, which `Text` paints after its text; a single-line area paints
them from an unclipped item after the text's.

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
`text.caret_rect(..)`), which answers `None`, `false` or nothing for a node of
another kind or one that is gone. Where nodes were laid out is a `Placements`
handle from `placements()`, which can be kept and read later, even by a memo a
test reads outside the document. The rest are functions: `focused_node`,
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
own signal, a component wakes only for the colors it actually uses. Radii and
spacing are constants in `styled::theme`. Add a field to `Theme`,
with a value in every built-in theme, when a color is part of the theme rather
than unique to one component.

Styled controls must visibly expose keyboard focus, and must keep labels and
accessible roles stable when visual state changes. Use glyphs from `beui::icons`
in a `styled::Icon` inside a `styled::Text`; do not use Unicode characters as
ad-hoc icons. The [keyboard guide](beui_keyboard.md) records the expected
behavior for each control family.

### Text

Styled text is written in tokens, never pixels or colors. `styled::Text`,
`styled::Span` and `styled::Icon` take the same children as the base ones and
resolve their tokens into a base `Text`'s `font` and `color`:

- `size`, a `TextSize` on a scale from `Xs` to `Xl5`; `TextSize::px` is the
  point size, for the rare layout that must measure against it.
- `weight` (`Weight::Normal`, `Weight::Bold`), `family` (`Family::Sans`,
  `Family::Mono`) and `italic`.
- `color`, a `TextColor` role (`Default`, `Muted`, `Faint`, `Accent`,
  `OnAccent`, `Danger`, `Warning`, `Success`) resolved through the nearest
  theme, so the text follows a theme change. `TextColor::Custom(color)` is for
  colors that are data rather than design - a user's label color, a map
  marker, a terminal's ANSI palette - and a memo that picks a color by state
  returns a `TextColor`.
- `style`, a `TextStyle` preset: `BODY`, `CAPTION` (small and muted),
  `HEADING`, `TITLE`, `DISPLAY`, `CODE` (small and monospace) and `PARAGRAPH`
  (muted); give a paragraph `wrap=Wrap::Words` as well. A prop given beside a preset
  wins over it.

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
(`beui-font-freetype/src/assets/icons/lucide.ttf`, committed unmodified from
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
Hack for monospace and Noto Emoji as the fallback.

`TextStyleProvider` gives every styled text created within it a style its own
props override, the way a disabled section mutes everything in it. It is
context, so it reaches what is created inside it, which is not always what is
laid out inside it: text a render callback builds for an overlay takes the
provider around where the callback was written. A `Span` or `Icon` written
inside a `Text` takes what it does not set from that `Text` instead.

Raw pixel sizes and `Color32`s belong to base `Text`, for text that is drawn
rather than designed: labels on a zoomable canvas, a terminal, a game board.

### Themes

`styled::Theme` holds the color tokens. The built-in themes are `Theme::DARK`,
`Theme::OLED` (dark on pure black, so OLED screens leave most pixels off),
`Theme::LIGHT`, and `Theme::EPAPER`, one for each `ThemeKind`; `Theme::of(kind)` picks one,
and a theme's `kind` says which it is, for code that must tell them apart without comparing
colors. What dims the app under an overlay is a theme colour too: `scrim` under a dialog,
sheet or dock dialog, `scrim_soft` under the command palette and launcher, which leave
the app readable, and `scrim_strong` under `Fullscreen`. Styled components that no provider covers use the document's theme, the one its
preferences choose (Preferences, above); `Theme::of(document.preferences().theme)` reads
it, for example to pick an app's clear color.

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
`node_rect`, `PlacementBuilder`'s rect, the rect `interact` is handed and a `CanvasView`'s
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
an adapter does: through the real `Runner`, on the headless window's `Simulation`
(`Stepped::new`) or on a `Platform` the test supplies (`Stepped::on`), with a gpu-free
`testing::Screen`, the clock stopped, and one frame interval per frame.
`testing::Showing(document)` is a `Document` as an app. Its gestures are `drive`'s commands,
run by the same automation engine: `click(Target::id("wheel").at(offset))`, `drag`, `down`,
`up`, `hover`, `scroll`, `zoom`, `pinch`, `fingers`, `type_text`, `key`, `key_down`, `wait`
and `settle` are typed forms of them, a `Pos2` is a target too, and `ask(&["back",
"0.5"])` sends any other. Each settles as `drive` does, unless the test turned that off
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

## HTML strings

`beui-html` builds ordinary HTML elements, laid out by the browser's CSS. It
uses `beui-tree` components and control flow, independently of the GUI's paints
and layout and of `beui-renderer-dom`.

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

### Typed CSS

Use `beui_html::tw` style components in an element's `css` prop. A component
prop can take a view directly: `prop=<Component />`, or a run of children with
`prop=<>...</>`. This syntax works on any component prop, not just CSS.
Fragments `<>...</>` also work anywhere children fit, including at the root
of `view!`; they group children without a backend node. Empty fragments
`<></>` build empty children. Expressions, reusable components and control
flow work inside fragments.

The compiler grammar lives in `beui-macros/src/lib.rs`; the source formatter has
its own grammar in `fix-rust-source/src/views.rs`, using lossless source tokens.
Changes to view syntax must cover both grammars and their tests.

```rust
use beui_html::{elements as html, tw, view};

view! {
    <html::Html>
        <html::Head><tw::Styles /></html::Head>
        <html::Body
            css=<>
                <tw::FontSans />
                <tw::BgSurface />
                <tw::TextForeground />
            </>
        >
            <html::Button
                css=<>
                    <tw::InlineFlex />
                    <tw::ItemsCenter />
                    <tw::Px4 />
                    <tw::Py2 />
                    <tw::BgAccent />
                    <tw::TextOnAccent />
                    <tw::RoundedMd />
                    <tw::Hover><tw::BgAccentHover /></tw::Hover>
                    <tw::Md><tw::Px4 /></tw::Md>
                </>
            >
                "Continue"
            </html::Button>
        </html::Body>
    </html::Html>
}
```

Style components return `tw::Style`; reusable compositions return
`Children<tw::Style>`. HTML and style children are different types. `Show` and
`ForEach` work with both; removing a style contribution removes its declarations
on the next serialization. Static styling needs no signals or effects.

Put one `<tw::Styles />` in the document head, or at the beginning of a fragment.
Serialization first collects CSS from the mounted tree, including current style
branches, then fills the outlet. Constructed but unmounted nodes contribute
nothing. A styled tree without an outlet is an error. Output is buffered, not
streamed. Rendering remains string serialization, not browser DOM binding.

Each distinct ordered style composition gets a deterministic generated class;
identical compositions share rules. Existing `class` attributes are preserved.
Within a composition, declarations keep their source order, so later declarations
win at equal specificity. Pseudo-class and descendant selectors still follow
normal CSS specificity. Nested breakpoint and pseudo-class components compose;
`Descendants target=tw::Target::Input` styles matching descendants, including
children added later by scripts. Semantic colors reference `--bht-*` CSS custom
properties with defaults; this theme is independent of GUI beui crates. Utilities
use rem spacing and text sizes. No reset is installed implicitly.

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

The web entry points are generated by `local-tools web-page` through its
`:block-index` and `:demo-index` Buck targets. Their scripts, import maps and
styles live under `crates/local-tools/src/web_page/`.
