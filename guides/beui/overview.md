# beui overview

beui is BE3's user interface toolkit. Read this first for any beui work: it covers what beui is,
the four rules most review comments are about, the crates, and the component layers. The other
beui guides take it from there:

| Guide | Read it for |
| --- | --- |
| `guides/beui/views.md` | writing views: `view!`, children, control flow, layout, the frame's phases |
| `guides/beui/components.md` | what beui already offers: scrolling, lists, overlays, docking, drag and drop, ... |
| `guides/beui/writing_components.md` | developing unstyled, styled and base components; text; themes |
| `guides/beui/apps.md` | beui in a standalone app or a plugin, the runners underneath, the inspector |
| `guides/beui/reactive.md` | signals, memos, stores, effects, scopes, context, controlled state, keyed lists |
| `guides/beui/input.md` | keyboard and input: actions, the command palette, focus, touch |

## What beui is

beui is SolidJS-like. A `Document` owns a persistent tree of nodes, `view!` builds that tree once,
and reactive signals update the nodes in place afterwards. There is no virtual DOM, no diffing and
no per-frame rebuild.

Building the view is not the render loop: build it once, keep the `Document`, and call
`Document::show` with it every frame to lay out, dispatch input, expose accessibility and paint.

A component body runs **once**. The closures and effects it leaves behind are what run again.
Everything else follows from that.

The quickest introduction is the component catalog in `crates/beui-demo`
(`./scripts/buck run //crates/beui-demo:native`): a dock whose Components pane opens a page for
each group of styled components, for the unstyled components painted by hand, and for the base
nodes (frames, text, lists, grids, control flow, interaction, layers and overlays). Every sample
shows the code it was written with: a function marked `#[beui_macros::sample]` (above its
`#[component]`) also gets a `Name::SOURCE` holding its text exactly as written, so a sample cannot
drift from its listing.

## The rules that matter most

### 1. Every function that builds part of a view is a `#[component]`

If a function returns a `NodeId`, or a value built around one such as a `CanvasItem`, annotate it
`#[component]` and name it in `CamelCase`. The attribute:

- makes the function usable as a tag;
- gives it `@test_id`, `@node_ref` and `@sizing`;
- makes `component_state`, `component_accessibility` and `component_placed` available inside it
  (they panic anywhere else);
- gives it a scope of its own, so the contexts it provides reach only what it builds, not its
  siblings.

Nothing checks for the attribute: a plain function that writes a `view!` silently builds into its
caller's scope.

Nodes and effects live and die together whether or not a function is a component: a node belongs
to the reactive scope it was created in, and disposing that scope removes the nodes it still owns
along with its effects (see `guides/beui/reactive.md`, "Scopes and ownership"). A
subtree that must outlive the code that placed it, such as one shown through a `Portal`, is built
in a scope that lives as long as it should.

A component may also return a value rather than a node (a menu item, a choice option). See
"Components that return a value" in `guides/beui/writing_components.md`.

Functions that build no part of a view are ordinary functions: deriving a colour from theme tokens
and interaction state, mapping a value to a label, reading state back out of a built node. See
`box_fill` and `checkbox_checked` in `beui-components-styled`'s `checkbox.rs`.

### 2. A component usually ends with one `view!` and nothing after it

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

Signals, memos, callbacks and handle destructuring go above it. Avoid code below it or a second
`view!` earlier in the body:

- `view!` builds nodes the moment it runs, so a subtree built into a local and then used
  conditionally has already been added to the document whether or not it ends up in the tree.
- A subtree built in one place but parented somewhere else obscures which component's scope owns
  it.

This is not enforced; break it where another shape is clearer. The exception is the base layer,
whose components create their root node directly (`List` calls `create_list` and binds setters
with effects). Above the base layer, one `view!` is the shape.

When part of the tree depends on something, express it in the view (`Show`, `ShowKeepAlive`,
`Dynamic`, `Keyed`, `ForEach`) rather than in Rust control flow around it. A component that is
genuinely two different trees is two components with a `Dynamic` or a `Show` choosing between
them. See "Conditional and repeated content" in `guides/beui/views.md`.

### 3. Call components from `view!`, never their builders

`#[component] fn Button` also generates `ButtonBuilder` and a `Button()` constructor returning it.
That is the machinery `view!` writes into, not an API. Write

```rust
view! {
    <Button label="Save" variant=ButtonVariant::Primary on_click={save} />
}
```

and not `Button().label("Save").variant(...).build()`. The macro is what:

- rejects a missing required prop and a prop written twice, at the line that wrote the tag;
- routes `@test_id`, `@node_ref` and `@sizing` to the right place;
- enforces a component's child arity;
- keeps render props unbuilt until the component calls them.

Hand-written builder chains lose the diagnostics, are invisible to the `view!` formatter that
`./scripts/verify` runs, and read nothing like the rest of the tree. To pass a component around,
hand over a `Render`/`RenderFn` closure that writes a `view!`, not a half-applied builder.

### 4. Use fine-grained reactivity

Bind a signal or memo to a prop; do not read it while building, and do not rebuild a subtree to
show a new value.

- A `Prop<T>` takes a plain `T`, a `ReadSignal<T>` or a `Memo<T>`. The reactive forms install an
  effect that writes just that one property when the value changes.
- A prop holds a signal or memo as it is; only a closure (`Prop::dynamic`) is boxed, so pass a memo
  rather than `move || memo.get()`.
- A `Prop<T>` is `Copy` whenever `T` is, so read one in as many places as needed. A
  `Prop<String>` or other non-`Copy` prop is cloned into each closure that keeps it.
- A callback prop (`Callback<V, R>` or `ClickCallback`) takes a closure or a callback the
  component already has, so a wrapper passes its own `on_change` straight on.
- Do not wrap a prop in `create_memo` just to read it twice. A memo adds a node to the graph, and a
  `T` that is not `PartialEq` cannot go in one at all.

```rust
let color = create_memo(move || paint(value.get()));
view! {
    <Frame color>
        <Text string={value} />
    </Frame>
}
```

The common mistake is a list. Removing every row and building replacements on each change throws
away the nodes, their scopes, their measured text and any focus or caret in them, and costs time
proportional to the whole list for a one-item edit. `ForEach` keyed by item identity reconciles in
place: rows that stayed keep their nodes, and only the bindings reading what changed run.

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

Key by identity, never by content: a key containing the row's text changes whenever the text does,
which destroys and rebuilds the row. Give the row the key and let it read its own item signal.
`KeyedStore` supplies both halves, one signal per item plus the order signal, and `reconcile`
writes only the items that differ.

The same instinct applies elsewhere:

- Derive with `create_memo` so a property wakes only when the derived value changes.
- Split a struct held in one signal with `#[derive(Store)]` so writing one field does not wake
  readers of the others. `use_theme()` returns such a store, which is why a component binds
  `theme.accent.clone()` rather than the whole theme.
- Use `create_selector` for "am I the selected row?", so moving a selection wakes two rows instead
  of all of them.
- Prefer `Keyed` over `Dynamic` when only part of a value decides the shape.
- Use `VirtualList` for a collection large enough that building every row is the cost.

## Crates

`beui` is the facade every app and editor depends on. It re-exports the others under the paths the
guides use (`beui::reactive`, `beui::styled`, `beui::unstyled`, `beui::Document`, `beui::icons`,
...), so code outside beui names only `beui`. The rest depend on each other in one direction:

| Crate | Owns |
| --- | --- |
| `beui-tree` | what makes a tree of components, whatever it is made of: `#[component]` and `view!`'s runtime, `Prop`, child slots (`Children`, `Run`, `SlotChild`, `SlotHost`), and `Show`, `ShowKeepAlive`, `Dynamic`, `Keyed`, `ForEach` |
| `beui-macros` | the `#[component]`, `view!` and `#[beui_macros::sample]` macros |
| `beui-core` | `Document` and `Context`, the retained nodes and their layout, input and its dispatch, accessibility, paint output and damage, the font and image interfaces, the icon codepoints (`beui_core::icons`), the `App` contract, and the `Runner` every adapter drives it through |
| `beui-font-freetype` | `FreetypeFonts`: FreeType and HarfBuzz shaping and rasterizing, and the fonts beui compiles in |
| `beui-font-browser` | `BrowserFonts`: text measured with the browser's own fonts through a canvas, for the DOM renderer; no fonts in the module |
| `beui-view` | `beui::reactive`: `beui-tree` re-exported, with the base components (`Frame`, `List`, `Text`, ...) that wrap core's nodes; `beui_core::tree` is what makes a `NodeId` a child |
| `beui-components-unstyled` | `beui::unstyled` and `beui::datetime` |
| `beui-components-styled` | `beui::styled`: themes and styled controls |
| `beui-inspector` | the inspector, the simulated screen reader and the simulated mouse and keyboard |
| `beui-renderer-wgpu` | the wgpu renderer, its shaders and filters, and presenting to a surface |
| `beui-renderer-dom` | the DOM renderer: the display tree as nested absolutely positioned elements |
| `beui-adapter-winit`, `beui-adapter-android`, `beui-adapter-web` | each platform's `Adapter` and `Platform`: event loop, window or view, input, IME, clipboard, file picker, accessibility adapter |
| `beui-adapter-headless` | `beui::Headless`: draws offscreen with no window or display server |
| `beui-adapter-drm` | Linux displays and input devices through DRM/KMS and libinput; see `guides/beui/apps.md` |
| `beui-adapter-plugin` | the `Adapter`, `Platform` and `Renderer` for one region of a block editor plugin, driven frame by frame by the plugin framework (`guides/plugins/architecture.md`); it depends on the plugin framework, so the facade does not offer it |
| `beui-plugin-input` | conversions between beui's input types (modifiers, key chords, pointer buttons, ...) and the plugin protocol's |
| `block-editor-beui` | the framework a beui plugin editor is written against (`guides/beui/apps.md`) |

Inside the family, crates name each other directly (`beui_core::document::Document`), and the
component crates declare `extern crate beui_view as beui;` so the `::beui::reactive` paths that
`#[component]` and `view!` expand to resolve there.

### How core reaches the crates above it

Core cannot see the crates above it, so it exposes hooks they fill in:

- **Inspector.** A `Tools` object on the document. `beui::reactive::build` installs it
  (`beui::install_inspector`), and `Document::show` hands the frame to it. A document built by
  `beui_view::reactive::build` has none. The simulated mouse is an `InputSimulation` the inspector
  installs on the `Context`.
- **Fonts.** Text is shaped by the `FontBackend` a `Context` is made with. A backend only shapes one
  line (`FontBackend::shape`, text with no `\n`); core breaks and aligns lines once, in
  `RichLayout`, which `Fonts::layout` (and so `Painter::layout`) uses for wrapped, aligned or
  multi-line text. `beui::context()` makes a context over `FreetypeFonts` with only the fonts beui
  bundles, which is what tests use; `beui::system_context()`, which the runners use, adds the
  system's fonts for whatever the bundled ones do not cover. Fonts live in a `FontLibrary` that
  contexts can share and that can be given more fonts later, which lays their text out again.
- **Theme.** The document's theme lives in a typed slot on the document (`Document::extension`);
  `beui::styled::DocumentTheme` reads and writes it.
- **Drawings.** A `Drawing` holds whatever its renderer draws; `beui::drawing` makes one for the
  wgpu renderer.
- **Platforms and renderers.** `beui_core::runner::Adapter`, `Platform` and
  `beui_core::renderer::Renderer` trait objects, which the facade picks; see
  `guides/beui/apps.md`, "Runners, adapters and renderers".

### Building other kinds of trees

`beui-tree` knows nothing of `Document`, layout or painting, so the same components and control
flow can build any tree: a DOM, a 3D scene, a chat message with buttons. Such a tree:

- gives its child type `ChildValue` (where a component's scope lives) and `SlotChild` (how a child
  is kept);
- gives what keeps children `SlotHost` (appending, and filling a slot a `Show` or `ForEach` holds)
  and `IntoSlotHost`, so a component can `children.mount(...)` into it.

The macros need `::beui::reactive` to resolve: `beui-tree` names itself `beui` for its own tests,
and a crate built on it does the same or re-exports `beui_tree::reactive` as a `reactive` module of
a crate it calls `beui`. `@sizing`, `@test_id` and `@node_ref` expand to
`beui::reactive::ListChild`, `with_test_id` and `with_node_ref`, which only `beui-view` has, and an
untyped `Render`/`RenderFn` child or a `Child` prop is `beui::reactive::Child`, a `NodeId`.
`beui-tree`'s tests build a small DOM this way.

## Component layers

beui separates mechanism, behavior and appearance, in this dependency direction:

| Layer | Location and public path | Responsibility |
| --- | --- | --- |
| Base | nodes in `crates/beui-core/src/base`, components in `crates/beui-view/src/components`; re-exported from `beui::reactive` | retained nodes for layout, painting, visibility, focus, pointer input, offset content and text |
| Unstyled | `crates/beui-components-unstyled`; `beui::unstyled` | accessible interaction behavior composed from base components, with no theme colours, typography, borders or spacing |
| Styled | `crates/beui-components-styled`; `beui::styled` | application-ready controls that compose an unstyled control and paint its state with base components and `styled::theme` tokens |

Work at the highest layer that can express what you need:

- **Adding a feature to the app:** compose styled controls with base layout and painting
  primitives.
- **Existing behavior, different appearance:** use the unstyled component and paint it yourself.
  Do not reimplement focus, keyboard, pointer, touch or accessibility handling in a styled
  component.
- **New behavior:** add an unstyled component composed from base components.
- **Tempted to add a base component:** almost always add an unstyled one instead. The base layer is
  small on purpose: `Frame`, `List`, `Layers`, `Grid`, `Text`, `Offset`, `VirtualList`, `Canvas`,
  `Drawing`, `Overlay`, `Interactive`, `Embed`, `Portal`, `BackHandler`, `Shift`, `Fade`. Most
  things are compositions of those (`unstyled::Picture` is a `Drawing` with a size). Add a base
  component only when the retained tree lacks a primitive: a new way to lay out, paint or receive
  input that no arrangement of existing nodes expresses. `Portal` is one: it shows a subtree it
  does not own, so a node laid out in one place this frame can be laid out elsewhere the next
  without being rebuilt. If a new concern can share `Frame`'s single-child box model, extend
  `Frame` rather than adding another pass-through node.

`unstyled::Button` shows the split. It is a focusable `Interactive` and owns button semantics,
disabled behavior, pointer and keyboard activation, and accessibility: it takes the `label`,
`glyph`, `role` or `Action` and builds its own accessibility node from them. Its content closure
receives a `ButtonHandle` of reactive `hovered`, `active`, `focused` and `disabled` state and the
resolved `label`, `glyph` and `tooltip`. `styled::Button` wraps it and uses that handle to choose
fills and paint a focus outline, so every visual treatment sits on the same behavior. Handles of
controls built on a button, like `MenuButtonHandle` and `PopoverTriggerHandle`, carry that
`ButtonHandle` as `button` rather than copying its fields.

Pure presentation components such as styled text and cards compose base components directly,
because they have no interaction behavior to delegate.
