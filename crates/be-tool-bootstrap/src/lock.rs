//! The lock step: Cargo.lock brought up to date with the Cargo.toml files, as
//! the cargo a developer runs would, and every crate it names pinned in
//! buck/cargo/crates.bzl's downloads, which buck/cargo/buckify.bxl hands
//! cargo on a worker that has no network. ./scripts/verify and ./scripts/ci run
//! it before their buck2 command when [`STAMP`] is not the hash of what the
//! step reads ([`inputs`]), and `//:buckify` before it generates; under
//! `--check` it fails instead. It needs the network, and cargo (cargo.rs).

use std::path::Path;
use std::process::{Command, ExitCode};

use sha2::{Digest as _, Sha256};

use crate::download::download_verified;
use crate::pins::Pins;
use crate::platform::triple;
use crate::{Result, cargo, repository, write_if_changed};

/// What the step last left in step: the hash of [`inputs`] it wrote.
pub const STAMP: &str = "buck/cargo/lock.sha256";

const CRATES: &str = "buck/cargo/crates.bzl";

/// The environment variable under which the bootstrap is cargo's rustc. cargo
/// asks rustc only for its version and host to resolve a lockfile (rust-version
/// aware resolution), so the step downloads no compiler.
pub const RUSTC_RELEASE: &str = "BE3_LOCK_RUSTC_RELEASE";

const USAGE: &str = "Usage: bootstrap lock [--check]";

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let check = match arguments.as_slice() {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err(USAGE.to_owned()),
    };
    ensure(check)?;
    Ok(ExitCode::SUCCESS)
}

pub fn ensure(check: bool) -> Result<()> {
    let pins = Pins::read()?;
    let version = cargo::version(&pins)?.to_owned();
    let repository = repository();
    let recorded = std::fs::read_to_string(repository.join(STAMP)).unwrap_or_default();
    if recorded_hash(&recorded) == Some(inputs(repository, &version)?.as_str()) {
        return Ok(());
    }
    if check {
        return Err(format!(
            "Out of date: Cargo.lock or the downloads in {CRATES}, with the Cargo.toml files ({STAMP} \
             is not the hash of them). Run ./scripts/verify without --check, which brings them up to date."
        ));
    }
    eprintln!(
        "Cargo.lock and the downloads in {CRATES} may be out of date with the Cargo.toml files ({STAMP}): \
         bringing them up to date with cargo {version}."
    );
    let cargo = cargo::ensure(&pins)?;
    update(repository, &cargo, &version)?;
    pin(repository)?;
    let stamp = format!(
        "# The SHA-256 of the Cargo.toml files, Cargo.lock, crates.bzl's downloads and\n\
         # the cargo that last brought them into step (crates/be-tool-bootstrap/src/lock.rs).\n\
         # ./scripts/verify brings them up to date again when it differs.\n{}\n",
        inputs(repository, &version)?
    );
    write_if_changed(&repository.join(STAMP), stamp.as_bytes())?;
    Ok(())
}

fn recorded_hash(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
}

/// The hash of everything the step reads: the root Cargo.toml and each
/// crates/*/Cargo.toml (every workspace member is one), Cargo.lock, crates.bzl's
/// downloads and cargo's release, with paths written with / and lines ended
/// with \n, so a checkout on Windows hashes the same.
pub fn inputs(repository: &Path, version: &str) -> Result<String> {
    let mut files = vec!["Cargo.toml".to_owned(), "Cargo.lock".to_owned()];
    let crates = repository.join("crates");
    let mut members: Vec<String> = std::fs::read_dir(&crates)
        .map_err(|error| format!("could not read {}: {error}", crates.display()))?
        .flatten()
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .map(|name| format!("crates/{name}/Cargo.toml"))
        .collect();
    members.sort();
    files.extend(members);
    let mut hasher = Sha256::new();
    hasher.update(format!("cargo {version}\n"));
    let mut add = |name: &str, text: &str| {
        let text = text.replace("\r\n", "\n");
        hasher.update(format!("{name}\n{}\n", text.len()));
        hasher.update(text);
    };
    for file in &files {
        add(file, &read(repository, file)?);
    }
    let generated = read(repository, CRATES)?;
    let (_, downloads) =
        be_lib_crate_pins::split(&generated).ok_or_else(|| format!("{CRATES} has no downloads"))?;
    add("downloads", downloads);
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn read(repository: &Path, file: &str) -> Result<String> {
    std::fs::read_to_string(repository.join(file))
        .map_err(|error| format!("could not read {file}: {error}"))
}

/// `cargo update --workspace`: what any cargo command does to a lockfile out of
/// date with the manifests, changing no crate it does not have to. cargo's home
/// is the step's own, so a developer's cargo configuration plays no part.
fn update(repository: &Path, cargo: &Path, version: &str) -> Result<()> {
    let rustc = std::env::current_exe()
        .map_err(|error| format!("could not find the bootstrap's own path: {error}"))?;
    let tools = repository.join("target/tools");
    let status = Command::new(cargo)
        .args(["update", "--workspace"])
        .current_dir(repository)
        .env("CARGO_HOME", tools.join("cargo-home"))
        .env("CARGO_TARGET_DIR", tools.join("cargo-target"))
        .env("RUSTC", rustc)
        .env(RUSTC_RELEASE, version)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .status()
        .map_err(|error| format!("could not run {}: {error}", cargo.display()))?;
    if !status.success() {
        return Err("cargo could not bring Cargo.lock up to date with the Cargo.toml files; it says why above.".to_owned());
    }
    Ok(())
}

/// Rewrites crates.bzl's downloads from Cargo.lock: a crate already pinned keeps
/// its size, and a new one is downloaded from crates.io, checked against the
/// checksum Cargo.lock has for it, and pinned with its size.
fn pin(repository: &Path) -> Result<()> {
    let locked = be_lib_crate_pins::locked(&read(repository, "Cargo.lock")?);
    let generated = read(repository, CRATES)?;
    let (rest, downloads) =
        be_lib_crate_pins::split(&generated).ok_or_else(|| format!("{CRATES} has no downloads"))?;
    let mut sizes = be_lib_crate_pins::pinned_sizes(downloads);
    let directory = repository.join("target/tools/crates");
    for (key, crate_) in &locked {
        if sizes.contains_key(key) {
            continue;
        }
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        let archive = directory.join(format!("{key}.crate"));
        download_verified(&crate_.url(), &archive, &crate_.sha256, &[])?;
        let size = std::fs::metadata(&archive)
            .map_err(|error| format!("could not read {}: {error}", archive.display()))?
            .len();
        let _ = std::fs::remove_file(&archive);
        eprintln!("Pinned {key}, {size} bytes.");
        sizes.insert(key.clone(), size);
    }
    let updated = format!("{rest}{}", be_lib_crate_pins::render(&locked, &sizes)?);
    if write_if_changed(&repository.join(CRATES), updated.as_bytes())? {
        eprintln!("Updated the downloads in {CRATES}.");
    }
    Ok(())
}

/// The bootstrap as cargo's rustc (`RUSTC_RELEASE`): `rustc -vV` answered
/// with the pinned release and this machine's triple, and nothing else.
pub fn rustc(release: &str, arguments: &[String]) -> ExitCode {
    if arguments != ["-vV"] {
        eprintln!(
            "The lock step's cargo asked rustc for more than its version ({}), which it cannot answer.",
            arguments.join(" ")
        );
        return ExitCode::FAILURE;
    }
    println!(
        "rustc {release}\nbinary: rustc\nhost: {}\nrelease: {release}",
        triple()
    );
    ExitCode::SUCCESS
}
