use std::process::{Command, ExitCode};

use serde_json::{Value, json};

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [rust_project, sysroot] = arguments else {
        return Err("usage: rust-project RUST_PROJECT SYSROOT".into());
    };
    let repository = std::env::current_dir().map_err(|error| error.to_string())?;
    let buck = repository
        .join("scripts/buck")
        .to_string_lossy()
        .into_owned();
    let develop = |extra: &[&str]| -> Result<Value, String> {
        let text = crate::output(
            Command::new(rust_project)
                .args([
                    "develop",
                    "--sysroot",
                    sysroot,
                    "--buck2-command",
                    &buck,
                    "--stdout",
                ])
                .args(extra),
        )?;
        serde_json::from_str(&text).map_err(|error| error.to_string())
    };

    println!("Reading the build graph...");
    let mut project = develop(&["//crates/..."])?;
    // Each app's guest library, by its directory: a target pattern cannot
    // match a prefix of a package name, and an app's other targets, such as
    // its :test, run natively.
    let mut apps: Vec<String> = std::fs::read_dir("crates")
        .map_err(|error| format!("could not list crates/: {error}"))?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("be-app-"))
        .map(|name| format!("//crates/{name}:wasm"))
        .collect();
    apps.sort();
    let mut guest_arguments: Vec<&str> = vec![
        "--mode=--target-platforms=root//buck/platforms:wasi_guest",
        "--rustc-target",
        "wasm32-wasip1-threads",
    ];
    guest_arguments.extend(apps.iter().map(String::as_str));
    guest_arguments.push("//crates/be-sdk:be-sdk");
    let guest = develop(&guest_arguments)?;

    let crates = project["crates"]
        .as_array_mut()
        .ok_or("rust-project wrote no crates")?;
    let offset = crates.len() as u64;
    for mut guest_crate in guest["crates"].as_array().cloned().unwrap_or_default() {
        for dependency in guest_crate["deps"].as_array_mut().into_iter().flatten() {
            dependency["crate"] = json!(dependency["crate"].as_u64().unwrap_or_default() + offset);
        }
        crates.push(guest_crate);
    }

    let mut runnables: Vec<Value> = project["runnables"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|runnable| runnable["kind"] != "flycheck")
        .collect();
    for runnable in &mut runnables {
        if runnable["program"] == "buck" {
            runnable["program"] = json!(buck);
        }
    }
    runnables.push(json!({
        "program": rust_project,
        "args": ["check", "--buck2-command", buck, "{saved_file}"],
        "cwd": repository,
        "kind": "flycheck",
    }));
    project["runnables"] = json!(runnables);
    let sysroot_src = format!(
        "{}/lib/rustlib/src/rust/library",
        project["sysroot"].as_str().unwrap_or_default()
    );
    project["sysroot_src"] = json!(sysroot_src);

    std::fs::write("rust-project.json", project.to_string()).map_err(|error| error.to_string())?;
    println!("Wrote rust-project.json.");
    Ok(ExitCode::SUCCESS)
}
