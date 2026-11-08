# `./scripts/verify` compiles each target in one configuration and runs no clippy

`./scripts/verify` compiles every first-party target on `linux_x86_64` and `wasi_guest`:
what its tests build, and rustc's check pass over the rest. It does not typecheck other
configurations (Windows, macOS, Android, the web's wasm32, host wasi, beui's HTML core beyond
its tests, release copies) and does not run clippy; each can still be checked by hand
(guides/buck2.md, guides/build_server.md), and all of them at once by the broad check
(.github/workflows/broad.yml, buck/dev/broad.bxl), a workflow that runs only when dispatched
by hand: on no push or pull request, so it costs a change nothing.

## Why

Each of those would be a second compile of every target, which doubled what a change cost,
and the build server is slow. Windows and the HTML core's libraries were checked until the
move off Namespace, and dropped then for the same reason.

## Rejected

- Typechecking the other configurations in verify: a second compile of every target.
- Running clippy in verify: a second compile of every target.
- Running the broad check on every pull request or push: the same cost, moved to CI.
