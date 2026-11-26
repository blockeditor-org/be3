# The real root is its own crate, not the testing root

The root be-host runs in place of its own UI, and then by default, is a guest
crate of its own, `crates/be-root` (plans/host_as_platform_adapter.md, 4g7a).
be-sdk-root-testing stays the fixture be-wasm-root-host's and be-host's tests
run. What 4g6 built in the testing root to prove each area of `be3:root`
moves into the real root rather than being copied, and the testing root keeps
only what its own tests check.

## Why

- The testing root is shaped for tests, not people: an F-key for each dialog
  (F2 to F12), one dialog surface of a fixed 152 points that several dialogs
  overflow, a triangle app at a fixed origin, and fixtures such as
  `UNLOCKS_TWICE` that hand over a stray password on purpose. None belongs in
  the product.
- The real root is worded and laid out as be-host's own UI is today
  (`src/ui`), so the switch to it changes nothing a person sees; the testing
  root's paintings are its own and need not follow be-host's.
- Each keeps one job: a change to the product does not repaint the fixture's
  paintings, and a test of the platform does not wait on the product's UI.

## Rejected

- **Growing the testing root into the real root in place:** less moving, but
  its test-only keys, sizes and fixtures would ship, or be fenced off behind
  flags that are two roots in one crate.
- **Copying the 4g6 areas into a new crate and keeping them in the testing
  root too:** two copies of each area to keep in step, which is what moving
  them avoids.
