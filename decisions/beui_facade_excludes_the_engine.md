# The beui facade is authoring only; the engine lives in beui-host

`beui`, the crate every program and editor depends on, has no features and carries no renderer, fonts, inspector, drive or platform. Those are in `beui-host`, which re-exports beui and adds the engine under `beui_host::native` for whatever runs documents (programs, demos, test harnesses, the app runtime). `beui-runner` is likewise depended on only by the engine and hosts. Editors reach the engine in tests through be-sdk-test or a beui-host dev-dependency, and apps through the app runtime. A program that opens a window of its own runs through `beui-host-window` (`run`, `run_with`), which holds the native runner and the platforms and layers only it needs (winit, Android, space, XR), so beui-host and everything tested through it do not depend on them.

## Why

An edit to the renderer, fonts, inspector, drive, a platform or the runner then rebuilds no editor library.

## The app runtime spells beui as everyone does, without the components

be-host-runtime runs app documents but builds no components, so it does not depend on the facade:
it depends on `beui-documents`, the facade without the components (types, `reactive`, views,
windows, `native`; none of the engine), under the name `beui` (`package = "beui-documents"` in its Cargo.toml), and on the
engine crates for the rest (beui-renderer-wgpu, beui-font-freetype, beui-layer-drive). `beui`
re-exports `beui-documents` at the same paths, so every item has one spelling in every crate:
`beui::Pos`, `beui::native::Context`. An edit to a component, the dock, the keyboard, the
inspector or a platform no longer rebuilds the runtime (17k lines) or its tests: 1.9% of the
library lines an average edit recompiles (November 2026). `buck/cargo/layers.bzl` holds it: a crate
outside beui depends on `beui-documents` only under the name `beui`, and never on both it and the
facade.

Rejected: spelling beui's items by their defining crate (`beui_types::Pos`) everywhere, which
gives one spelling too but has every editor depend on beui-types, beui-view and the rest and know which defines
each item, and saves nothing more.

