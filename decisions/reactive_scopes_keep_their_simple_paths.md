# Reactive scopes keep their simple creation and disposal paths

beui-reactive gives every component a scope of its own, enters a reactive context when
disposing a scope, and grows a scope's cleanups as they are added. Three optimizations of
these paths were measured and not adopted.

## Why

Each changed nothing worth its code, or made things slower.

## Rejected

- Skipping the reactive context a disposing scope enters when disposal is already
  untracked: checking cost more than it saved.
- Giving a component no scope of its own until something is created in it: only about one
  component scope in ten ends up empty, because nearly every component probes a binding.
- Reserving room for eight cleanups in every scope: it saved a tenth of the allocations and
  made opening a page 2.5% slower.
