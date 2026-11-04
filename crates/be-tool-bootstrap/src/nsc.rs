use std::process::{Command, ExitCode};

use crate::download::{download_verified, make_executable};
use crate::pins::Pins;
use crate::{Result, repository};

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    if arguments.len() > 1 {
        return Err("Usage: install-nsc [linux_amd64 | linux_arm64]".to_owned());
    }
    let platform = arguments
        .into_iter()
        .next()
        .unwrap_or_else(|| match std::env::consts::ARCH {
            "x86_64" => "linux_amd64".to_owned(),
            "aarch64" => "linux_arm64".to_owned(),
            other => format!("linux_{other}"),
        });
    let pins = Pins::read()?;
    let version = pins.get("nsc_version")?;
    let sha256 = pins.get(&format!("nsc_sha256_{platform}"))?;
    let destination = repository().join(format!("target/tools/nsc-{version}-{platform}/nsc"));
    if destination.is_file() {
        println!("{}", destination.display());
        return Ok(ExitCode::SUCCESS);
    }
    let release = format!("nsc_{version}_{platform}.tar.gz");
    let url = format!("https://get.namespace.so/packages/nsc/v{version}/{release}");
    let directory = destination.parent().unwrap_or(repository()).to_path_buf();
    let archive = directory.join(&release);
    eprintln!("Downloading nsc {version} from {url}...");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    download_verified(&url, &archive, sha256, &[])?;
    let unpacked = directory.join("unpacked");
    let _ = std::fs::remove_dir_all(&unpacked);
    std::fs::create_dir_all(&unpacked)
        .map_err(|error| format!("could not create {}: {error}", unpacked.display()))?;
    let extracted = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&unpacked)
        .args(["--no-same-owner", "nsc"])
        .status()
        .is_ok_and(|status| status.success());
    if !extracted {
        return Err(format!("could not unpack {}", archive.display()));
    }
    make_executable(&unpacked.join("nsc"))?;
    std::fs::rename(unpacked.join("nsc"), &destination)
        .map_err(|error| format!("could not write {}: {error}", destination.display()))?;
    let _ = std::fs::remove_dir_all(&unpacked);
    let _ = std::fs::remove_file(&archive);
    println!("{}", destination.display());
    Ok(ExitCode::SUCCESS)
}
