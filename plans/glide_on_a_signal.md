# Reorder rows glide on their own index

Depends on: plans/for_each_handle.md, step 1.

`Glide` is armed by `on: Option<Trackable>` (guides/beui/components.md, Time). `Reorder` arms
every row's `Glide` from one `Trigger` it notifies on any move, so every row looks for a move
of its own after each reorder.

Once a `ForEach` row has its index, `Reorder` hands each row's `Glide` that index
(`on={index}`) and drops its `arm: Trigger`: a row arms only when its own place in the list
changes. Test: a reorder glides the rows whose index changed and leaves the others still.
