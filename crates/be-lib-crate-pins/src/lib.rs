//! The crates Cargo.lock names from crates.io, and the last part of
//! buck/cargo/crates.bzl, `downloads`, which pins each by its SHA-256 and size
//! for buck2 to download (buck/cargo/third_party.bzl). crates/be-tool-bootstrap's
//! lock step writes the table after bringing Cargo.lock up to date, and
//! crates/be-tool-buck's generate writes it again with the rest of crates.bzl,
//! both through [`render`], so the two agree byte for byte.

use std::collections::BTreeMap;

/// What the table starts with, on a line of its own after the rest of crates.bzl.
pub const DOWNLOADS: &str = "downloads = ";

/// A crate Cargo.lock names with a checksum: one from crates.io.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locked {
    pub name: String,
    pub version: String,
    pub sha256: String,
}

impl Locked {
    /// Where crates.io serves the archive whose SHA-256 is [`Locked::sha256`].
    pub fn url(&self) -> String {
        format!(
            "https://static.crates.io/crates/{name}/{name}-{version}.crate",
            name = self.name,
            version = self.version
        )
    }
}

/// Every crate Cargo.lock names with a checksum, by `name-version`.
pub fn locked(lock: &str) -> BTreeMap<String, Locked> {
    let mut found = BTreeMap::new();
    let mut name = "";
    let mut version = "";
    for line in lock.lines().map(str::trim) {
        let value = |prefix: &str| {
            line.strip_prefix(prefix)
                .map(|rest| rest.trim().trim_matches('"'))
        };
        if line == "[[package]]" {
            name = "";
            version = "";
        } else if let Some(value) = value("name =") {
            name = value;
        } else if let Some(value) = value("version =") {
            version = value;
        } else if let Some(value) = value("checksum =") {
            found.insert(
                format!("{name}-{version}"),
                Locked {
                    name: name.to_owned(),
                    version: version.to_owned(),
                    sha256: value.to_owned(),
                },
            );
        }
    }
    found
}

/// crates.bzl split before its downloads, or None when it has none.
pub fn split(crates_bzl: &str) -> Option<(&str, &str)> {
    let at = crates_bzl.find(&format!("\n{DOWNLOADS}"))? + 1;
    Some(crates_bzl.split_at(at))
}

/// The size each entry of a downloads table pins, by key.
pub fn pinned_sizes(downloads: &str) -> BTreeMap<String, u64> {
    let mut sizes = BTreeMap::new();
    let mut key = None;
    for line in downloads.lines() {
        if let Some(name) = line
            .strip_prefix("    \"")
            .and_then(|rest| rest.strip_suffix("\": {"))
        {
            key = Some(name.to_owned());
        } else if let Some(size) = line
            .trim()
            .strip_prefix("\"size_bytes\": ")
            .and_then(|rest| rest.trim_end_matches(',').parse().ok())
            && let Some(key) = key.take()
        {
            sizes.insert(key, size);
        }
    }
    sizes
}

/// The downloads table for every crate Cargo.lock names, built for some
/// platform or not, since cargo reads the whole lockfile (buck/cargo/buckify.sh).
/// Every one needs its size in `sizes`.
pub fn render(
    locked: &BTreeMap<String, Locked>,
    sizes: &BTreeMap<String, u64>,
) -> Result<String, String> {
    let mut text = format!("{DOWNLOADS}{{\n");
    for (key, crate_) in locked {
        let size = sizes
            .get(key)
            .ok_or_else(|| format!("no size for {key}, which Cargo.lock names"))?;
        for value in [key, &crate_.name, &crate_.version, &crate_.sha256] {
            if !value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._+-".contains(character))
            {
                return Err(format!("Cargo.lock names a crate as {value:?}"));
            }
        }
        text.push_str(&format!(
            "    \"{key}\": {{\n        \"name\": \"{}\",\n        \"sha256\": \"{}\",\n        \"size_bytes\": {size},\n        \"version\": \"{}\",\n    }},\n",
            crate_.name, crate_.sha256, crate_.version
        ));
    }
    text.push_str("}\n");
    Ok(text)
}

#[cfg(test)]
mod tests;
