use std::path::Path;
use std::process::Command;

/// The browser rasterizer that draws a browser painting (`Painting::render`): the one in
/// `built`, the directory its `:release` is laid out in (browser_program in
/// buck/program/drive.bzl) as CI downloads it, or else built and run through buck.
pub fn renderer(built: Option<&Path>) -> Command {
    match built {
        Some(renderer) => Command::new(renderer.join("browser-rasterizer")),
        None => {
            let mut command = Command::new("./scripts/buck");
            command.args([
                "run",
                "//crates/be-tool-browser-rasterizer:browser-rasterizer",
                "--",
            ]);
            command
        }
    }
}
