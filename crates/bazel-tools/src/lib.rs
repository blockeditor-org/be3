pub mod apk;
pub mod clippy;
pub mod resolve;
pub mod rust_project;
mod starlark;

use std::process::Command;

pub(crate) fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))
}

pub(crate) fn json(path: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(&read(path)?).map_err(|error| format!("{path}: {error}"))
}

pub(crate) fn status(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{:?}: {error}", command.get_program()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{:?} failed: {status}", command.get_program()))
    }
}

pub(crate) fn output(command: &mut Command) -> Result<String, String> {
    let output = command
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|error| format!("{:?}: {error}", command.get_program()))?;
    if !output.status.success() {
        return Err(format!(
            "{:?} failed: {}",
            command.get_program(),
            output.status
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}
