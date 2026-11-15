// v8's build script, which links the prebuilt V8 that third-party/v8 pins for
// the platform being built (RUSTY_V8_ARCHIVE, unpacked) rather than
// downloading one or building V8 from source, and hands the crate the Rust
// bindings the crate ships for that platform (src/binding.rs, overlaid too),
// or RUSTY_V8_SRC_BINDING's where it ships none (Android).
// Cargo's own script keeps both outside OUT_DIR, which buck does not keep.
// The link flags are the ones its print_link_flags gives with the default
// use_custom_libcxx feature: V8 brings its own libc++, so none is linked.
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    let windows = env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows";
    let library = if windows { "rusty_v8.lib" } else { "librusty_v8.a" };
    fs::copy(env::var("RUSTY_V8_ARCHIVE").unwrap(), out.join(library)).unwrap();
    let binding = match env::var("RUSTY_V8_SRC_BINDING") {
        Ok(binding) if !binding.is_empty() => PathBuf::from(binding),
        _ => PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join(format!("gen/src_binding_release_{target}.rs")),
    };
    fs::copy(&binding, out.join("src_binding.rs"))
        .unwrap_or_else(|error| panic!("{}: {error}", binding.display()));

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=rusty_v8");
    if windows {
        println!("cargo:rustc-link-lib=dylib=winmm");
        println!("cargo:rustc-link-lib=dylib=dbghelp");
        println!("cargo:rustc-link-lib=dylib=msvcprt");
    }
}
