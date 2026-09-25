use std::path::Path;
use std::process::{Command, ExitCode};

use serde_json::{Value, json};

const GENERATOR: &str = "@rules_rust//tools/rust_analyzer:gen_rust_project";

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [bazel] = arguments else {
        return Err("usage: rust-project BAZEL".into());
    };
    let repository = std::env::current_dir().map_err(|error| error.to_string())?;
    let scratch = repository.join("target/rust-project");
    std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;

    println!("Reading the build graph...");
    let mut project = generate(bazel, &repository, &scratch, None, &["//crates/..."])?;
    let guest_bazel = scratch.join("bazel-wasi-guest");
    std::fs::write(
        &guest_bazel,
        format!(
            "#!/bin/sh\nstartup=''\nwhile [ $# -gt 0 ]; do\n    case \"$1\" in\n        --*) startup=\"$startup $1\"; shift ;;\n        *) break ;;\n    esac\ndone\ncommand=\"$1\"\nshift\nexec '{bazel}' $startup \"$command\" --platforms=//bazel/platforms:wasi_guest \"$@\"\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    make_executable(&guest_bazel)?;
    let guest = generate(
        bazel,
        &repository,
        &scratch,
        Some(&guest_bazel),
        &["//crates/editors/...", "//crates/block-editor-plugin:all"],
    )?;

    let crates = project["crates"]
        .as_array_mut()
        .ok_or("gen_rust_project wrote no crates")?;
    let offset = crates.len() as u64;
    for mut guest_crate in guest["crates"].as_array().cloned().unwrap_or_default() {
        for dependency in guest_crate["deps"].as_array_mut().into_iter().flatten() {
            dependency["crate"] = json!(dependency["crate"].as_u64().unwrap_or_default() + offset);
        }
        crates.push(guest_crate);
    }

    std::fs::write("rust-project.json", project.to_string()).map_err(|error| error.to_string())?;
    println!("Wrote rust-project.json.");
    Ok(ExitCode::SUCCESS)
}

fn generate(
    bazel: &str,
    repository: &Path,
    scratch: &Path,
    wrapper: Option<&Path>,
    targets: &[&str],
) -> Result<Value, String> {
    let mut command = Command::new(bazel);
    command
        .arg("run")
        .arg(GENERATOR)
        .arg("--")
        .arg("--workspace")
        .arg(repository);
    if let Some(wrapper) = wrapper {
        command.arg("--bazel").arg(wrapper);
    } else {
        command.arg("--bazel").arg(bazel);
    }
    crate::status(command.args(targets))?;
    let written = repository.join("rust-project.json");
    let kept = scratch.join(format!("{}.json", targets.len()));
    std::fs::rename(&written, &kept).map_err(|error| format!("{}: {error}", written.display()))?;
    crate::json(&kept.to_string_lossy())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}
