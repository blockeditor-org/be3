# The deployed bundle uses fat LTO and keeps symbols and precompiled code

`./scripts/deploy` builds with fat LTO and one codegen unit (`be3.lto`, a ten-minute link),
release builds use legacy symbol mangling (buck/platforms/profile.bzl), linked modules use
hashed export names (guides/app_libraries.md), and the bundle ships a `.cwasm` of every
module with be-host's symbols and the modules' name sections kept.

## Why

Fat LTO is limited to the deploy because of its ten-minute link. The symbols and name sections are read by the panic guard's reports and a guest's
backtrace. Shipping the `.cwasm` files keeps compiling off the first start.

## Rejected

- Thin LTO: left be-host as it was.
- `opt-level=s` for wasm: took a twentieth off the apps while growing both libraries and
  beui-gallery by four fifths.
- Stripping be-host's symbols or the modules' name sections: would save 24 MB and 27 MB,
  but the panic guard's reports and a guest's backtrace read them.
- Shipping without `.cwasm`: it is 595 MB of the 896 MB bundle (October 2026), but shipping
  without it moves 95 seconds of compiling (four cores) to the first start, 42 of them the
  beui library's.
