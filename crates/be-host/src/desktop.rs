#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dev_token = std::env::var(be_host::DEV_TOKEN)
        .ok()
        .filter(|token| !token.is_empty());
    // No other thread runs yet. The token leaves the environment, so the programs and terminals
    // be-host starts do not inherit it.
    // unsafe(env)
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
        std::env::remove_var(be_host::DEV_TOKEN);
    };
    be_host::run(dev_token)
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn main() {}
