//! Writes OUT, every app's manifest and its type module in one file, for an
//! executable that carries them (be, buck/program/defs.bzl's type_bundle): for
//! each app, its manifest's length as 8 hex digits and the manifest, then its
//! type module's the same way (length 0 for an app with no module of its own).

use std::io::Write;
use std::process::ExitCode;

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [out, apps @ ..] = arguments else {
        return Err("usage: bundle OUT (MANIFEST=MODULE | MANIFEST=)...".into());
    };
    let mut bundle = Vec::new();
    for app in apps {
        let (manifest, module) = app
            .split_once('=')
            .ok_or_else(|| format!("{app} is not MANIFEST=MODULE"))?;
        record(&mut bundle, manifest)?;
        record(&mut bundle, module)?;
    }
    std::fs::write(out, bundle).map_err(|error| format!("cannot write {out}: {error}"))?;
    Ok(ExitCode::SUCCESS)
}

/// A file's length as 8 hex digits and the file, or 0 for no file.
pub fn record(bundle: &mut Vec<u8>, path: &str) -> Result<(), String> {
    if path.is_empty() {
        bundle.extend_from_slice(b"00000000");
        return Ok(());
    }
    let contents = std::fs::read(path).map_err(|error| format!("cannot read {path}: {error}"))?;
    write!(bundle, "{:08x}", contents.len()).map_err(|error| error.to_string())?;
    bundle.extend_from_slice(&contents);
    Ok(())
}
