# `./scripts/verify` compiles each target in one configuration and runs no clippy

`./scripts/verify` compiles every first-party target on `linux_x86_64` and `wasi_guest`:
what its tests build, and rustc's check pass over the rest. It does not typecheck other
configurations (Windows, macOS, Android, the web's wasm32, host wasi, beui's HTML core beyond
its tests, release copies) and does not run clippy; each can still be checked by hand
(guides/buck2.md, guides/build_server.md).

## Why

Each of those would be a second compile of every target, which doubled what a change cost,
and the build server is slow. Windows and the HTML core's libraries were checked until the
move off Namespace, and dropped then for the same reason.

## Rejected

- Typechecking the other configurations in verify: a second compile of every target.
- Running clippy in verify: a second compile of every target.
