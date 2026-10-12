#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    block_desktop::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {}
