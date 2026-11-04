# Reactive handles are `Copy` keys into a scope-owned arena

Signals, memos, selectors, stores, stored values, callbacks, bindings and node refs in `beui-reactive` and `beui::reactive` are `Copy` handles: keys into a thread-local arena, owned by the scope current when they were created and freed when it is disposed. State that outlives any scope uses the `Owned*` counterparts, which free their value when they drop. See guides/reactive.md.

## Why

- A `move` closure takes its own copy and the original stays usable, so handlers and bindings capture handles without cloning, and a closure that captures only `Copy` values is itself `Copy`.
- A struct made of handles derives `Clone, Copy` and is passed around by value; nothing clones it.
- Lifetime follows the view: disposing a component's scope frees everything it created, along with its effects, cleanups and nodes.

## Rejected

- Reference-counted handles: each closure would need its own clone, which `Copy` handles avoid.
