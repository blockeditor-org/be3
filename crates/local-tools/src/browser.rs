use std::path::Path;
use std::process::Command;

pub fn rasterize(input: &Path, output: &Path, renderer: Option<&Path>) -> Result<(), String> {
    let mut command = match renderer {
        Some(renderer) => Command::new(renderer),
        None => {
            let mut command = Command::new("./scripts/buck");
            command.args(["run", "//crates/browser-rasterizer:bin", "--"]);
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
