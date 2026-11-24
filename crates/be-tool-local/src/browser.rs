use std::path::Path;
use std::process::Command;

/// The browser rasterizer that draws a browser painting (`Painting::render`): the one at
/// `built`, or else crates/be-tool-browser-rasterizer built and run through buck.
pub fn renderer(built: Option<&Path>) -> Command {
    match built {
        Some(renderer) => Command::new(renderer),
        None => {
            let mut command = Command::new("./scripts/buck");
            command.args(["run", "//crates/be-tool-browser-rasterizer:bin", "--"]);
            command
        }
    }
}
