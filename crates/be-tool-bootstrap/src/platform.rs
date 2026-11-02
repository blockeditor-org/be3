use std::env::consts::{ARCH, OS};

pub const EXE: &str = std::env::consts::EXE_SUFFIX;

pub fn triple() -> &'static str {
    match (ARCH, OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        ("aarch64", "windows") => "aarch64-pc-windows-msvc",
        _ => "unknown",
    }
}

pub fn target_platform() -> Option<&'static str> {
    Some(match (ARCH, OS) {
        ("x86_64", "linux") => "linux_x86_64",
        ("aarch64", "linux") => "linux_arm64",
        ("x86_64", "macos") => "macos_x86_64",
        ("aarch64", "macos") => "macos_arm64",
        ("x86_64", "windows") => "windows_x86_64",
        ("aarch64", "windows") => "windows_arm64",
        _ => return None,
    })
}
