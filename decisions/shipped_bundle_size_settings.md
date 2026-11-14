# The deployed bundle uses fat LTO, type modules at opt-level s, and keeps symbols and precompiled code

`./scripts/deploy` builds with fat LTO and one codegen unit (`buck/constraints:lto`, a ten-minute link),
release builds use legacy symbol mangling and build first-party crates for
wasm32-unknown-unknown (the type modules) at `opt-level=s` (buck/platforms/profile.bzl), linked modules use
hashed export names (guides/app_libraries.md), and the bundle ships a `.cwasm` of every
module with be-host's symbols and the modules' name sections kept.

## Why

Fat LTO is limited to the deploy because of its ten-minute link. The symbols and name sections are read by the panic guard's reports and a guest's
backtrace. Shipping the `.cwasm` files keeps compiling off the first start. Type modules build
at `s`: a quarter to a third smaller and quicker to compile, for a slower seal of a very large
edit, measured in buck/platforms/profile.bzl.

## Rejected

- Thin LTO: left be-host as it was.
- `opt-level=s` or `z` for apps and their libraries: measured again in November 2026, `z`
  took two fifths off the apps and a tenth off the libraries, but the canvas's slowest frame
  took four times as long, and at `s` apps' frames were a fifth to a third slower
  (buck/platforms/profile.bzl). An earlier toolchain-wide `s` took a twentieth off the apps
  while growing both libraries and beui-gallery by four fifths.
- Stripping be-host's symbols or the modules' name sections: would save 24 MB and 27 MB,
  but the panic guard's reports and a guest's backtrace read them.
- Shipping without `.cwasm`: it is 595 MB of the 896 MB bundle (October 2026), but shipping
  without it moves 95 seconds of compiling (four cores) to the first start, 42 of them the
  beui library's.
