# The deployed bundle keeps symbols (and once built type modules at opt-level s, with fat LTO and precompiled code)

Release builds use legacy symbol mangling and build first-party crates for
wasm32-unknown-unknown at `opt-level=s` (buck/platforms/profile.bzl), which is now only the
browser's gpu shim: the type modules build at the apps' 3 (Since, below). Linked modules use
hashed export names (guides/app_libraries.md), and the bundle ships a `.cwasm` of every
module with be-host's symbols and the modules' name sections kept.

Dev builds use legacy mangling too, for a reason of their own: symbol names were the largest
part of a native test binary, and the build server stores every test binary a change relinks
(the sizes are in buck/platforms/profile.bzl, `SYMBOL_FLAGS`).

## Why

The symbols and name sections are read by the panic guard's reports and a guest's
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

## Since

November 2026: every guest now runs on V8 and precompilation was removed (decisions/guests_run_on_v8.md),
so the bundle ships no `.cwasm` and V8 compiles each module as it loads. Shipping precompiled code
is superseded, with the rejection of shipping without it; the mangling, opt-level and
symbol settings still stand.

November 2026: the pure guests (type modules, games, the input remaps) are built for
wasm32-wasip1 in the apps' plan (plans/pure_guests_in_the_apps_plan.md), so a crate they share
with the apps compiles once, at the apps' release opt-level 3 (option a). Type modules no longer
build at `s` and ship a third bigger again: the release checklist module 1,895,013 bytes to
2,497,166, text's 1,957,971 to 2,566,523. `s` stays only for the gpu shim, the one crate left on
wasm32-unknown-unknown. The owner may still choose (b), `s` for every guest, the apps' crates
too, against the frame times above, or (c), `s` for each pure guest's own crates (its
`be-type-*` or `tabletop-rules-*` crate and `:wasm`) while the crates it shares stay at 3.

November 2026: the deploy's fat LTO over one codegen unit was removed (decisions/no_lto.md), with
the rejection of thin LTO here.
