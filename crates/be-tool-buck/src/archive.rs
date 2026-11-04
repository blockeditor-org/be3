use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

const USAGE: &str = "usage: archive OUT.zip PREFIX (DIR | FILE)";

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let [out, prefix, input] = arguments else {
        return Err(USAGE.into());
    };
    let input = Path::new(input);
    let file = std::fs::File::create(out).map_err(|error| format!("{out}: {error}"))?;
    let mut archive = ZipWriter::new(file);
    if input.is_dir() {
        let mut found = Vec::new();
        files(input, &mut found)?;
        found.sort();
        for path in found {
            let relative = path
                .strip_prefix(input)
                .map_err(|error| error.to_string())?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            entry(&mut archive, &format!("{prefix}/{relative}"), &path)?;
        }
    } else {
        entry(&mut archive, prefix, input)?;
    }
    archive.finish().map_err(|error| error.to_string())?;
    Ok(ExitCode::SUCCESS)
}

fn entry<W: Write + std::io::Seek>(
    archive: &mut ZipWriter<W>,
    name: &str,
    path: &Path,
) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default())
        .large_file(data.len() > u32::MAX as usize)
        .unix_permissions(if executable(path)? { 0o755 } else { 0o644 });
    archive
        .start_file(name, options)
        .map_err(|error| error.to_string())?;
    archive.write_all(&data).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn executable(path: &Path) -> Result<bool, String> {
    use std::os::unix::fs::PermissionsExt;
    let metadata =
        std::fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(_path: &Path) -> Result<bool, String> {
    Ok(false)
}

fn files(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    for item in
        std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
    {
        let path = item.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            files(&path, found)?;
        } else {
            found.push(path);
        }
    }
    Ok(())
}
