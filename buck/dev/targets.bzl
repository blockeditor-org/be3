# What //:verify builds that buck/cargo/crates.bzl cannot say: crates.bzl
# names every workspace crate, each with a :fix and, unless it is named below,
# a :test, and these name the rest. A rule cannot find targets by pattern, so
# each list is checked where its targets are defined, and a target missing
# from one fails the load that defines it: package_fix (buck/dev/verify.bzl),
# be_test (buck/dev/test.bzl), cargo_binary (buck/cargo/defs.bzl) and the
# wasm macros (buck/wasm/defs.bzl). A name
# here that is no target fails //:verify instead.

# The packages with no crate, each with its own :fix.
PACKAGES = [
    "root//:fix",
    "root//buck/android:fix",
    "root//buck/cargo:fix",
    "root//buck/constraints:fix",
    "root//buck/dev:fix",
    "root//buck/platforms:fix",
    "root//buck/program:fix",
    "root//buck/sysroot:fix",
    "root//buck/tools:fix",
    "root//buck/wasm:fix",
    "root//compile_fail:fix",
    "root//third-party/pdfium:fix",
    "root//third-party/rust:fix",
    "root//third-party/system:fix",
    "toolchains//:fix",
]

# The crates with no :test.
UNTESTED = [
    "crates/be-block-fault-relay",
    "crates/be-block-model-derive",
    "crates/be-host-accounts-html",
    "crates/be-host-bench",
    "crates/be-host-testing",
    "crates/be-lib-remap-module",
    "crates/be-sdk-beui-platform-api",
    "crates/be-sdk-database",
    "crates/be-sdk-library-base",
    "crates/be-sdk-library-beui",
    "crates/be-sdk-test-runner",
    "crates/be-sdk-type-api",
    "crates/be-sdk-type-host",
    "crates/be-server-bin",
    "crates/be-tool-browser-rasterizer",
    "crates/be-tool-paint-review-manifest",
    "crates/be-tool-paint-review-web",
    "crates/be-tool-tls-diamond-app",
    "crates/be-tool-tls-diamond-base",
    "crates/be-tool-tls-diamond-left",
    "crates/be-tool-tls-diamond-right",
    "crates/be-type-bundle",
    "crates/be-type-settings",
    "crates/be-wasm-gpu-shim",
    "crates/be-wasm-guest-abi",
    "crates/be-wasm-guest-echo",
    "crates/beui",
    "crates/beui-demo-html",
    "crates/beui-dock",
    "crates/beui-draw",
    "crates/beui-layer-drive",
    "crates/beui-platform-headless",
    "crates/beui-web-demo",
]

# Every be_test (buck/dev/test.bzl) but a crate's :test and the parts of a
# run_apart one (buck/cargo/defs.bzl), which its :test holds.
TESTS = [
    "root//compile_fail:beui-html-test",
    "root//compile_fail:beui-test",
    "root//crates/be-block-model:fuzz-lists-test",
    "root//crates/be-host-accounts-html:html-test",
    "root//crates/be-host-accounts-html:web-test",
    "root//crates/be-lib-sequence:fuzz-sequence-test",
    "root//crates/be-tool-tls-diamond-app:web-test",
    "root//crates/be-wasm-host:standalone-web-test",
    "root//crates/beui-demo-html:web-test",
    "root//crates/beui-tests:web-test",
    "root//crates/beui-view:html-test",
    "root//crates/beui:html-test",
]

# The tests built for a platform of their own (their default_target_platform)
# rather than linux_x86_64, which //:verify builds every other one for.
TEST_PLATFORMS = {
    "root//crates/be-host-accounts-html:html-test": "root//buck/platforms:linux_x86_64_html",
    "root//crates/be-server-portal-ui:test": "root//buck/platforms:linux_x86_64_html",
    "root//crates/beui-view:html-test": "root//buck/platforms:linux_x86_64_html",
    "root//crates/beui:html-test": "root//buck/platforms:linux_x86_64_html",
}

# The crates whose library is a wasm module alone (wasm_library and
# linked_app, buck/wasm/defs.bzl), so that it has no rust_library to check.
MODULES = [
    "crates/be-sdk-library-base",
    "crates/be-sdk-library-beui",
    "crates/be-tool-tls-diamond-app",
]

# A [[bin]] whose target is not cargo_binary's default name (`bin`, or the
# binary's own in a crate with several), by package and binary.
BINARY_NAMES = {
    "crates/be-tool-launcher": {"be-launcher": "native"},
    "crates/beui-demo": {"demo": "native"},
}
