# Data that may not have arrived is a `Ready<T>`, whose readiness is part of the graph, and views wait for it at a boundary

Data that may not have arrived is a `Ready<T>` (`Result<T, Pending>`, with `Pending` either `Loading` or `Failed(message)`). Memos derive from it with `?`. It comes from a root that knows whether it is there: an async memo (`create_async_memo`) or a source settled from outside (`create_source`). Whether a value is there is a property of each graph node, as in SolidJS 2.0: a memo is pending (or failed) while anything it read is. A read of a pending value outside a memo registers with the nearest `<Loading>` boundary (an editor's `<Loaded>`), which builds its content hidden, with its effects paused, until nothing read under it is pending, then reveals it and never shows the fallback again. A failed value goes to the nearest `<Errored>` boundary above the read, before or after a `Loading` between them reveals (with none, to the nearest unrevealed `Loading`'s fallback); its reset asks again. See guides/reactive.md, "Loading".

## Why

- An editor shows nothing it would have to correct a moment later: not an empty or editable view before its content arrives, and not a placeholder ("no rows") that the next answer replaces.
- `?` carries the pending state through derived values, so the waiting state lives in one place.
- A boundary waits for what its content reads, not for what was made inside it, so a source made once and shared waits wherever it is shown, and one made but never shown holds nothing back.
- Effects that write outside the graph (an intrinsic size, seeding a child block) run once at reveal with the data that arrived, so they never act on data that has not.
- A later reload updates the revealed view in place rather than flashing the fallback; a failure after the reveal still reaches an `Errored`.

## Consequences

- A memo that reads a pending value and handles it (`ready.is_err()`) still counts as pending until the value settles: readiness follows what was read, not what was returned.
- Data that arrives later is a source or an async memo, never a `Signal<Ready<T>>`, whose readiness the graph would not see.

## Rejected

- An `Option` or a `loaded: bool` kept beside the value: `Ready<T>` and `?` already carry that state.
- Telling a read of a pending `Ready` from the value's type at the read: generic reads cannot tell a `Ready<T>` from another `T` on stable Rust, so readiness lives on the node.
- Registering each source with the boundary around the scope that made it (`wait_for`): it missed sources made outside the boundary and read inside it, and held boundaries for sources nothing showed.
