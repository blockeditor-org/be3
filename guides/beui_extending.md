# Extending beui

How to add a component to beui itself. [beui.md](beui.md) is the guide to using
beui, and it explains which of the three layers a new thing belongs in; this one
is the checklist for each layer once you have chosen.

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
update the node's own retained state, which is how a virtual `Scroll` realises
the rows the viewport and offset call for. Both receive `&mut Document` and are
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
