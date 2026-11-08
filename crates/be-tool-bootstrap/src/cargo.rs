//! The cargo the lock step runs (lock.rs): rust-toolchain.toml's release, Rust's
//! own build for this machine, which scripts/internal/pins pins by its hash and
//! size, unpacked into target/tools/cargo-<version>/.

use std::path::PathBuf;

use crate::download::{download_verified, is_executable, make_executable};
use crate::pins::Pins;
use crate::platform::{EXE, triple};
use crate::{Result, partial_path, repository};

/// The cargo release pinned, once rust-toolchain.toml is seen to ask for it.
pub fn version(pins: &Pins) -> Result<&str> {
    let version = pins.get("cargo_version")?;
    let toolchain = repository().join("rust-toolchain.toml");
    let text = std::fs::read_to_string(&toolchain)
        .map_err(|error| format!("could not read {}: {error}", toolchain.display()))?;
    let channel = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("channel"))
        .filter_map(|rest| rest.trim().strip_prefix('='))
        .map(|value| value.trim().trim_matches('"'))
        .next()
        .unwrap_or_default();
    if channel != version {
        return Err(format!(
            "rust-toolchain.toml's channel is {channel:?}, but scripts/internal/pins pins cargo {version}: \
             pin the cargo of {channel:?} there, each platform's hash as \
             https://static.rust-lang.org/dist/channel-rust-{channel}.toml lists it."
        ));
    }
    Ok(version)
}

pub fn ensure(pins: &Pins) -> Result<PathBuf> {
    let version = version(pins)?;
    let destination = repository().join(format!("target/tools/cargo-{version}/cargo{EXE}"));
    if is_executable(&destination) {
        return Ok(destination);
    }
    let triple = triple();
    let sha256 = pins.sha256("cargo", triple)?;
    let size_name = format!("cargo_size_{}", triple.replace('-', "_"));
    let size: u64 = pins
        .get(&size_name)?
        .parse()
        .map_err(|_| format!("scripts/internal/pins' {size_name} is not a number of bytes"))?;
    let release = format!("cargo-{version}-{triple}");
    let url = format!("https://static.rust-lang.org/dist/{release}.tar.gz");
    let directory = destination.parent().unwrap_or(repository()).to_path_buf();
    let archive = directory.join(format!("{release}.tar.gz"));

    eprintln!("Downloading cargo {version} from {url}...");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    download_verified(&url, &archive, sha256, &[])?;
    let file = std::fs::File::open(&archive)
        .map_err(|error| format!("could not read {}: {error}", archive.display()))?;
    let length = file
        .metadata()
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    if length != size {
        let _ = std::fs::remove_file(&archive);
        return Err(format!(
            "{url} is {length} bytes, but scripts/internal/pins says {size}."
        ));
    }
    let inside = format!("{release}/cargo/bin/cargo{EXE}");
    let binary = crate::tar::file(flate2::read::GzDecoder::new(file), &inside)?
        .ok_or_else(|| format!("{url} holds no {inside}"))?;
    let partial = partial_path(&destination);
    std::fs::write(&partial, binary)
        .map_err(|error| format!("could not write {}: {error}", partial.display()))?;
    make_executable(&partial)?;
    std::fs::rename(&partial, &destination)
        .map_err(|error| format!("could not write {}: {error}", destination.display()))?;
    let _ = std::fs::remove_file(&archive);
    eprintln!("Installed cargo {version} at {}", destination.display());
    Ok(destination)
}
