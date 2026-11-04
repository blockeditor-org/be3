use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::Result;

pub struct Pins {
    path: PathBuf,
    values: BTreeMap<String, String>,
}

impl Pins {
    pub fn read() -> Result<Pins> {
        Pins::read_from(&crate::repository().join("scripts/internal/pins"))
    }

    pub fn read_from(path: &Path) -> Result<Pins> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        Ok(Pins {
            path: path.to_path_buf(),
            values: Pins::parse(&text),
        })
    }

    pub fn parse(text: &str) -> BTreeMap<String, String> {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .map(|(name, value)| {
                (
                    name.trim().to_owned(),
                    value.trim().trim_matches('\'').to_owned(),
                )
            })
            .collect()
    }

    pub fn get(&self, name: &str) -> Result<&str> {
        match self.values.get(name).map(String::as_str) {
            Some(value) if !value.is_empty() => Ok(value),
            _ => Err(format!("{} pins no {name}.", self.path.display())),
        }
    }

    pub fn sha256(&self, tool: &str, triple: &str) -> Result<&str> {
        let name = format!("{tool}_sha256_{}", triple.replace('-', "_"));
        self.get(&name).map_err(|_| {
            format!(
                "No {tool} release is pinned in {} for {triple}.",
                self.path.display()
            )
        })
    }
}
