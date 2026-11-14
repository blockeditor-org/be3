# App tests linked against the libraries

Depends on: none.

An app's tests (`app_tests` in buck/wasm/defs.bzl) are one standalone wasm module with
everything in it, compiled by rustc and then precompiled by Cranelift (`wasm_test_harness`). A
change under beui recompiles about thirty of them. Each holds the same base and beui code again:
checklist's test module has 25.7 MB of code in 56,856 functions, where checklist's app linked
against the libraries has 2.5 MB (guides/app_libraries.md). Its rustc took 95 CPU seconds and
its precompile 60 (November 2026, guides/performance.md), for five tests that run in 0.14 s.
Thirty such modules are about 4,500 CPU seconds of every verify that changes beui, more than all
the tests together.

## The change

Link each app's test module against `DEFAULT_LIBRARIES` (or its app's `libraries`), as the app
itself is, so that base and beui are compiled and precompiled once for every test and the host
(the `.cwasm` be-host's `:precompiled` already makes), and each test module holds only its own
crate, its tests, be-sdk-test and the generics they instantiate. The tests then run against the
very libraries the app ships with.

- **Link.** rustc `--test` makes a binary; crates/be-wasm-link links a module that names
  libraries `-shared`. It drops `crt1-command.o` for a linked binary and exports
  `__main_argc_argv`, which be-wasm-host calls once the libraries' `_initialize` has run (std on
  WASI reads its arguments through `args_get`, so the call needs none).
- **Run.** be-sdk-test-runner instantiates the closure as be-wasm-host does for an app
  (src/dylink.rs), then the test module, and runs it on the same thread as now; a panic still
  ends the instance, and the rerun one at a time is unchanged. `wasm_test_harness` passes the
  libraries' `.cwasm` beside the module's.
- **Check.** A test crate's dependency that a library holds is left out of the module, as for an
  app; a dependency only tests use stays in it. The linker's duplicate check still applies.

## Expected

If a test module shrinks as the app's did, to about a tenth, each app's precompile falls from
about 60 to under 10 CPU seconds, and its rustc by the link of 50 MB, roughly 1,500 to 2,000
CPU seconds off a verify that changes beui. Measure checklist first: build its test module both
ways and compare rustc and precompile CPU (`/proc/stat` around a local build, guides/performance.md).

## Rejected

- Winch for the precompile: it does not support the exception handling apps are built with.
- Cranelift's single-pass register allocator: no faster here (74 CPU s against 71), and it ran a
  deep paint out of stack before.
- Opt-level 2 for the test crate: twice the rustc time for a module a little smaller (opt-level 1
  is taken: same rustc, 15% less precompile).
