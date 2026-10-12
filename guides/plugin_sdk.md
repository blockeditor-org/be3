# The plugin SDK

Every plugin is linked against one shared module, the plugin SDK (`crates/block-plugin-sdk`),
instead of carrying its own copy of std, beui, wgpu, FreeType, HarfBuzz and the rest. A plugin
module holds only its own code and the generics instantiated for its own types; the host links
it to the SDK when it loads it, on wasmtime and in the browser alike, from the same files.

## What is in it

`crates/block-plugin-sdk` is a crate that depends on what the plugins have in common: beui,
block-editor-beui, block-editor-plugin, serde, serde_json and uuid, and through them
everything underneath. All of it is linked into the SDK whole and exported, since the SDK cannot
know what a plugin it has not seen will use. A crate only one or two plugins use stays in those
plugins. To move one into the SDK, add it to the crate's `Cargo.toml` (and its `lib.rs`) and
run `./scripts/buck run //:buckify`.

Only the SDK's Rust is linked whole. The C libraries under it (wasi-libc, libc++abi,
libsetjmp) are linked for what the SDK uses, and the C math library is forced in
(`MATH` in `crates/wasm-link/src/lib.rs`). A plugin that calls a C function the SDK does not
have fails to link, with the function named; add it there.

A plugin that links a library that is not position-independent cannot be linked against the SDK
(the PDF plugin's PDFium is an Emscripten archive): `editor(..., shared_sdk = False)` builds it
whole, as every plugin used to be, and the hosts load either kind.

## Identity

An SDK is one exact build: Rust has no stable ABI, so a plugin works only with the SDK it was
linked against. The SDK's identity is the SHA-256 of its module. A plugin says which it needs
with an exported global named `__be3_sdk_<identity>`, and the SDK is staged beside the plugins as
`sdk-<identity>.wasm` (and `.cwasm` where plugins are precompiled; `buck/app/stage.sh`), so any
number of plugins built against one SDK share one file and one compiled module.

## How the links are made

Every wasm link runs through `crates/wasm-link` (the toolchain's linker in `buck/tools`), which
otherwise passes the link to rust-lld unchanged.

- `--be3-sdk` (the SDK's `linker_flags`, from `plugin_sdk()` in `buck/wasm/defs.bzl`): links
  every Rust object whole and exports everything (`--export-all`), with a growable exported
  function table. It writes a `be3.sdk` custom section listing the archives and libraries it
  linked and the SDK's thread-locals.
- `--be3-plugin=SDK` (each editor's `linker_flags`): links the plugin as a position-independent
  shared module (`-shared`), leaving out every archive the SDK linked, so what the plugin uses of
  them becomes imports. It then checks that the SDK exports every import, and adds the identity
  and an export `__be3_dylink_<memory size>_<memory align>_<table size>_<table align>`, which a
  host reads from the compiled module before instantiating it.

Every crate on the plugins' platform is already compiled position-independent (buck2's default),
so nothing else changes about how they build.

## Thread-locals

LLVM compiles a thread-local access as an offset from the module's own `__tls_base`
(local-exec) on every target but Emscripten, and that offset cannot point into another module.
A plugin reaches the SDK's thread-locals through generics it instantiates, so the two share one
thread-local block instead:
- The SDK's link ends its thread-locals with a reserve, `__be3_plugin_tls`
  (`PLUGIN_TLS_SIZE`, 16 KiB), in every thread's block.
- A plugin's link starts its thread-locals with a generated object that defines each of the
  SDK's at the SDK's offset, so the plugin's own come after them, inside the reserve.
- The host points the plugin's `__tls_base` at the SDK's block for each thread and copies the
  plugin's own initial values into the reserve. The link fails if a plugin's own thread-locals
  outgrow it.

## Loading

`crates/block-wasm-host/src/dylink.rs` (wasmtime) and `crates/block-app/web/dylink.js` (the
browser) do the same things:
1. Instantiate the SDK against the plugin's shared memory, and run its constructors.
2. Plan: allocate the plugin's memory with the SDK's `malloc`, and pick table slots after the
   SDK's own for the plugin's functions and for every function whose address it takes.
3. Instantiate the plugin, answering its `env` imports with the SDK's exports, `GOT.mem` with
   addresses and `GOT.func` with table slots, and apply its data relocations.
4. Set up thread-local storage as above, then run the plugin's constructors.

A thread the plugin spawns instantiates the SDK and then the plugin again from the same plan,
with the thread-local block wasi-libc made for it (read from its start argument). Everything is
linked when the plugin starts, before any thread exists, since a table cannot be shared between
threads: there is no loading of another module later.

Natively the host keeps one compiled SDK per identity (`Host` in block-wasm-host; one wasmtime
engine per cache directory), so every plugin after the first only compiles itself. In a browser
each plugin's worker fetches and compiles the SDK itself.
