use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

const PINS: &str = "scripts/internal/pins";

pub enum Outcome {
    Current,
    Published(Vec<String>),
    Stale,
}

pub fn check(built: Option<&Path>, out: Option<&Path>, check: bool) -> Result<Outcome, String> {
    let built = built.ok_or("crates/bootstrap's release did not build")?;
    let listed = fs::read_to_string(built.join("unpinned"))
        .map_err(|error| format!("could not read {}: {error}", built.display()))?;
    let values: Vec<(String, String)> = listed
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
    if values.is_empty() {
        return Ok(Outcome::Current);
    }
    let (Some(out), false) = (out, check) else {
        return Ok(Outcome::Stale);
    };
    fs::create_dir_all(out)
        .map_err(|error| format!("could not create {}: {error}", out.display()))?;
    for (_, sha256) in &values {
        fs::copy(built.join(sha256), out.join(sha256))
            .map_err(|error| format!("could not copy the binary {sha256}: {error}"))?;
    }
    let pins =
        fs::read_to_string(PINS).map_err(|error| format!("could not read {PINS}: {error}"))?;
    let written = set(&pins, &values)?;
    fs::write(PINS, written).map_err(|error| format!("could not write {PINS}: {error}"))?;
    Ok(Outcome::Published(
        values
            .into_iter()
            .map(|(name, value)| format!("{name}='{value}'"))
            .collect(),
    ))
}

pub fn set(pins: &str, values: &[(String, String)]) -> Result<String, String> {
    let mut lines: Vec<String> = pins.lines().map(str::to_owned).collect();
    for (name, value) in values {
        let line = lines
            .iter_mut()
            .find(|line| line.starts_with(&format!("{name}=")))
            .ok_or_else(|| format!("{PINS} has no {name}= line"))?;
        *line = format!("{name}='{value}'");
    }
    let mut written = lines.join("\n");
    if pins.ends_with('\n') {
        written.push('\n');
    }
    Ok(written)
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
