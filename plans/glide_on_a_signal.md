# Glide on a signal: what is left

Depends on: plans/for_each_handle.md, step 1, for the row index; and `view!` forwarding an
optional prop (`Option<Func<..>>`, `Option<Prop<..>>`, `Option<RenderFn<..>>`) from one
component to another, for the styled `Reorder`.

`Glide` is armed by `on: Option<Trackable>` and takes its spring as a value; the styled `Glide`
passes `Movement::Settle` (guides/beui/components.md, Time). Two parts are left:

- **`Reorder`'s spring.** `unstyled::Reorder` still settles its rows and gaps on a private
  `SPRING` (500, 42). Make it a required `spring: Spring` prop and add a styled `Reorder` that
  passes `Movement::Settle.spring()`, replacing styled's re-export of the unstyled one (its 18
  callers then use the styled one unchanged). The styled one must forward `accepts`, `takes`
  and `slot`, which are optional `Func`s: wait for `view!` to forward an `Option` prop rather
  than writing a tag per combination. Then accept the reorder paintings again.
- **Rows armed by their index.** Once a `ForEach` row has its index, `Reorder` hands each row's
  `Glide` the index (`on={index}`) and drops its `arm: Trigger`, which today wakes every row on
  any move. Test: arming from a row's index glides a reordered row and leaves the others still.
