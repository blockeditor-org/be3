fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    println!("cargo:rustc-env=BEUI_DEMO={manifest}/../beui/examples/demo.rs");
    println!("cargo:rerun-if-changed=../beui/examples/demo.rs");
}
