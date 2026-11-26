# A root is a staged app flagged `"root": true`

The root guest `be-host --root=APP` runs in place of its own UI
(plans/host_as_platform_adapter.md, The root guest) is staged as an app is: a
manifest and an entry point, found by the host's app discovery, whose manifest
says `"root": true` (`AppManifest::root`). A root declares no editors, block
types or host components, and be-host refuses to run an app without the flag as
a root.

## Why

- The root spawns the apps it embeds by id through the sandbox, which finds
  them with the staged-app loader (`staged_app`); the root is found the same
  way, so there is one way to stage and find a module, natively and later on the
  web.
- A test stages a root as it stages an app: crates/be-host-tests names its
  package's label in `APPS`, and only that test's host holds it. To be staged so,
  the package needs what `program` zips per app beside `:manifest` and
  `:module`: an empty `app_data` (`:data`) and `app_types` (`:types`), as
  crates/be-sdk-root-testing/BUCK has.
- The flag keeps the manifest's rule that an app declares an editor or a block
  type, which catches a mistaken manifest, and lets be-host refuse to run an
  ordinary app as its root.

## Rejected

- **A package kind of its own** (a `root()` rule and its own discovery): a
  second way to stage and find a module, for what differs from an app only in
  having no editors.
- **A module built into be-host**: the root is meant to be swapped (the
  compositor, Space, a test's root), and a built-in one could not be staged for
  a test.
- **Relaxing the rule that an app declares an editor or a block type**: a
  manifest that declares nothing by mistake would no longer be caught, and be-host
  could not tell a root from an app.
