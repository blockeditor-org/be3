# Nothing is linked with LTO

No build links with link-time optimisation: `./scripts/deploy` ships be-host from the plain
release profile, and there is no `-m lto` or LTO constraint to ask for one. Every rlib is compiled
with `-Cembed-bitcode=no` (buck/toolchains/BUCK).

## Why

be-host's fat LTO link over one codegen unit took about ten minutes and about 10 GB on one
worker. During a deploy it was a long tail with only a few actions running while the rest of the
build server sat idle, and beside other actions it pushed the server into swap
(servers/build/default.nix). The deploy is quicker and the server is used evenly
without it.

## What it gave up

Measured in October 2026, fat LTO took a tenth off be-host's code and two thirds off its symbols
(159 MB to 113 MB), and some runtime speed, never measured on its own. The apps were never
linked with LTO, so they are unchanged.

## Re-evaluating

Re-adding it is a revert of the commit that removed it: a constraint in buck/constraints, its
modifier in buck/platforms/profile.bzl, `-Clto=fat -Ccodegen-units=1` on be-host's binary under
it, `-Cembed-bitcode` left on under it, and the deploy's native transitions adding it. Measure
first what it buys at runtime (guides/performance.md), and whether thin LTO or more codegen units
give most of that for a shorter link.

## Rejected

- Keeping `-m lto` as an unused switch: one way to build a release, and a switch nothing runs
  goes stale.
