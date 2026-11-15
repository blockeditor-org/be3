// The root of an app's test crate when its tests are compiled apart from the
// app (`app(tests_apart = True)` in buck/wasm/defs.bzl): the app's items, so
// the tests' `crate::` paths name them, and the tests themselves, from the
// package's src/tests.rs.
pub use app::*;

mod tests;
