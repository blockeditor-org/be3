//! Writes OUT, every file under DIR in one file, for an executable that carries a staged
//! directory (be's host apps, buck/program/defs.bzl's packed_dir): for each file, in path
//! order, its path relative to DIR as a record and then its contents as one, each record
//! written as `bundle::record` writes it.

use std::path::Path;
use std::process::ExitCode;

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [out, dir] = arguments else {
        return Err("usage: pack OUT DIR".into());
    };
    let mut files = Vec::new();
    walk(Path::new(dir), Path::new(""), &mut files)?;
    files.sort();
    let mut packed = Vec::new();
    for relative in files {
        let name = relative
            .to_str()
            .ok_or_else(|| format!("{} is not UTF-8", relative.display()))?;
        super::bundle::write_record(&mut packed, name.as_bytes());
        let path = Path::new(dir).join(&relative);
        let path = path
            .to_str()
            .ok_or_else(|| format!("{} is not UTF-8", path.display()))?;
        super::bundle::record(&mut packed, path)?;
    }
    std::fs::write(out, packed).map_err(|error| format!("cannot write {out}: {error}"))?;
    Ok(ExitCode::SUCCESS)
}

fn walk(
    root: &Path,
    relative: &Path,
    files: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    let dir = root.join(relative);
    let entries =
        std::fs::read_dir(&dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let inner = relative.join(entry.file_name());
        // Staged files may be symlinks into the build's outputs: follow them.
        let meta = std::fs::metadata(entry.path()).map_err(|error| error.to_string())?;
        match meta.is_dir() {
            true => walk(root, &inner, files)?,
            false => files.push(inner),
        }
    }
    Ok(())
}
