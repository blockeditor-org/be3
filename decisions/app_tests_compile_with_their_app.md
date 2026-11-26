# An app's tests compile with the app

An app's tests stay unit tests of its crate: `app_tests` (buck/wasm/defs.bzl) compiles the crate again with its tests for the test module, beside the compile of the crate for `:module`. So every edit beneath an app compiles the app twice.

## Why

Piloted in November 2026: the app compiled once as an rlib, which both a one-line cdylib for `:module` and a test crate of its own (a root file plus the package's `src/**/tests*`) linked, the rlib compiled with `-Zdefault-visibility=hidden` so the module exported exactly what it did before. buck2 compiles a binary or cdylib against a dependency's real rlib, not its `-Zno-codegen` metadata, so it added no frontend pass. Measured from the build server's records after a no-op edit to the app (wall time, noisy by about a third):

- checklist (about 400 library lines): 6 to 8 s before, 4 to 10 s after; nothing measurable.
- infinite-canvas (15.6k lines): 33 to 41 s before, 19 to 29 s after, the test module's 19 to 28 s replaced by a test crate of 7 to 9 s. Its tests used 29.6 CPU s against 26.4, the app's code being at opt-level 0 in them rather than 1.
- Over all 28 apps (76k library lines), estimated at 50 to 70 s of wall time for each edit under beui or the SDK, most of it from the five largest apps.

It was not adopted because:

- Everything a test reaches must be `pub`: about 30 modules, functions and fields in infinite-canvas. rustc does not warn about an unused `pub` item of a library, so the apps would lose their dead-code warnings for all of it, and their encapsulation.
- 12 of 28 apps have white-box tests inside their modules (`src/root/tests.rs` and the like, reaching private items through `super::*`), and 13 have `#[cfg(test)]` code in their library; each would have to be rewritten as a black-box test.
- Adopting it in only the apps where it pays would leave two ways an app's tests are built.

## Reopen when

- A lint reports unused `pub` items in app crates (rustc's `unreachable_pub` with a dead-code pass for them, or an equivalent in be-tool-fix-rust-source), so making test-reached items public costs no warnings; or
- apps grow several times larger, so that the saving dominates a verify; or
- buck2 or rustc can compile a crate's tests against its already-compiled library while keeping private access.

The pilot is on the branch `claude/app-tests-apart` (commit 49fa8dd89).
