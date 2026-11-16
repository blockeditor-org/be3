# Headless apps

Depends on: none.

`be` (`crates/be-tool-cli`) holds blocks through type modules, as be-host's
peer does, but runs no apps: the terminal's host is native code of its own
(guides/terminal.md). A hosted block's host is an app's host component
(plans/hosted_blocks.md, "The host component"), so a server that hosts one
needs `be` to run ordinary app wasm, the same modules be-host stages, with no
graphics. One runtime, not a second one for `be`.

## Shape

- `be` runs apps through be-host-runtime's `Runtime` and the wasmtime backend
  (guides/apps/runtime.md, Backends), with a host that has no compositor, no
  windows and no gpu: an app instance there gets no frames and no input, and
  answers host services from be-host-stack as be-host does, with the calls
  that need a person or a screen (pickers, dialogs) refused.
- An app that asks for the gpu there gets an error it can handle; a host
  component never needs one.
- `be` carries app wasm in its executable, so a `be` hosts with the build it
  was made with. This is new: today it carries only each app's manifest and
  type module (`apps.rs`, the `type_bundle` in its BUCK). It carries only the
  apps that declare a host component, not every app.
- Whatever be-host's headless `:dev` needs that this provides, it shares
  rather than keeping its own.
- plans/host_as_platform_adapter.md moves be-host-runtime's orchestration into
  a guest library. Whichever of the two lands second carries `be` along, so
  `be` keeps running apps the way be-host does.

## Order of work

1. **The runtime in `be`**, and bundling the apps that declare a host
   component, landed in three reviewed pieces:
   - a. Host components in the manifest and the app protocol: done
     (guides/apps/host_services.md, "Host components").
   - b. A headless driver for be-host-runtime, with no beui setup, no
     presenter and no gpu, which be-host's headless `:dev` shares.
   - c. `be` wiring (be-host-stack and be-host-runtime, the apps that declare
     `hosts` bundled) and a small example app with a host component.
   Done: `be` runs a test app's host component
   against a block and the test reads what it wrote, with no display.
2. **`be BLOCK host`.** Signs in (as an agent, or a block's agent, as the
   terminal host does), opens the block and runs its type's host component
   while it holds the lease, standing by otherwise. Done: plans/hosted_blocks.md
   step 4's headless test.
3. **Guides.** guides/apps/runtime.md and guides/common_commands.md say how
   `be` runs apps.
