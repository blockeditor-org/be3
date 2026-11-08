//! The binaries of crates/be-tool-bootstrap's release compared with the ones
//! scripts/internal/pins names (//scripts/internal:bootstrap-unpinned), so that
//! nothing is downloaded while they match. OUT/unpinned lists
//! "<pin>=<sha256>" for each that does not, and with --publish each such binary
//! is beside it, named by its SHA-256, for CI to upload and pin.

use std::fmt::Write;
use std::path::Path;
use std::process::ExitCode;

use sha2::{Digest, Sha256};

pub fn run(arguments: &[String]) -> Result<ExitCode, String> {
    let (out, dist, pins, publish) = match arguments {
        [out, dist, pins] => (out, dist, pins, false),
        [out, dist, pins, flag] if flag == "--publish" => (out, dist, pins, true),
        _ => return Err("usage: unpinned OUT DIST PINS [--publish]".into()),
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)
        .map_err(|error| format!("cannot create {}: {error}", out.display()))?;
    let pins =
        std::fs::read_to_string(pins).map_err(|error| format!("cannot read {pins}: {error}"))?;
    let mut triples: Vec<_> = std::fs::read_dir(dist)
        .map_err(|error| format!("cannot read {dist}: {error}"))?
        .flatten()
        .map(|entry| entry.path())
        .collect();
    triples.sort();
    let mut unpinned = String::new();
    for name in ["bootstrap", "bootstrap.exe"] {
        for triple in &triples {
            let binary = triple.join(name);
            if !binary.is_file() {
                continue;
            }
            let contents = std::fs::read(&binary)
                .map_err(|error| format!("cannot read {}: {error}", binary.display()))?;
            let sha256 = hex(&Sha256::digest(&contents));
            let pin = pin_name(&triple.file_name().unwrap_or_default().to_string_lossy());
            if pinned(&pins, &pin, &sha256) {
                continue;
            }
            let _ = writeln!(unpinned, "{pin}={sha256}");
            if publish {
                std::fs::write(out.join(&sha256), &contents).map_err(|error| error.to_string())?;
            }
        }
    }
    std::fs::write(out.join("unpinned"), unpinned).map_err(|error| error.to_string())?;
    Ok(ExitCode::SUCCESS)
}

/// The name scripts/internal/pins gives a target triple's bootstrap.
pub fn pin_name(triple: &str) -> String {
    format!("bootstrap_sha256_{}", triple.replace('-', "_"))
}

/// Whether pins holds the line `<pin>='<sha256>'`.
pub fn pinned(pins: &str, pin: &str, sha256: &str) -> bool {
    let line = format!("{pin}='{sha256}'");
    pins.lines().any(|candidate| candidate == line)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}
