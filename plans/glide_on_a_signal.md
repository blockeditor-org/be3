# Glide armed by a signal, with its spring from the theme

Depends on: plans/motion_tokens.md, step 1, for the styled `Glide`; plans/for_each_handle.md,
step 1, for the row-index test.

`unstyled::Glide` (`beui-components-unstyled` `glide.rs`) is already one component on both cores,
built from `Frame`'s `translate` and `raised`, `measure`, `measure::scrolled_by` and
`create_animation`. Two things are left:

- It is armed by bumping a `u64` (`arm`), a counter every caller hand-rolls (`Reorder`'s
  `arm: Signal<u64>`, beui-demo's base pages, the beui-host glide tests).
- Its spring, `GLIDE_SPRING`, is a constant in the unstyled layer, against the rule that unstyled
  components take concrete values and know nothing about the theme.

## Shape

```rust
// unstyled: the curve is a value, with no default
#[component]
pub fn Glide(spring: Prop<Spring>, #[prop(default = None)] on: Option<Track>, /* api, from, raised, within as today */
             children: Child) -> NodeId

// styled: no curve to choose; the theme's `Movement::Settle`
#[component]
pub fn Glide(#[prop(default = None)] on: Option<Track>, /* api, from, raised, within as today */
             children: Child) -> NodeId
```

Code on the styled layer uses the styled `Glide`; the unstyled one is for unstyled components
(`Reorder`, the dock's unstyled layer), which take the spring from whoever styles them. A styled
token prop is added only when a second look is needed.

- **`on`:** anything trackable (a `ReadSignal`, a `Memo`, a `Trigger`, a `ForEach` row's index,
  plans/for_each_handle.md). When it changes, the next move glides rather than jumps, exactly as
  bumping `arm` does today. An explicit prop, never a context: a layer that provided one by
  accident would arm glides nobody asked to move.
- **`spring`:** required on the unstyled `Glide`, as a curve is appearance
  (decisions/beui_unstyled_layer_has_no_appearance_defaults.md). `GLIDE_SPRING` goes; its value
  becomes the theme's `Movement::Settle`. This plan owns that move.

## beui-reactive's `Trigger`

Add `create_trigger() -> Trigger` with `notify()` and `track()`: a signal with no value. It is
what `on` takes when the move is not a keyed one, and it replaces counters kept only to be bumped.

## Steps

1. `Trigger`.
2. `on` replaces `arm`; `spring` on the unstyled `Glide`, and the styled `Glide`. Migrate every
   `arm` caller (`rg 'arm=' crates`) and drop `Reorder`'s `arm` signal.
3. Tests: arming from a row's index glides a reordered row; arming from a `Trigger` glides a
   row moved by something other than its key; the existing glide tests keep passing on `on`.
4. Update components.md's paragraph on `Glide` (Timers and animation).
