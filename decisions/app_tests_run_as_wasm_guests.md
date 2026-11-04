# An app's tests run as the wasm guest it ships as

An editor's tests, and those of beui-demo and the other crates that paint as apps do, are compiled to wasm32-wasip1-threads and run through crates/be-sdk-test-runner, the same wasmtime runtime be-host opens an app with, with the app's own imports linked. Each crate's BUCK runs its tests in exactly one place: `app_tests` as wasm for these crates, `cargo_test(paints = True)` natively for the host's own crates, whose paintings are what the host draws. See guides/testing_a_gui.md, 5.

## Why

- What the editor paints is then what the shipped app paints, not what the machine running the tests happens to have installed: beui shapes and rasterizes glyphs with the HarfBuzz, FreeType and fonts it carries once compiled to wasm, so paintings compare exactly on any machine.
- A painting is compared wherever its test runs, so running a crate's tests in one place keeps one set of paintings.

## Rejected

- Building and testing an editor package natively by itself: an editor's tests mean something only as the wasm guest it ships as.
