# A root links beui as an app does

A root guest (plans/host_as_platform_adapter.md, The root guest) is built as an app is: a
linked module in the apps' plan (`wasi-guest`, for single-threaded `wasm32-wasip1`,
decisions/guests_are_single_threaded.md), linked against the app libraries (base and beui,
`app_module` in buck/wasm/defs.bzl), and loaded by be-host as an app is
(`be_wasm_host::Host::new` on the window's device, `Staged::load`), with app WASI. It draws its
own surfaces with beui through crates/be-sdk-beui-root. Only the host runs it differently: `Root`
(crates/be-wasm-root-host) steps that `App` inline on the frame thread rather than on a thread of
its own.

A root still never blocks its thread (plans/host_as_platform_adapter.md, The root guest, Its
rules), which a linked module allows as a pure one does: nothing it runs waits, and as every
guest it is single-threaded, so it can start no threads.

## Why

- beui's text needs FreeType and HarfBuzz, C libraries that need a libc. The apps' toolchain
  builds them, position-independent, against wasi-libc; `wasm32-unknown-unknown` has no libc, so
  a pure root would need a second build of them against one of its own, beside a second build
  of beui, wgpu's `custom` backend and every crate they share with the apps.
- The libraries already hold beui, its runner, its wgpu renderer, wgpu's `custom` backend over
  `be3:gpu` and FreeType. A root linked against them holds only what they do not
  (be-sdk-root-testing's module, 1.4 MB, holds be-sdk-root, be-sdk-app-host, be-sdk-beui-root
  and itself), and natively shares the one compile of each library every app on the device
  uses (`Host`'s library cache is per flavor, so a pure root would compile beui again).
- Every guest is to end up on one triple (plans/pure_guests_in_the_apps_plan.md); building the
  root with the apps is that, for the root, already.
- What a root needs it gets as an app: the host's device (`Host::new` gives it the gpu), the sandbox (`be3:sandbox` over its ops connection,
  which no flavor changes), and WASI with nothing it cannot have: an app's WASI has no files,
  arguments or environment, only the clocks, randomness, standard error and exiting
  (be-wasm-host's `wasi.rs`, and be-host/web/wasi.js on the page).

## Rejected

- **A pure root** (`wasm32-unknown-unknown`, no WASI, its memory its own): a second build of
  beui, wgpu and the C libraries for a triple the apps do not use, and a libc for FreeType and
  HarfBuzz to link against, for a sandbox the root does not need, since the root is be-host's own
  module and trusted (plans/host_as_platform_adapter.md, This plan adds no security boundary).
  The pure root be-wasm-root-host's own tests run (be-wasm-root-testing) stays one: it draws no
  UI, and `Root` runs either.
- **A root that draws with raw `be3:gpu` calls and no UI framework**: every area of the host's
  UI that moves into the root (plans/host_as_platform_adapter.md, 4g3 onward) is beui code today.
- **beui in be3:root**: the protocol stays framework-independent (AGENTS.md); beui is only what
  this root links. `be3:root` gained only what any framework needs: the host's fonts
  (`host.fonts`) and, for the host's tests, what the root's own surfaces painted (`paintings`,
  as `be3:debug`'s `paintings` is for an app).

## What it costs

- On the web the page runs the root on its main thread (decisions/the_web_page_never_waits.md),
  so a linked root there needs the page to link its libraries into the root's unshared memory
  on that thread with `dylink.js` and serve it `wasi.js`, which today only an app's worker does.
- Natively its gpu calls are ordinary V8 calls, slower than fast-API ones, as every
  single-threaded guest's are (decisions/guests_are_single_threaded.md, What it costs). The root
  pays that on the frame's critical path, since the host steps it inline, so it gains first from
  winning the fast path back for an unshared memory (plans/single_threaded_guests.md, step 4).
