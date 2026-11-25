# One way each in the host and its runtimes

Depends on: none. Steps 2 and 3 are best done inside plans/host_as_platform_adapter.md's step
4g7e, which moves `compositor/desktop.rs`, be-host's root handover and the desktop hosts' window
lists; step 1 is independent of that plan.

The host and the runtimes under it do several things twice, and two of the copies already
disagree. Each step below removes one of the pair.

## Steps

1. **One block-stack test fixture.** `be-host-testing`'s `Harness` is nearly line for line
   `be-host-stack/src/tests.rs`'s `Harness`/`serving` (the `ONE_STACK` mutex, a temp directory,
   registering `counter@example.com`, the "Counters" organization, `stop_server`/`start_server`,
   `connect`, stop-and-delete on drop), and `be-host-tests/src/server.rs`'s `Server` is a richer
   version of it (several accounts, a fault relay per device, `slow`, `offline`). Make one
   fixture in `be_host_stack::testing` (behind its `testing` feature): a server with accounts
   and per-device relays, with options for the push config and the clock. The single-account
   harness is that server with one account and the stack connected. `stop_server` and
   `start_server` stay a listener closed and rebound at the same address, which a relay going
   offline is not. be-host-testing re-exports it or goes. The temp directory stops being named
   `be-host-be-test-…`.
2. **One rule for keeping a floating window grabbable.** be-sdk-window-stack
   (`reachable`, `grabbable`, `fitted`, `KEPT` 96×48), be-host's `compositor/desktop.rs`
   (`clamped`, `reachable`, `KEEP_VISIBLE`, `GRAB_MARGIN`, `SHRINK_TO`) and beui-dock-host
   (`reachable_origin`, `clamped_origin`, `WINDOW_KEEP_VISIBLE`) each keep a grip-sized corner
   inside one or more areas. One function in be-lib-geometry takes the window, the areas and the
   grip size; the three call it. Where the shrink rules differ, keep the desktop's.
3. **Publish, don't poll, the last revision counters.** `be_host_base::watches::publish`
   pushes a change to the `WatchHost` and skips unchanged bytes, and most services call it on
   change. Two still keep a counter be-host compares every frame: `DesktopHost::revision()`
   with `bound_windows()` (on Linux a counter beside the reactive `windows.list()`), and
   `program_editors::listed_since`, which be-host's `root.rs` reads to hand the root its
   catalog. Each publishes from an effect over what it describes (the windows' effect tracks
   both the list and `bound_windows`), and the counters and their per-frame checks go.

## The test

- The existing tests of the stack and of the desktop's windows pass on the unified code; the
  host tests' paintings do not change.
