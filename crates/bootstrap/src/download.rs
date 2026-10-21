use std::io::Read as _;
use std::path::Path;
use std::process::Command;

use sha2::{Digest as _, Sha256};

use crate::Result;

pub const MIRROR: &str = "https://lfs.pfg.pw/by-name/be3";

pub fn curl_error(error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        "curl was not found on PATH. Install curl.".to_owned()
    } else {
        format!("could not run curl: {error}")
    }
}

pub fn fetch_https(destination: &Path, source: &str) -> Result<bool> {
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
        ])
        .args([
            "--connect-timeout",
            "30",
            "--speed-limit",
            "1024",
            "--speed-time",
            "60",
        ])
        .args(["--retry", "4", "--retry-connrefused", "--output"])
        .arg(destination)
        .arg(source)
        .status()
        .map_err(curl_error)?;
    Ok(status.success())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn download_verified(
    url: &str,
    destination: &Path,
    expected: &str,
    mirrors: &[String],
) -> Result<()> {
    let sources: Vec<&str> = std::iter::once(url)
        .chain(mirrors.iter().map(String::as_str))
        .collect();
    for source in &sources {
        let _ = std::fs::remove_file(destination);
        if fetch_https(destination, source)? {
            let actual = sha256_file(destination)?;
            if actual == expected {
                return Ok(());
            }
            let _ = std::fs::remove_file(destination);
            return Err(format!(
                "\nThe download from {source} is not the file this repository expects.\n  sha256 is   {actual}\n  sha256 want {expected}\n\n\
                 Nothing has been installed. If the release was legitimately replaced, check the\n\
                 new bytes against upstream's own provenance before recording the new hash here;\n\
                 recomputing it from the same download only proves the download agrees with\n\
                 itself."
            ));
        }
        eprintln!("The download from {source} did not complete.");
    }
    let _ = std::fs::remove_file(destination);
    let mut message = format!(
        "\nNothing could be downloaded for {}. These were tried:",
        destination.display()
    );
    for source in sources {
        message.push_str(&format!("\n  {source}"));
    }
    Err(message)
}

#[cfg(unix)]
pub fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("could not make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
pub fn make_executable(_: &Path) -> Result<()> {
    Ok(())
}

pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
