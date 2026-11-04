# `./scripts/verify` compiles each target in one configuration and runs no clippy

`./scripts/verify` compiles and checks every first-party target on `linux_x86_64` and
`wasi_guest`, beui for Windows, and the HTML core's libraries. It does not typecheck other
configurations (Android, the web's wasm32, host wasi, release copies) and does not run
clippy; clippy can still be run by hand (guides/buck2.md).

## Why

Each of those would be a second compile of every target, which doubled what a change cost.

## Rejected

- Typechecking the other configurations in verify: a second compile of every target.
- Running clippy in verify: a second compile of every target.
