# One way each in the host and its runtimes

Depends on: none. Step 4 is best done inside plans/host_as_platform_adapter.md's step 4g7e,
which moves `compositor/desktop.rs` into the root; the rest is independent of that plan.

The host and the runtimes under it do several things twice, and two of the copies already
disagree. Each step below removes one of the pair.

## Steps

1. **One panic container, which the panic hook hears.** Guest threads catch a panic two ways:
   `be_wasm_guest_host::thread::catch` (the sandbox, `be_wasm_host::sandbox::spawn`, and type
   modules, `be-block-type-host/src/modules.rs`) and `be_host_base::panic_guard::contain` (app
   runtimes, `be-host-runtime/src/wasm.rs` and `runtime.rs`). Only `contain` raises the
   thread-local `CONTAINING` count that `panic_guard`'s hook reads, so a sandbox guest or a type
   module that panics inside be-host is taken for an uncontained panic: the hook sets
   `LAST_PANIC`, writes last-panic.txt, and `Host::frame` shows the whole host's crash screen
   for a failure that was already reported as the guest's. Move `contain` and its counter into
   be-wasm-guest-host (the lowest crate), have the hook in be-host-base read it there, remove
   the `Contain` parameter from `Spawn` and `be_wasm_host::spawn`, and keep one
   `panic_message` (today in be-wasm-guest-host, be-host-base and beui-runner).
   Test: a type module that panics inside a host test leaves the host running and writes no
   last-panic.txt.
   Done: `contain`, `contain_task`, the counter (`containing`) and one `message` are in
   be-wasm-panic, the base layer both be-wasm-guest-host and beui-runner may use (be-lib and beui
   crates may not be beneath a be-wasm one); every guest thread and in-process guest runs in it,
   and be-host-base's hook is set through `be_wasm_panic::set_hook`. A type module cannot be made
   to panic in host code from a host test, so the test is be-host-base's
   `a_guest_thread_that_panics_is_its_failure_not_a_crash`, a guest thread spawned as type modules
   and the sandbox spawn theirs.
2. **One `RootSettings`.** `be-sdk/src/root_settings.rs` (`own_settings_blocks`,
   `RootSettings::{read, find, ensure_with}`, `layers`, over `SettingsGraph`) and
   `be-host/src/root_settings.rs` (`owned`, `find`, `ensure`, `settings`, `layers`, against
   `be_host_stack`) state the same rules: settings authored by the account at the root, sorted,
   merged, created once if missing, then the every-device and this-device layers. be-host
   implements `SettingsGraph` over the stack, keeping its read cache and its `ready(Roots)`
   check inside that implementation, and builds its sessions, profiles, My Files and Unfiled on
   `be_sdk::root_settings`. guides/settings.md then names one place.
   Done: the rules moved down to `be_block::root_settings`, which be-sdk re-exports, since be-sdk
   is the guest's crate (on the web it would bring the guest's wgpu into be-host). `SettingsGraph`
   makes a block `under` a block or at the root, and its `listed` is the roots check be-host's
   implementation answers with `ready(Roots)`, dropping what it read before; be-host's
   `Stack` keeps the read cache.
3. **One block-stack test fixture.** `be-host-testing`'s `Harness` is nearly line for line
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
4. **One rule for keeping a floating window grabbable.** be-sdk-window-stack
   (`reachable`, `grabbable`, `fitted`, `KEPT` 96×48), be-host's `compositor/desktop.rs`
   (`clamped`, `reachable`, `KEEP_VISIBLE`, `GRAB_MARGIN`, `SHRINK_TO`) and beui-dock-host
   (`reachable_origin`, `clamped_origin`, `WINDOW_KEEP_VISIBLE`) each keep a grip-sized corner
   inside one or more areas. One function in be-lib-geometry takes the window, the areas and the
   grip size; the three call it. Where the shrink rules differ, keep the desktop's.
5. **Publish, don't poll, the last revision counters.** `be_host_base::watches::publish`
   already pushes a change to the `WatchHost` and skips unchanged bytes, and most services call
   it on change. A few still keep a counter be-host's main loop compares every frame:
   `push_device::REVISION`, `DesktopHost::revision()` with `bound_windows()` (on Linux a counter
   beside the reactive `windows.list()`), and `program_editors::listed_since`. Each publishes
   from an effect over what it describes (the windows' effect tracks both the list and
   `bound_windows`), and the counters and their per-frame checks go.
   Done for `push_device`: `set_state` publishes `PushDeviceWatch` as it changes, and be-host
   publishes the first state as it starts. Left: the windows' revision (be-host-desktop's
   Linux and Windows hosts, be-host-wayland) and `program_editors::listed_since`, which be-host's
   `root.rs` reads to hand the root its catalog; both are in what plans/host_as_platform_adapter.md's
   step 4g7e moves, so they go with it, as step 4 does.
6. **One failure budget for guests that restart.** Type modules restart up to `RESTART_LIMIT`
   (8) over their lifetime and then stop with only an `eprintln`; the remapper stops after
   `FAILURES_BEFORE_STOPPING` (3) within `FAILURE_WINDOW` and posts a notice; Xwayland gives up
   after `MAX_RESTARTS` (3). One `FailureBudget` (failures within a window, then stop) used by
   all three, and every stop posts a notice, so a type module that stops for good is no longer
   silent to the person.
   Done: be-lib-failure-budget's `FailureBudget` (failures within a window, then `Stop`, until
   `reset`), with each its own count and a minute's window: type modules 8
   (`Replicas::take_stopped_modules`, which be-host-stack shows as a notice), the remaps 3, and
   Xwayland 3 (`ServerEvent::Failed`, through the compositor's `on_failure`). Type modules and
   Xwayland counted over their lifetime before; now a failure more than a minute after the
   others no longer adds to them.

## The test

- Step 1's host test above.
- The existing tests of root settings, of the stack and of the desktop's windows pass on the
  unified code; the host tests' paintings do not change.
