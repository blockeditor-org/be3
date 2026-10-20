use std::path::PathBuf;
use std::process::{Command, ExitCode};

use crate::download::{MIRROR, download_verified, is_executable, make_executable};
use crate::pins::Pins;
use crate::platform::{EXE, triple};
use crate::{Result, partial_path, repository};

pub fn path(pins: &Pins) -> Result<PathBuf> {
    let version = pins.get("buck2_version")?;
    Ok(repository().join(format!("target/tools/buck2-{version}/buck2{EXE}")))
}

pub fn install_command(arguments: Vec<String>) -> Result<ExitCode> {
    if !arguments.is_empty() {
        return Err("Usage: install-buck2".to_owned());
    }
    let pins = Pins::read()?;
    let destination = path(&pins)?;
    if is_executable(&destination) {
        println!(
            "buck2 {} is already installed at {}",
            pins.get("buck2_version")?,
            destination.display()
        );
        return Ok(ExitCode::SUCCESS);
    }
    install(&pins)?;
    Ok(ExitCode::SUCCESS)
}

pub fn ensure(pins: &Pins) -> Result<PathBuf> {
    let destination = path(pins)?;
    if !is_executable(&destination) {
        install(pins)?;
    }
    Ok(destination)
}

fn install(pins: &Pins) -> Result<()> {
    let version = pins.get("buck2_version")?;
    let triple = triple();
    let sha256 = pins.sha256("buck2", triple)?;
    let destination = path(pins)?;
    let release = format!("buck2-{triple}{EXE}.zst");
    let url = format!("https://github.com/facebook/buck2/releases/download/{version}/{release}");
    let mirror = format!("{MIRROR}/{version}/{release}");
    let directory = destination.parent().unwrap_or(repository()).to_path_buf();
    let archive = directory.join(&release);

    eprintln!("Downloading buck2 {version} from {url}...");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    download_verified(&url, &archive, sha256, &[mirror])?;
    let compressed = std::fs::read(&archive)
        .map_err(|error| format!("could not read {}: {error}", archive.display()))?;
    let binary = zstd::decode_all(compressed.as_slice())
        .map_err(|error| format!("could not decompress {release}: {error}"))?;
    let partial = partial_path(&destination);
    std::fs::write(&partial, binary)
        .map_err(|error| format!("could not write {}: {error}", partial.display()))?;
    make_executable(&partial)?;
    std::fs::rename(&partial, &destination)
        .map_err(|error| format!("could not write {}: {error}", destination.display()))?;
    let _ = std::fs::remove_file(&archive);

    let installed = Command::new(&destination)
        .arg("--version")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default();
    eprintln!("Installed {installed} at {}", destination.display());
    Ok(())
}
