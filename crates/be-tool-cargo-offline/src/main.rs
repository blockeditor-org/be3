//! buckify's action (buck/cargo/buckify.bzl): a copy of the workspace that cargo
//! plans offline, from the crates third-party/rust/downloads.bzl pins as a
//! vendored directory, handed to crates/be-tool-buck's plan, which writes the
//! crates' facts into OUT/files. When cargo cannot bring Cargo.lock up to date
//! from those crates, such as when a new dependency needs one they lack, OUT/stale
//! says why in place of the facts: the bootstrap's lock step, which needs the
//! network, is what pins it (crates/be-tool-bootstrap/src/lock.rs).
//!
//! It depends on nothing from crates.io, so buck2 builds it from the checked-in
//! facts even when they are too stale to build be-tool-buck, which it then
//! builds with cargo (--cargo-build).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: cargo-offline OUT WORKSPACE PLACEHOLDERS CARGO NIGHTLY (--tools EXECUTABLE | --cargo-build) [PLAN=TRIPLE]... -- [KEY=SHA256=DIRECTORY]...";

type Result<T> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("cargo-offline: {message}");
            ExitCode::FAILURE
        }
    }
}

enum Tools {
    Built(PathBuf),
    CargoBuild,
}

fn absolute(path: &str) -> Result<PathBuf> {
    std::path::absolute(path).map_err(|error| format!("{path}: {error}"))
}

fn run(arguments: &[String]) -> Result<()> {
    let [
        out,
        workspace,
        placeholders,
        cargo,
        nightly,
        tools,
        rest @ ..,
    ] = arguments
    else {
        return Err(USAGE.to_owned());
    };
    let (tools, rest) = match tools.as_str() {
        "--tools" => {
            let (executable, rest) = rest.split_first().ok_or(USAGE)?;
            (Tools::Built(absolute(executable)?), rest)
        }
        "--cargo-build" => (Tools::CargoBuild, rest),
        _ => return Err(USAGE.to_owned()),
    };
    let split = rest
        .iter()
        .position(|argument| argument == "--")
        .ok_or(USAGE)?;
    let (cross, crates) = (&rest[..split], &rest[split + 1..]);
    let out = absolute(out)?;
    let cargo = absolute(cargo)?;
    let scratch = std::env::temp_dir().join(format!("be3-cargo-offline-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let result = prepare_and_plan(
        &Paths {
            out: &out,
            workspace: &absolute(workspace)?,
            placeholders: &absolute(placeholders)?,
            cargo: &cargo,
            nightly: &absolute(nightly)?,
            scratch: &scratch,
        },
        &tools,
        cross,
        crates,
    );
    let _ = fs::remove_dir_all(&scratch);
    result
}

struct Paths<'a> {
    out: &'a Path,
    workspace: &'a Path,
    placeholders: &'a Path,
    cargo: &'a Path,
    nightly: &'a Path,
    scratch: &'a Path,
}

fn prepare_and_plan(
    paths: &Paths,
    tools: &Tools,
    cross: &[String],
    crates: &[String],
) -> Result<()> {
    let vendor = paths.scratch.join("vendor");
    let workspace = paths.scratch.join("workspace");
    let cargo_home = paths.scratch.join("cargo-home");
    create_dir(&vendor)?;
    create_dir(&cargo_home)?;
    create_dir(paths.out)?;

    // The crates as a cargo directory source: each crate's files, linked,
    // beside the checksum Cargo.lock has for it.
    let mut vendored = std::collections::BTreeSet::new();
    for entry in crates {
        let mut parts = entry.splitn(3, '=');
        let (Some(key), Some(sha256), Some(directory)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(format!("{entry} is not KEY=SHA256=DIRECTORY"));
        };
        let into = vendor.join(key);
        create_dir(&into)?;
        let directory = absolute(directory)?;
        for file in read_dir(&directory)? {
            link(&file, &into.join(file.file_name().unwrap_or_default()))?;
        }
        write(
            &into.join(".cargo-checksum.json"),
            &format!("{{\"files\":{{}},\"package\":\"{sha256}\"}}"),
        )?;
        vendored.insert(key.to_owned());
    }

    // buck2 hands the action its inputs read-only, but cargo rewrites
    // Cargo.lock in place when the manifests have changed, so the copy is
    // writable. Then an empty file at every path cargo discovers a target at,
    // and at every target path a Cargo.toml names.
    copy_tree(paths.workspace, &workspace)?;
    for path in read(paths.placeholders)?
        .lines()
        .filter(|line| !line.is_empty())
    {
        write(&workspace.join(path), "")?;
    }
    let mut manifests = Vec::new();
    find_manifests(&workspace.join("crates"), &mut manifests)?;
    for manifest in manifests {
        let directory = manifest.parent().unwrap_or(Path::new("")).to_owned();
        for target in target_paths(&read(&manifest)?) {
            let path = directory.join(target);
            if !path.exists() {
                write(&path, "")?;
            }
        }
    }

    // cargo reads the vendored crates in place of crates.io, and never the
    // network: the action's output depends on its inputs alone.
    write(
        &cargo_home.join("config.toml"),
        &format!(
            "[source.crates-io]\nreplace-with = \"vendored\"\n\n[source.vendored]\ndirectory = \"{}\"\n\n[net]\noffline = true\n",
            vendor.display()
        ),
    )?;
    let stale = |why: &str, said: &str| -> Result<()> {
        // Without the scratch directory's name, so the same inputs say the same.
        let said = said.replace(&format!("{}/", paths.scratch.display()), "");
        write(&paths.out.join("stale"), &format!("{why}\n{said}"))
    };
    let lock = read(&workspace.join("Cargo.lock"))?;
    let missing: Vec<String> = be_lib_crate_pins::locked(&lock)
        .into_keys()
        .filter(|key| !vendored.contains(key))
        .collect();
    if !missing.is_empty() {
        return stale(
            &format!(
                "Cargo.lock names crates {} does not download: {}",
                be_lib_crate_pins::DOWNLOADS_PATH,
                missing.join(" ")
            ),
            "",
        );
    }
    let path = std::env::join_paths(std::iter::once(paths.cargo.join("bin")).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .map_err(|error| error.to_string())?;
    let toolchain = |arguments: &[&str]| {
        let mut command = Command::new(paths.cargo.join("bin/cargo"));
        command
            .args(arguments)
            .current_dir(&workspace)
            .env("CARGO_HOME", &cargo_home)
            .env("PATH", &path)
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER");
        command
    };
    let metadata = toolchain(&["metadata", "--format-version", "1", "--quiet", "--offline"])
        .output()
        .map_err(|error| format!("could not run cargo: {error}"))?;
    if !metadata.status.success() {
        return stale(
            &format!(
                "Cargo.lock cannot be brought up to date with the Cargo.toml files from the crates {} downloads; cargo said:",
                be_lib_crate_pins::DOWNLOADS_PATH
            ),
            &String::from_utf8_lossy(&metadata.stderr),
        );
    }
    // Cargo.lock as cargo left it, which ./scripts/verify writes back to the checkout.
    let files = paths.out.join("files");
    create_dir(&files)?;
    fs::copy(workspace.join("Cargo.lock"), files.join("Cargo.lock"))
        .map_err(|error| format!("could not copy Cargo.lock: {error}"))?;

    let tools = match tools {
        Tools::Built(executable) => executable.clone(),
        Tools::CargoBuild => {
            let target = paths.scratch.join("target");
            let status = toolchain(&[
                "build",
                "--quiet",
                "--release",
                "--locked",
                "-p",
                "be-tool-buck",
            ])
            .env("CARGO_TARGET_DIR", &target)
            .status()
            .map_err(|error| format!("could not run cargo: {error}"))?;
            if !status.success() {
                return Err("cargo could not build be-tool-buck; it says why above".to_owned());
            }
            target.join("release/buck-tools")
        }
    };
    let status = Command::new(&tools)
        .arg("plan")
        .arg(paths.out)
        .arg(&workspace)
        .arg(paths.nightly)
        .args(cross)
        .env("CARGO_HOME", &cargo_home)
        .status()
        .map_err(|error| format!("could not run {}: {error}", tools.display()))?;
    if !status.success() {
        return Err(format!("be-tool-buck's plan failed: {status}"));
    }
    Ok(())
}

/// The target paths a Cargo.toml names with `path = "....rs"`, a [lib]'s,
/// a [[bin]]'s or another target's.
fn target_paths(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .filter_map(|line| {
            let value = line
                .trim()
                .strip_prefix("path")?
                .trim_start()
                .strip_prefix('=')?
                .trim_start();
            let value = value.strip_prefix('"')?;
            let value = &value[..value.find('"')?];
            value.ends_with(".rs").then(|| value.to_owned())
        })
        .collect()
}

fn find_manifests(directory: &Path, into: &mut Vec<PathBuf>) -> Result<()> {
    for path in read_dir(directory)? {
        if path.is_dir() {
            find_manifests(&path, into)?;
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            into.push(path);
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    create_dir(to)?;
    for path in read_dir(from)? {
        let into = to.join(path.file_name().unwrap_or_default());
        if path.is_dir() {
            copy_tree(&path, &into)?;
        } else {
            fs::copy(&path, &into)
                .map_err(|error| format!("could not copy {}: {error}", path.display()))?;
            let mut permissions = fs::metadata(&into)
                .map_err(|error| format!("{}: {error}", into.display()))?
                .permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            fs::set_permissions(&into, permissions)
                .map_err(|error| format!("{}: {error}", into.display()))?;
        }
    }
    Ok(())
}

fn read_dir(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?
        .flatten()
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    Ok(paths)
}

#[cfg(unix)]
fn link(from: &Path, to: &Path) -> Result<()> {
    std::os::unix::fs::symlink(from, to)
        .map_err(|error| format!("could not link {}: {error}", to.display()))
}

#[cfg(not(unix))]
fn link(from: &Path, to: &Path) -> Result<()> {
    if from.is_dir() {
        copy_tree(from, to)
    } else {
        fs::copy(from, to)
            .map(|_| ())
            .map_err(|error| format!("could not copy {}: {error}", from.display()))
    }
}

fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|error| format!("could not read {}: {error}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    fs::write(path, text).map_err(|error| format!("could not write {}: {error}", path.display()))
}
