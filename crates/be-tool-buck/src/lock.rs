//! `buck-tools lock CARGO`: what buckify's action cannot do without the
//! network. It brings Cargo.lock up to date with the Cargo.toml files, as the
//! cargo a developer runs would, and pins every crate the lockfile names in
//! buck/cargo/crates.bzl's downloads, the crates the action hands cargo
//! (buck/cargo/buckify.sh). //:buckify runs it, built by cargo, when the
//! action says it is out of date, then builds the action again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::generate::{DOWNLOADS, checksums, downloads, render_downloads};

const CRATES: &str = "buck/cargo/crates.bzl";

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [toolchain] = arguments else {
        return Err("usage: lock CARGO_SYSROOT, run in the checkout".to_owned());
    };
    let bin = Path::new(toolchain).join("bin");
    let mut path =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    path.insert(0, bin.clone());
    let path = std::env::join_paths(path).map_err(|error| error.to_string())?;
    let cargo = |arguments: &[&str]| {
        let mut command = Command::new(bin.join("cargo"));
        command
            .args(arguments)
            .env("PATH", &path)
            .stdout(Stdio::null());
        crate::status(&mut command)
    };
    cargo(&["metadata", "--format-version", "1", "--quiet"])?;

    let lock = crate::read("Cargo.lock")?;
    let checksums = checksums(&lock);
    let generated = crate::read(CRATES)?;
    let at = generated
        .find(&format!("\n{DOWNLOADS}"))
        .ok_or_else(|| format!("{CRATES} has no downloads"))?
        + 1;
    let mut sizes = pinned_sizes(&generated[at..]);
    let missing: Vec<&String> = checksums
        .keys()
        .filter(|key| !sizes.contains_key(*key))
        .collect();
    if !missing.is_empty() {
        // cargo fetch downloads every crate the lockfile names that cargo's
        // own cache lacks, so each new one's size is that of its archive.
        cargo(&["fetch", "--locked", "--quiet"])?;
        let caches = cargo_home()?.join("registry").join("cache");
        let caches: Vec<PathBuf> = std::fs::read_dir(&caches)
            .map_err(|error| format!("{}: {error}", caches.display()))?
            .flatten()
            .map(|entry| entry.path())
            .collect();
        for key in missing {
            let size = caches
                .iter()
                .find_map(|cache| std::fs::metadata(cache.join(format!("{key}.crate"))).ok())
                .ok_or_else(|| format!("cargo fetch did not download {key}"))?
                .len();
            println!("Pinned {key}, {size} bytes.");
            sizes.insert(key.clone(), size);
        }
    }
    let updated = format!(
        "{}{}",
        &generated[..at],
        render_downloads(downloads(&checksums, &sizes)?)
    );
    if updated != generated {
        let partial = format!("{CRATES}.partial");
        std::fs::write(&partial, updated)
            .and_then(|()| std::fs::rename(&partial, CRATES))
            .map_err(|error| format!("{CRATES}: {error}"))?;
        println!("Updated the downloads in {CRATES}.");
    }
    Ok(ExitCode::SUCCESS)
}

/// The size of each crate crates.bzl's downloads already pin, by key.
pub(crate) fn pinned_sizes(downloads: &str) -> BTreeMap<String, u64> {
    let mut sizes = BTreeMap::new();
    let mut key = None;
    for line in downloads.lines() {
        if let Some(name) = line
            .strip_prefix("    \"")
            .and_then(|rest| rest.strip_suffix("\": {"))
        {
            key = Some(name.to_owned());
        } else if let Some(size) = line
            .trim()
            .strip_prefix("\"size_bytes\": ")
            .and_then(|rest| rest.trim_end_matches(',').parse().ok())
            && let Some(key) = key.take()
        {
            sizes.insert(key, size);
        }
    }
    sizes
}

fn cargo_home() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        return Ok(home.into());
    }
    std::env::var_os("HOME")
        .map(|home| Path::new(&home).join(".cargo"))
        .ok_or_else(|| "neither CARGO_HOME nor HOME is set".to_owned())
}
