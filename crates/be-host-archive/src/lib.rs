use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"be3-app\x01";
const LEVEL: i32 = 3;
const WINDOW_LOG: u32 = 27;

pub fn pack(dir: &Path, archive: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    walk(dir, "", &mut files)?;
    files.sort();
    let out = File::create(archive).map_err(|error| failed("create", archive, error))?;
    let mut out = BufWriter::new(out);
    out.write_all(MAGIC)
        .map_err(|error| failed("write", archive, error))?;
    let mut encoder =
        zstd::Encoder::new(out, LEVEL).map_err(|error| failed("compress", archive, error))?;
    encoder
        .long_distance_matching(true)
        .and_then(|()| encoder.window_log(WINDOW_LOG))
        .map_err(|error| failed("compress", archive, error))?;
    for name in files {
        let path = dir.join(&name);
        let metadata = fs::metadata(&path).map_err(|error| failed("read", &path, error))?;
        encoder
            .write_all(&(name.len() as u32).to_le_bytes())
            .and_then(|()| encoder.write_all(name.as_bytes()))
            .and_then(|()| encoder.write_all(&[u8::from(executable(&metadata))]))
            .and_then(|()| encoder.write_all(&metadata.len().to_le_bytes()))
            .map_err(|error| failed("write", archive, error))?;
        let mut file = File::open(&path).map_err(|error| failed("read", &path, error))?;
        let copied =
            std::io::copy(&mut file, &mut encoder).map_err(|error| failed("pack", &path, error))?;
        if copied != metadata.len() {
            return Err(format!("{} changed while it was packed", path.display()));
        }
    }
    encoder
        .write_all(&0u32.to_le_bytes())
        .map_err(|error| failed("write", archive, error))?;
    encoder
        .finish()
        .and_then(|mut out| out.flush())
        .map_err(|error| failed("write", archive, error))
}

fn walk(dir: &Path, prefix: &str, files: &mut Vec<String>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| failed("list", dir, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| failed("list", dir, error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|name| format!("{} is not UTF-8", name.to_string_lossy()))?;
        let name = format!("{prefix}{name}");
        let path = entry.path();
        if fs::metadata(&path)
            .map_err(|error| failed("read", &path, error))?
            .is_dir()
        {
            walk(&path, &format!("{name}/"), files)?;
        } else {
            files.push(name);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &fs::Metadata) -> bool {
    false
}

pub fn unpack(archive: &Path, dir: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| failed("open", archive, error))?;
    let mut input = BufReader::new(file);
    let mut magic = [0; 8];
    input
        .read_exact(&mut magic)
        .map_err(|error| failed("read", archive, error))?;
    if &magic != MAGIC {
        return Err(format!("{} is not an app archive", archive.display()));
    }
    let mut decoder =
        zstd::Decoder::with_buffer(input).map_err(|error| failed("decompress", archive, error))?;
    decoder
        .window_log_max(WINDOW_LOG)
        .map_err(|error| failed("decompress", archive, error))?;
    let read = |decoder: &mut zstd::Decoder<'_, BufReader<File>>, bytes: &mut [u8]| {
        decoder
            .read_exact(bytes)
            .map_err(|error| failed("decompress", archive, error))
    };
    loop {
        let mut length = [0; 4];
        read(&mut decoder, &mut length)?;
        let length = u32::from_le_bytes(length) as usize;
        if length == 0 {
            return Ok(());
        }
        let mut name = vec![0; length];
        read(&mut decoder, &mut name)?;
        let name = String::from_utf8(name)
            .map_err(|_| format!("{} names a file that is not UTF-8", archive.display()))?;
        if name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(format!("{} names the file {name}", archive.display()));
        }
        let mut flags = [0; 1];
        read(&mut decoder, &mut flags)?;
        let mut size = [0; 8];
        read(&mut decoder, &mut size)?;
        let size = u64::from_le_bytes(size);
        let path = dir.join(&name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| failed("create", parent, error))?;
        }
        let mut out = File::create(&path).map_err(|error| failed("create", &path, error))?;
        let copied = std::io::copy(&mut (&mut decoder).take(size), &mut out)
            .map_err(|error| failed("unpack", &path, error))?;
        if copied != size {
            return Err(format!(
                "{} ends in the middle of {name}",
                archive.display()
            ));
        }
        if flags[0] == 1 {
            make_executable(&path)?;
        }
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|error| failed("change", path, error))
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<(), String> {
    Ok(())
}

pub fn unpacked(archive: &Path) -> Result<PathBuf, String> {
    let archive = archive
        .canonicalize()
        .map_err(|error| failed("find", archive, error))?;
    let metadata = fs::metadata(&archive).map_err(|error| failed("read", &archive, error))?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |time| time.as_nanos());
    let stamp = format!("{} {modified}\n", metadata.len());
    let root = archive
        .ancestors()
        .find(|ancestor| ancestor.file_name().is_some_and(|name| name == "buck-out"))
        .and_then(Path::parent)
        .map_or_else(
            || archive.with_extension("unpacked"),
            |checkout| checkout.join("target").join("apps"),
        );
    let key: String = Sha256::digest(archive.to_string_lossy().as_bytes())[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let stem = archive.file_stem().map_or_else(
        || "app".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let dir = root.join(format!("{stem}-{key}"));
    let stamped = root.join(format!("{stem}-{key}.stamp"));
    if dir.is_dir() && fs::read_to_string(&stamped).is_ok_and(|held| held == stamp) {
        return Ok(dir);
    }
    let fresh = root.join(format!("{stem}-{key}.{}", std::process::id()));
    let _ = fs::remove_dir_all(&fresh);
    fs::create_dir_all(&fresh).map_err(|error| failed("create", &fresh, error))?;
    if let Err(error) = unpack(&archive, &fresh) {
        let _ = fs::remove_dir_all(&fresh);
        return Err(error);
    }
    let _ = fs::remove_file(&stamped);
    let _ = fs::remove_dir_all(&dir);
    fs::rename(&fresh, &dir).map_err(|error| failed("move", &fresh, error))?;
    fs::write(&stamped, stamp).map_err(|error| failed("write", &stamped, error))?;
    Ok(dir)
}

pub fn run(archive: &Path, program: &str, arguments: &[String]) -> Result<(), String> {
    let program = unpacked(archive)?.join(program);
    let mut command = std::process::Command::new(&program);
    command.args(arguments);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(failed("run", &program, command.exec()))
    }
    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .map_err(|error| failed("run", &program, error))?;
        std::process::exit(status.code().unwrap_or(1))
    }
}

fn failed(what: &str, path: &Path, error: std::io::Error) -> String {
    format!("could not {what} {}: {error}", path.display())
}

#[cfg(test)]
mod tests;
