//! The autofixes as an action, which a package's :fix runs (buck/dev/verify.bzl):
//! a copy of SOURCES, fixed, compared with SOURCES. OUT/changed holds every file
//! the fixes changed or added, at its path in the repository, and OUT/deleted
//! names every file they removed, one a line; ./scripts/verify copies the one
//! into the checkout and deletes the other. OUT/original holds each of those
//! files as the fixes read it, and ./scripts/verify touches only the files that
//! still read the same: an output left behind by an earlier run, when this one
//! could not run, is not this checkout's fix.
//!
//! With --fix-rust-source and --rustfmt, the Rust files are fixed by
//! fix-rust-source, then formatted by the sysroot's rustfmt; with
//! --starlark-fmt and --starlark-config, the BUCK, PACKAGE, .bzl and .bxl files
//! are formatted by starlark_fmt.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: fix OUT SOURCES [--fix-rust-source BIN --rustfmt SYSROOT] [--starlark-fmt BIN --starlark-config JSON]";

#[derive(Default)]
struct Fixers {
    fix_rust_source: Option<PathBuf>,
    rustfmt: Option<PathBuf>,
    starlark_fmt: Option<PathBuf>,
    starlark_config: Option<PathBuf>,
}

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [out, sources, flags @ ..] = arguments else {
        return Err(USAGE.into());
    };
    let mut fixers = Fixers::default();
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        let value = flags.next().map(absolute).ok_or(USAGE)?;
        let slot = match flag.as_str() {
            "--fix-rust-source" => &mut fixers.fix_rust_source,
            "--rustfmt" => &mut fixers.rustfmt,
            "--starlark-fmt" => &mut fixers.starlark_fmt,
            "--starlark-config" => &mut fixers.starlark_config,
            _ => return Err(format!("unknown argument {flag}\n{USAGE}")),
        };
        *slot = Some(value);
    }
    let tree = scratch_directory()?;
    let result = fix(&absolute(out), &absolute(sources), &tree, &fixers);
    let _ = std::fs::remove_dir_all(&tree);
    result.map(|()| ExitCode::SUCCESS)
}

fn fix(out: &Path, sources: &Path, tree: &Path, fixers: &Fixers) -> Result<(), String> {
    for directory in ["changed", "original"] {
        create_dir(&out.join(directory))?;
    }
    let before = files(sources)?;
    for path in &before {
        copy(&sources.join(path), &tree.join(path))?;
    }
    if let (Some(fix_rust_source), Some(rustfmt)) = (&fixers.fix_rust_source, &fixers.rustfmt) {
        run_tool(Command::new(fix_rust_source).current_dir(tree))?;
        let rust: Vec<_> = files(tree)?
            .into_iter()
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .collect();
        for chunk in rust.chunks(256) {
            run_tool(
                Command::new(rustfmt.join("bin/rustfmt"))
                    .current_dir(tree)
                    .args(["--edition", "2024"])
                    .args(chunk),
            )?;
        }
    }
    if let (Some(starlark_fmt), Some(config)) = (&fixers.starlark_fmt, &fixers.starlark_config) {
        let starlark: Vec<_> = files(tree)?
            .into_iter()
            .filter(|path| is_starlark(path))
            .collect();
        for chunk in starlark.chunks(256) {
            run_tool(
                Command::new(starlark_fmt)
                    .current_dir(tree)
                    .arg("--config")
                    .arg(config)
                    .arg("fmt")
                    .args(chunk),
            )?;
        }
    }
    let after = files(tree)?;
    let mut deleted = String::new();
    for path in before.difference(&after) {
        deleted.push_str(&path.to_string_lossy());
        deleted.push('\n');
        copy(&sources.join(path), &out.join("original").join(path))?;
    }
    write(&out.join("deleted"), deleted.as_bytes())?;
    for path in &after {
        let fixed = read(&tree.join(path))?;
        let original = before
            .contains(path)
            .then(|| read(&sources.join(path)))
            .transpose()?;
        if original.as_ref() == Some(&fixed) {
            continue;
        }
        write(&out.join("changed").join(path), &fixed)?;
        if let Some(original) = original {
            write(&out.join("original").join(path), &original)?;
        }
    }
    Ok(())
}

/// A BUCK, PACKAGE, .bzl or .bxl file, which starlark_fmt formats.
pub fn is_starlark(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    matches!(name.as_ref(), "BUCK" | "BUCK.v2" | "PACKAGE")
        || name.ends_with(".bzl")
        || name.ends_with(".bxl")
}

/// Every file under root, as a path relative to it, following symbolic links,
/// as the sources an action is given are.
pub fn files(root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let mut found = BTreeSet::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let directory = root.join(&relative);
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = relative.join(entry.file_name());
            let metadata = std::fs::metadata(entry.path())
                .map_err(|error| format!("cannot read {}: {error}", entry.path().display()))?;
            if metadata.is_dir() {
                pending.push(path);
            } else {
                found.insert(path);
            }
        }
    }
    Ok(found)
}

fn absolute(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        return path.to_owned();
    }
    std::env::current_dir()
        .map(|here| here.join(path))
        .unwrap_or_else(|_| path.to_owned())
}

fn scratch_directory() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("fix.{}.{nanos}", std::process::id()));
    create_dir(&path)?;
    Ok(path)
}

fn run_tool(command: &mut Command) -> Result<(), String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .status()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed: {status}"))
    }
}

fn create_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn write(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    std::fs::write(path, contents)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

// A copy that can be written, whatever the source's mode: the fixers rewrite
// files in place.
fn copy(from: &Path, to: &Path) -> Result<(), String> {
    write(to, &read(from)?)
}
