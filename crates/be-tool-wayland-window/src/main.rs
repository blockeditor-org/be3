//! A program window for be-host's host tests (plans/host_as_platform_adapter.md, 4g7e3): a
//! Wayland client that opens one xdg toplevel titled as its first argument, filled with one color
//! at whatever size the compositor configures it to (320 by 240 when it lets the window choose),
//! and runs until the compositor closes it or the connection ends. It reads the compositor from
//! `WAYLAND_DISPLAY`, as every program `drive exec` starts does.

#[cfg(target_os = "linux")]
mod window;

#[cfg(target_os = "linux")]
fn main() {
    let title = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Test window".to_owned());
    if let Err(why) = window::run(&title) {
        eprintln!("be-tool-wayland-window: {why}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("be-tool-wayland-window: runs only inside a Wayland compositor, on Linux");
    std::process::exit(1);
}
