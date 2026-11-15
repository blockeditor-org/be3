// The root of an app's module when its tests are compiled apart
// (`app(tests_apart = True)` in buck/wasm/defs.bzl): it links the app's rlib,
// the same one the tests compile against, so the app is compiled once.
pub use app::*;
