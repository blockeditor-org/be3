# Beui

Beui is BE3's retained-mode user interface toolkit. A `Document` owns a
persistent tree of nodes, and reactive signals update that tree in place. The
tree is laid out, receives input, exposes accessibility, and paints into a
`Context` whenever `Document::show` is called. Building the view is therefore
not the per-frame render loop: build it once, keep the `Document`, and show that
same document every frame.

The quickest introductions are the small
`crates/beui/examples/counter.rs` example and the component catalog in
`crates/beui/examples/demo.rs`.

## Component layers

Beui separates mechanism, behavior, and appearance. The dependency direction
is deliberate:

| Layer | Location and public path | Responsibility |
| --- | --- | --- |
| Base | `crates/beui/src/base`; re-exported from `beui::reactive` | Retained nodes for layout, painting, visibility, focus, pointer input, scrolling, and text. |
| Unstyled | `crates/beui/src/unstyled`; `beui::unstyled` | Accessible interaction behavior composed from base components, without theme colors, typography, borders, or spacing. |
| Styled | `crates/beui/src/styled`; `beui::styled` | Application-ready controls that compose an unstyled control and paint its state with base components and `styled::theme` tokens. |

Application components normally compose styled controls with base layout and
painting primitives. Use an unstyled component when the existing behavior is
right but the appearance is not. Add a base component only when the retained
tree lacks a primitive needed to implement behavior or rendering.

For example, `unstyled::Button` composes `Focusable` and `ClickCatcher`. It owns
button semantics, disabled behavior, pointer and keyboard activation, and
accessibility. Its content closure receives a `ButtonHandle` containing
reactive `hovered`, `active`, and `focused` state. `styled::Button` wraps that
unstyled button and uses the handle to choose fills and paint a focus outline.
This keeps every visual treatment on the same interaction behavior.

Pure presentation components such as styled text and cards can compose base
components directly because they have no interaction behavior to delegate.
Interactive styled controls must use the matching unstyled behavior instead of
reimplementing focus, keyboard, pointer, touch, or accessibility handling.

The main base building blocks are `Row`, `Column`, `List`, `Frame`, `Text`,
`Scroll`, and `VirtualList`. `Frame` combines optional sizing, padding, fill,
outline, and visibility on one retained node.
`Focusable` and `ClickCatcher` are lower-level interaction primitives mainly
used to develop unstyled controls. The unstyled module contains behavior such
as `Button`, `Pressable`, `Toggle`, `Choice`, `Slider`, `TextInput`,
`Disclosure`, `Tree`, `Select`, `ContextMenu`, `Container`, and `Stack`. The
styled module supplies the themed buttons, text styles, cards, toggles, choices,
inputs, menus, tabs, trees, progress, scrollbars, and responsive layout
components.
The module re-exports in `unstyled.rs` and `styled.rs` are the authoritative
component lists.

## Use Beui in a standalone app

The default `beui` feature is `window`, which includes the winit runner and the
wgpu renderer. A standalone app builds its document once and implements
`beui::App`:

```rust
use beui::reactive::{
    build, component, create_memo, create_signal, view, Column, Frame,
};
use beui::styled::{use_theme, Button, ButtonVariant, Display};
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
        <Frame color={theme.pick(|theme| theme.background)}>
            <Column spacing=8.0>
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
            </Column>
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

Use Ctrl+Shift+I in a standalone beui window to open the node, accessibility,
and performance inspector. Ctrl+Shift+C enables node picking. Ctrl+Shift+F moves
keyboard focus into the panel and back out again, and Escape inside the panel
returns focus to the document, so the whole inspector is reachable without a
mouse. Its tree rows select and expand together: clicking a row, or pressing
Enter or Space on it, selects the node it lists and opens or closes its
children, and the arrow keys walk the tree.

### Input simulation

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

Beui has three feature levels:

- No features provides the document, components, layout, input model, and
  painting output. This is enough for headless logic tests.
- `render` adds the wgpu renderer without creating a window. Embedded hosts
  use this level.
- `window` adds the desktop runner and enables `render`; it is the default.

## Write views and components

Import `view!`, `#[component]`, signals, and base components from
`beui::reactive`. A component is a function returning its root `NodeId`:

```rust
use beui::reactive::{component, view, Column, Prop, Text};
use beui::NodeId;

#[component]
fn LabeledValue(label: Prop<String>, value: Prop<String>) -> NodeId {
    view! {
        <Column spacing=4.0>
            <Text string={label} />
            <Text string={value} />
        </Column>
    }
}
```

A `Prop<T>` accepts a plain `T`, a `ReadSignal<T>`, or a `Memo<T>`. Bind a
signal or memo directly rather than reading it while building the view; the
binding installs an effect that updates the retained node. Use `create_memo`
for derived values so downstream properties update only when the derived value
changes. Event callbacks may write signals, and those updates are reflected in
the same frame.

`view!` supports three framework attributes on every tag:

- `@sizing` selects the child's `ItemSize` in a list. Children are intrinsic by
  default; fixed children reserve a logical-point size, and percent children
  share the remaining bounded space by weight.
- `@test_id` gives a node a stable name for headless interaction tests.
- `@node_ref` fills a `NodeRef` when enclosing code genuinely needs the
  resulting `NodeId`.

Use `Frame`'s `width` and `height` props to constrain a component's own size.
Use `@sizing` to describe how that component participates among siblings in a
`Row`, `Column`, or `List`. `Container` and `narrower_than` provide container-responsive state;
`unstyled::Stack` and `styled::Stack` switch between a row and a column without
rebuilding their children.

Use `Show` for a lazy conditional subtree, `Dynamic` when a value replaces a
subtree, `ForEach` for keyed retained children, and `VirtualList` for a large
scrolling collection. Give `Dynamic` an `item_size` when its generated subtree
must take a fixed or percentage share of the available space. These primitives
own the scopes of nodes they add and remove, so effects and cleanup follow the
retained tree.

The [reactive guide](reactive.md) describes attribute syntax, children and
render props, controlled state, keyed lists, scopes, context, and responsive
layout in detail.

### Update a document from outside its events

Component callbacks run with their `Document` installed, so signal writes from
clicks and key events need no special handling. A host-driven update happens
outside that context and must enter the document's reactive scope:

```rust
use beui::reactive::{with_reactive_scope, WriteSignal};
use beui::Document;

fn set_value(document: &mut Document, value: &WriteSignal<String>, next: String) {
    let value = value.clone();
    with_reactive_scope(document, move || value.set(next));
}
```

Keep the write handles needed by the host next to its `Document`. Do not rebuild
the document to display new data.

## Use beui in a block editor plugin

A beui editor implements `block_editor_plugin::BeuiApp` and uses
`block_editor_plugin::beui_plugin!` instead of the egui `App` and `plugin!`.
Build and retain the editor's `Document`, then show it from `frame`:

```rust
impl block_editor_plugin::BeuiApp for Editor {
    fn frame(&mut self, context: &beui::Context, rect: beui::Rect) {
        self.document.show(context, rect);
    }
}

block_editor_plugin::beui_plugin!(Editor, "../manifest.json");
```

The host supplies input, fonts, clipboard integration, rendering, and the frame
rectangle. A plugin normally depends on beui without the window runner:

```toml
beui = { path = "../../beui", default-features = false, features = ["render"] }
```

The counter editor under `crates/editors/counter` is the reference integration.
The [plugin editor guide](adding_a_plugin_editor.md) covers the manifest,
creation flow, host connection, and current beui plugin capability limits.

A plugin with `"creation": "Dialog"` implements `creation_frame` to show a
separate retained document in the host's creation dialog. Host services such as
`BlockPicker` work there in the same way they do from an egui creation UI.

## Develop an unstyled component

An unstyled component owns semantics and interaction, not appearance. Put it in
`crates/beui/src/unstyled/<name>.rs`, declare it in `unstyled.rs`, and re-export
the public component, handles, state readers, and supporting types there.

Compose it from base components. For an interactive control this normally
means:

1. Model controlled values and transient interaction values with signals.
2. Use `Focusable` for tab order, keyboard events, activation, and focus state.
3. Use `ClickCatcher` for pointer and touch interaction.
4. Publish the correct AccessKit role and state with
   `component_accessibility`.
5. Give the caller a `Render<Handle>` or `RenderFn<Handle>` containing the
   reactive state needed to paint the control.
6. Return the root base node directly so component state and framework slots
   attach to the node callers receive.

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
fn CustomButton(label: Prop<String>, on_click: ClickCallback) -> NodeId {
    view! {
        <unstyled::Button
            on_click={move || on_click.call()}
            content={move |handle| view! {
                <CustomButtonFace handle label />
            }}
        />
    }
}
```

`CustomButtonFace` derives colors or visibility with memos reading
`handle.hovered`, `handle.active`, and `handle.focused`, then composes `Frame`
and `Text`. Keep keyboard and pointer handling in the
unstyled button. Read colors from the nearest theme with `styled::use_theme()`:
bind a token directly with `theme.pick(|theme| theme.accent)`, or call
`theme.get()` inside a memo that also reads interaction state, so the control
repaints when the theme changes. Sizes, radii, and font sizes are constants in
`styled::theme`. Add a field to `Theme`, with a value in every built-in theme,
when a color is part of the theme rather than unique to one component.

Styled controls must visibly expose keyboard focus. Keep labels and accessible
roles stable when visual state changes. Use glyphs from `beui::icons` with
`styled::Icon` or `styled::IconSized`; do not use Unicode characters as ad-hoc
icons. The [keyboard guide](beui_keyboard.md) records the expected behavior for
each control family.

### Themes

`styled::Theme` holds the color tokens, and `Theme::DARK` and `Theme::EINK`
are the built-in themes. Every `Document` owns a theme signal that styled
components use when no provider covers them. Change it with
`Document::set_theme` and read it with `Document::theme`, for example to pick an
app's clear color. The inspector's Sim tab switches it at runtime.

`ThemeProvider` overrides the theme for the subtree written between its tags
and follows the `Prop<Theme>` it is given:

```rust
view! {
    <ThemeProvider theme={theme_signal}>
        <Settings />
    </ThemeProvider>
}
```

## Develop a base component

Base nodes are the only layer that should normally mutate a `Document`
directly. Put the node in `crates/beui/src/base/<name>.rs` and register the
module in `base.rs`. A base implementation has three parts:

- A crate-private node struct containing its retained state and child ids.
- An `Element` implementation for measurement, layout, painting, interaction,
  child traversal, and inspector metadata.
- `Document::create_*` and `Document::set_*` methods plus a public
  `#[component]` wrapper. The wrapper creates the node with `with_document` and
  binds reactive props to setters with `create_effect`.

Only invalidate retained state when a setter actually changes a value. A
spurious mutation invalidates layout or paint caching for the entire document.
Return children from both interaction traversal and `children`, give the node a
stable `kind` for the inspector, and add a concise `detail` when it makes the
tree easier to understand.

The `base` module itself is private. Re-export base components intended for
composition from `beui::reactive`, as the existing `Frame`, `Text`, and `Scroll`
components are. Extend `Frame` when a new concern can share its single-child
box model instead of adding another pass-through node. Keep implementation-only primitives crate-private when they
exist solely to support an unstyled control.

## Test and verify changes

Beui behavior is tested headlessly. For library behavior, add a test under the
relevant `tests` directory and declare it in that directory's `tests.rs`; this
repository keeps one test per file. The document tests use their `Harness` to
build a `Document`, send `Event` values through a `Context`, and inspect
component state, layout, accessibility, or painting output.

For a beui block editor, use `block_ui_test::BeuiTest`. Give every interacted
node an `@test_id`, call `run` after queued gestures, assert the resulting block
state, and snapshot only when the painting is meaningful. `BeuiTest` also
supports key presses, text, hover, pointer clicks, and touch gestures. See the
[GUI testing guide](testing_a_gui.md) and the counter editor tests for examples.

From the workspace root, use:

```text
./scripts/check
./scripts/verify
```

`./scripts/check` is the fast complete-workspace compile check. Always finish a
coherent change with `./scripts/verify`; it runs the workspace tests, lints,
formatting, project structure checks, and snapshot updates. Use a package-scoped
Cargo command only as a narrow diagnostic after one of the supported scripts
has exposed a failure. Run `./scripts/run --smoke` as well when a change can
affect native startup or runtime integration.
