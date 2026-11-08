//! Lays out a program as it runs (buck/program/defs.bzl's program): the
//! executable under cargo's name, each --file beside it as it is, and each
//! app's manifest renamed <id>.app.json with the module it names and its
//! .cwasm, and --data, the app's read-only data (app_data), under data/<id>/.
//! Each --library is a library the modules are linked against
//! (guides/app_libraries.md), staged as lib-<its SHA-256>.wasm, which is the
//! name a module finds it by, with --library-artifact, its .cwasm. --index
//! writes apps.json, the list of manifests the app finds its apps through.
//! For the web bundle, --tree stages what wasm-bindgen wrote. --app-type,
//! after an app's MANIFEST=MODULE, stages that app's type module
//! (guides/type_modules.md) under the name its manifest's block_types give
//! it, with its SHA-256 as NAME.sha256 (the tag of its sessions) and, with
//! --type-artifact, its .cwasm. --compiled-only leaves out each module that
//! has a .cwasm, for the APK, which only falls back to one. With an
//! executable, program-files.txt lists every file staged, which is what
//! --install-login-session installs.
//!
//! The arguments are read in order: an --artifact, --library-artifact or
//! --type-artifact is the .cwasm of the module staged just before it, and a
//! --data or --app-type belongs to the app named just before it.
//!
//! The stage writes only what it makes itself into OUT (the .sha256 tags,
//! apps.json and program-files.txt) and, at LAYOUT, every path of the program
//! with what it is: one of those, or a file an argument gave (its index among
//! the arguments, which of the two files a MANIFEST=MODULE is, and its path
//! within a --tree or --data directory). The rule then lays the program out
//! from that, linking each file rather than copying it. It reads only the
//! manifests, libraries, type modules and directories, so an executable,
//! --file, module or .cwasm may be given by its file name alone.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use sha2::{Digest, Sha256};

const USAGE: &str = "usage: stage OUT LAYOUT [--executable=EXECUTABLE=NAME] [--file=FILE]... [--tree=DIR]... \
[--index] [--compiled-only] [--library=MODULE [--library-artifact=CWASM]]... \
(MANIFEST=MODULE [--data=DIR] [--artifact=CWASM] [--app-type=MODULE [--type-artifact=CWASM]])...";

/// What one path in the program is.
#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
    /// A file an argument gave: the `argument`th, the `part`th of its files
    /// (a MANIFEST=MODULE has two), at `within` inside it if it is a directory.
    Input {
        argument: usize,
        part: usize,
        within: String,
    },
    /// A file the stage writes itself.
    Write(Vec<u8>),
}

/// Every path in the program, relative to it, in byte order.
pub type Layout = BTreeMap<String, Entry>;

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [out, layout_path, arguments @ ..] = arguments else {
        return Err(USAGE.into());
    };
    let layout = layout(arguments)?;
    write(Path::new(out), &layout)?;
    std::fs::write(layout_path, describe(&layout))
        .map_err(|error| format!("{layout_path}: {error}"))?;
    Ok(ExitCode::SUCCESS)
}

/// The program the arguments describe.
pub fn layout(arguments: &[String]) -> Result<Layout, String> {
    let mut layout = Layout::new();
    let mut index = false;
    let mut executable = false;
    let mut compiled_only = false;
    let mut manifests = Vec::new();
    // The module the next .cwasm belongs to, and the app the next --data or
    // --app-type does: its manifest and id.
    let mut staged: Option<String> = None;
    let mut app: Option<(String, serde_json::Value, String)> = None;
    for (position, argument) in arguments.iter().enumerate() {
        let input = |part: usize| Entry::Input {
            argument: position,
            part,
            within: String::new(),
        };
        let (flag, value) = match argument.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, value),
            _ => (argument.as_str(), ""),
        };
        match flag {
            "--executable" => {
                let (_, name) = value
                    .split_once('=')
                    .ok_or_else(|| format!("{argument} is not --executable=EXECUTABLE=NAME"))?;
                layout.insert(name.to_owned(), input(0));
                executable = true;
            }
            "--file" => {
                layout.insert(file_name(value)?, input(0));
            }
            "--tree" => tree(Path::new(value), position, "", "", &mut layout)?,
            "--index" => index = true,
            "--compiled-only" => compiled_only = true,
            "--library" => {
                let name = format!("lib-{}.wasm", sha256(value)?);
                layout.insert(name.clone(), input(0));
                staged = Some(name);
            }
            "--library-artifact" | "--artifact" | "--type-artifact" => {
                let module = staged
                    .as_ref()
                    .ok_or_else(|| format!("{argument} follows no module"))?;
                let artifact = format!("{}.cwasm", module.strip_suffix(".wasm").unwrap_or(module));
                layout.insert(artifact, input(0));
                if compiled_only {
                    layout.remove(module);
                }
            }
            "--app-type" => {
                let (path, manifest, _) = app
                    .as_ref()
                    .ok_or_else(|| format!("{argument} follows no app"))?;
                let named = type_module(path, manifest)?;
                if file_name(value)? != named {
                    return Err(format!(
                        "{path} names {named} as its type module, but its type module is {value}"
                    ));
                }
                let tag = format!("{}.sha256", named.strip_suffix(".wasm").unwrap_or(&named));
                layout.insert(tag, Entry::Write(format!("{}\n", sha256(value)?).into_bytes()));
                layout.insert(named.clone(), input(0));
                staged = Some(named);
            }
            "--data" => {
                let (_, _, id) = app
                    .as_ref()
                    .ok_or_else(|| format!("{argument} follows no app"))?;
                let prefix = format!("data/{id}/");
                tree(Path::new(value), position, &prefix, "", &mut layout)?;
            }
            _ if flag.starts_with("--") => return Err(format!("unknown argument {argument}\n{USAGE}")),
            _ => {
                let (path, module) = argument
                    .split_once('=')
                    .ok_or_else(|| format!("{argument} is not MANIFEST=MODULE\n{USAGE}"))?;
                let manifest = crate::json(path)?;
                let id = string(path, &manifest, "id")?;
                let entry_point = string(path, &manifest, "entry_point")?;
                if file_name(module)? != entry_point {
                    return Err(format!(
                        "{path} names {entry_point} as its entry point, but its module is {module}"
                    ));
                }
                let name = format!("{id}.app.json");
                layout.insert(name.clone(), input(0));
                manifests.push(name);
                layout.insert(entry_point.clone(), input(1));
                staged = Some(entry_point);
                app = Some((path.to_owned(), manifest, id));
            }
        }
    }
    if index {
        layout.insert("apps.json".into(), Entry::Write(apps(&manifests).into_bytes()));
    }
    if executable {
        layout.remove(PROGRAM_FILES);
        let list: String = layout.keys().map(|path| format!("{path}\n")).collect();
        layout.insert(PROGRAM_FILES.into(), Entry::Write(list.into_bytes()));
    }
    Ok(layout)
}

const PROGRAM_FILES: &str = "program-files.txt";

/// apps.json: the manifests' names, one to a line, in the order they were given.
pub fn apps(manifests: &[String]) -> String {
    if manifests.is_empty() {
        return "[]\n".into();
    }
    let names: Vec<String> = manifests.iter().map(|name| format!("  \"{name}\"")).collect();
    format!("[\n{}\n]\n", names.join(",\n"))
}

/// Writes the files the stage makes at `out`.
pub fn write(out: &Path, layout: &Layout) -> Result<(), String> {
    let failed = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    std::fs::create_dir_all(out).map_err(|error| failed(out, error))?;
    for (path, entry) in layout {
        if let Entry::Write(contents) = entry {
            let to = out.join(path);
            std::fs::write(&to, contents).map_err(|error| failed(&to, error))?;
        }
    }
    Ok(())
}

/// The layout as the rule reads it: each path with `argument`, `part` and
/// `within` for a file an argument gave, or nothing more for one in OUT.
pub fn describe(layout: &Layout) -> String {
    let entries: Vec<serde_json::Value> = layout
        .iter()
        .map(|(path, entry)| match entry {
            Entry::Input {
                argument,
                part,
                within,
            } => serde_json::json!({
                "path": path,
                "argument": argument,
                "part": part,
                "within": within,
            }),
            Entry::Write(_) => serde_json::json!({ "path": path }),
        })
        .collect();
    serde_json::Value::Array(entries).to_string()
}

/// Every file under `dir`, the `argument`th argument, under `prefix` in the
/// program; `within` is where `dir` is inside the argument's directory.
fn tree(
    dir: &Path,
    argument: usize,
    prefix: &str,
    within: &str,
    layout: &mut Layout,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|error| format!("{}: {error}", dir.display()))?.path();
        let name = format!("{within}{}", file_name(&path.to_string_lossy())?);
        if path.is_dir() {
            tree(&path, argument, prefix, &format!("{name}/"), layout)?;
        } else {
            layout.insert(
                format!("{prefix}{name}"),
                Entry::Input {
                    argument,
                    part: 0,
                    within: name,
                },
            );
        }
    }
    Ok(())
}

fn file_name(path: &str) -> Result<String, String> {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{path} has no file name"))
}

fn string(path: &str, manifest: &serde_json::Value, key: &str) -> Result<String, String> {
    manifest[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{path} has no \"{key}\""))
}

/// The module every block type of the manifest names.
fn type_module(path: &str, manifest: &serde_json::Value) -> Result<String, String> {
    let mut named = None;
    for block_type in manifest["block_types"].as_object().into_iter().flat_map(|types| types.values()) {
        if let Some(module) = block_type["module"].as_str() {
            if named.is_some_and(|named| named != module) {
                return Err(format!("{path}'s block types name more than one module"));
            }
            named = Some(module);
        }
    }
    named
        .map(str::to_owned)
        .ok_or_else(|| format!("{path} names no type module"))
}

fn sha256(path: &str) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|error| format!("{path}: {error}"))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|error| format!("{path}: {error}"))?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
