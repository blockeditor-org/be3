# Glide in the unstyled layer, armed by a prop

Depends on: plans/motion_tokens.md, for the styled `Glide`.

`Glide` is written twice today: a layout node in beui-core (`base/glide.rs`, with glue in
`beui-view/src/native/glide.rs`) and an HTML version in `beui-view/src/html/glide.rs` built
from `measure`, a CSS transform and `create_animation`. It is armed by bumping a `u64`
(`arm`), a counter every caller hand-rolls. Move it to `beui-components-unstyled` as one
component on public pieces, and arm it from any signal.

## Shape

```rust
// unstyled: the curve is a value, with no default
#[component]
pub fn Glide(spring: Prop<Spring>, #[prop(default = None)] on: Option<Track>,
             #[prop(default = None)] from: Prop<Option<Pos2>>, children: Child) -> NodeId

// styled: no curve to choose; the theme's `Movement::Settle`
#[component]
pub fn Glide(#[prop(default = None)] on: Option<Track>,
             #[prop(default = None)] from: Prop<Option<Pos2>>, children: Child) -> NodeId
```

Code on the styled layer uses the styled `Glide`; the unstyled one is for unstyled components
(`Reorder`, the dock's unstyled layer), which take the spring from whoever styles them. A styled
token prop is added only when a second look is needed.

- **`on`:** anything trackable (a `ReadSignal`, a `Memo`, a `Trigger`). When it changes, the glide
  takes the position its child had at the last layout and, after the next one, springs from
  there to where it now rests. An explicit prop, never a context: a layer that provided one by
  accident would arm glides nobody asked to move.
- **`from`:** a window point to settle from, as today (a dropped preview).
- **Built from:** `Frame`'s `translate` (paints shifted, leaves the space the frame takes alone,
  skips painting off screen), `measure`, `create_animation` and `GlideMotion`, which moves from
  beui-types to unstyled; `Spring` stays in beui-types as the value base components take, and
  `GLIDE_SPRING` becomes the theme's `Movement::Settle`. `hold_glide` becomes `translate = hold point
  - resting point` from the same measurement.

The rule it keeps: a glide animates only the part of a move that layout made within its own
space, never a change of transform. A scroll during a glide moves it at once and leaves the rest
of its motion as it was, and so does a scroll in the same update that arms it.

## What core gains and loses

- **Gains:** `Measured` carries the position in its enclosing scroll content and its scale, not
  only the window rect (core.md, Measurement). Without it an unstyled glide would count a scroll
  as movement; the HTML glide works around this privately with
  `beui_core_html::measure::scrolled_by`. Scroll anchoring and drag candidates
  (plans/declarative_dragging.md) use it too.
- **Gains:** `Frame`'s `raised` prop, replacing `set_raised` (paint order among siblings, which
  only core can do).
- **Loses:** `GlideNode` and its document methods, the native glue and the HTML glide.

## beui-reactive's `Trigger`

Add `create_trigger() -> Trigger` with `notify()` and `track()`: a signal with no value. It is
what `on` takes when the move is not a keyed one, and it replaces counters kept only to be bumped.

## Steps

1. `Measured`'s content position and scale on both cores; `Frame`'s `raised`; `Trigger`.
2. `Glide` in unstyled and styled; migrate every `arm` caller (`Reorder`, the dock's bars, tree, the rest
   found by `rg 'arm'` around `<Glide`) to `on`; delete the core node and the HTML glide.
3. Tests: a glide scrolled mid-flight moves with the scroll at once and finishes its own motion;
   a scroll in the update that arms one adds nothing to it; arming reads the last layout, not the
   next; `Motion::Instant` lands at once. Paintings over frames for the first.
4. Benchmark a 1000-row list reordered with the old node and the new component
   (guides/performance.md). Each moved glide now measures and may cost a second layout pass in the
   frame it moves; record the numbers in the PR.
5. Update components.md's paragraph on `Glide`.
