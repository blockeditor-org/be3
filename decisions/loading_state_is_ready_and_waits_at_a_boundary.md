# Data that may not have arrived is a `Ready<T>`, and views wait for it at a `Loading` boundary

Data that may not have arrived is a `Ready<T>` (`Result<T, Pending>`, with `Pending` either `Loading` or `Failed(message)`). Memos derive from it with `?`. A `<Loading>` boundary (an editor's `<Loaded>`) builds its content hidden, with its effects paused, until every source its content made has arrived, then reveals it and never shows the fallback again. See guides/reactive.md, "Loading".

## Why

- An editor shows nothing it would have to correct a moment later: not an empty or editable view before its content arrives, and not a placeholder ("no rows") that the next answer replaces.
- `?` carries the pending state through derived values, so the waiting state lives in one place.
- Effects that write outside the graph (an intrinsic size, seeding a child block) run once at reveal with the data that arrived, so they never act on data that has not.
- A later reload updates the revealed view in place rather than flashing the fallback.

## Rejected

- An `Option` or a `loaded: bool` kept beside the value: `Ready<T>` and `?` already carry that state.
