# A thorough step beside verify

Depends on: none. Waiting for the owner to confirm which tests move.

`./scripts/verify` reruns the web host tests on nearly every change, since they depend on
be-host's `:web` bundle and every app in it, and they are the costliest tests it runs
(guides/performance.md, Keeping verify cheap). This moves the tests that stay expensive after
being made cheaper into a second step, run by hand when a change is likely to break them, and by
CI on a schedule.

## What moves

Measured on the build server in November 2026, in CPU seconds a run, after their pages went from
1280×800 to 960×600 (a quarter less):

| Test | CPU s | Why it stays expensive |
|---|---|---|
| `//crates/be-host:push-web-host-test` | about 175 | two accounts, each the whole web host with every app in Chromium on SwiftShader |
| `//crates/be-host:web-host-test` | about 78 | the whole web host, a canvas, a text and the debug view |

Together about 250 CPU seconds of every verify that changes be-host or an app, a fifth of the
CPU all tests took when every test reran. Not moved: beui-tests' and beui-demo-html's
`:web-test` (35 and 23 CPU s), the only check of beui's HTML core, which rerun only when beui
changes; and the host tests, which are cheap one by one.

## How

- `be_test` takes `labels`; a test that moves gets `labels = ["thorough"]` (`web_host_test` in
  buck/program/drive.bzl passes it).
- buck/dev/verify.bxl's `_tests` leaves out tests labelled `thorough` unless the part
  `thorough` is asked for, which builds only those tests' `[run]`.
- `./scripts/verify --thorough` adds that part: still one buck2 invocation
  (crates/be-tool-bootstrap's verify.rs takes the flag as it takes `--app-tests`). Without the
  other flags it runs only the thorough tests; with them, both.
- `./scripts/test` is unchanged: naming a thorough test runs it.
- CI: a scheduled workflow (daily) runs `./scripts/ci --thorough` on main and opens an issue
  when it fails. Pull requests do not run it, unless labelled.
- guides/buck2.md (What verify builds) and guides/testing_a_gui.md name the label, what carries
  it and how to run it, so the coverage is not lost silently.

## Rejected

- Dropping the tests: they are the only end-to-end check of the web page.
- Running them on every pull request in CI only: CI uses the same build server.
