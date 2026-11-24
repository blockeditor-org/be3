use std::path::Path;
use std::process::Command;

/// Draws a browser painting with be-tool-browser-rasterizer: `renderer` is the directory its
/// `:release` is laid out in (browser_program in buck/program/drive.bzl), as CI downloads it, and
/// without one it is built and run with `./scripts/buck run`.
pub fn rasterize(input: &Path, output: &Path, renderer: Option<&Path>) -> Result<(), String> {
    let mut command = match renderer {
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
    };
    let result = command
        .args([input, output])
        .output()
        .map_err(|error| error.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "browser rasterizer failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}
