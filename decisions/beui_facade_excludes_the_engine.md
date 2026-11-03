# The beui facade is authoring only; the engine lives in beui-host

`beui`, the crate every program and editor depends on, has no features and carries no renderer, fonts, inspector, drive or platform. Those are in `beui-host`, which re-exports beui and adds the engine under `beui_host::native` for whatever runs documents (programs, demos, test harnesses, the app runtime). `beui-runner` is likewise depended on only by the engine and hosts. Editors reach the engine in tests through be-sdk-test or a beui-host dev-dependency, and apps through the app runtime.

## Why

An edit to the renderer, fonts, inspector, drive, a platform or the runner then rebuilds no editor library.
