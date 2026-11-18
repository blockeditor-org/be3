# Headless apps

Depends on: none.

`be` (`crates/be-tool-cli`) holds blocks through type modules, as be-host's
peer does, but runs no apps: the terminal's host is native code of its own
(guides/terminal.md). A hosted block's host is an app's host component
(plans/hosted_blocks.md, "The host component"), so a server that hosts one
needs `be` to run ordinary app wasm, the same modules be-host stages, with no
graphics. One runtime, not a second one for `be`.

## Shape

- `be` runs apps through be-host-runtime's `Runtime` and the V8 backend (`wasm.rs`)
  (guides/apps/runtime.md, Backends), with a host that has no compositor, no
  windows and no gpu: an app instance there gets no frames and no input, and
  answers host services from be-host-stack as be-host does, with the calls
  that need a person or a screen (pickers, dialogs) refused.
- An app that asks for the gpu there gets an error it can handle; a host
  component never needs one.
- `be` finds its apps as be-host does, with no app's wasm, manifest, type
  module, list or hash compiled in: a build made here reads those staged
  beside it (`be-tool-cli:dev`), and a deployed `be` downloads the manifests,
  type modules and apps it uses from its own release, checked against the
  list of hashes its `build-info.json` pins and cached by them
  (guides/apps/runtime.md, "Headless"), so `be` stays one small download.
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
     presenter and no gpu (guides/apps/runtime.md, "Headless"), and a host
     component kept to its own block (guides/apps/host_services.md): done.
   - c. `be` wiring (be-host-stack and be-host-runtime; the host app staged
     beside a `be` built here, `be-tool-cli:dev`, or downloaded from a
     deployed `be`'s release and checked against the `app-hashes.json` its
     `build-info.json` pins) and a small example app with a host component:
     done (guides/apps/runtime.md, "Headless"; be-tool-cli's
     `be_runs_an_apps_host_component_with_no_display`). Not yet run against a
     real deploy: only releases deployed after it name their `apps`.
   - d. No app list in `be` at all (the user's decision: "the app list should
     not be in be at all"): done. `be` reads manifests and type modules
     through `be_host_stack::discovery` (`be_host_stack::types::app_sources`),
     from the bundle beside it or its release as in c, so an app changing
     never relinks `be` (be-tool-cli's
     `be_reads_a_block_type_from_the_type_module_staged_beside_it`). Like c,
     not yet run against a real deploy. A development build's terminal host
     uploads `be-tool-cli:terminal-host`, `be` with every manifest and type
     module beside it (guides/terminal.md).
2. **`be BLOCK host`** (done). Signs in (as an agent, or a block's agent, as the
   terminal host does), opens the block and runs its type's host component
   while no other device hosts it, claiming the lease as it starts, standing by
   otherwise (guides/apps/runtime.md, "Headless");
   `be_hosts_a_block_while_no_other_device_does`. Not yet run against a real
   deploy. Left: plans/hosted_blocks.md step 4's acceptance test.
3. **Guides.** guides/apps/runtime.md and guides/common_commands.md say how
   `be` runs apps: done for what step 1 built (guides/apps/runtime.md,
   "Headless"; `be-tool-cli:dev` in guides/common_commands.md). Step 2's
   command is in both.
