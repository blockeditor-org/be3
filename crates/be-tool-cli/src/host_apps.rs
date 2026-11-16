//! Where `be` finds the apps whose host components it runs (guides/apps/runtime.md,
//! "Headless"): downloaded from the release it was built for, the web bundle a deploy
//! publishes at `data/BUILD/web/` on the mirror, a file at a time and only as the runtime
//! reads it, so `be` stays one small file and fetches only the app it runs, with its type
//! module and the libraries it links. Each file is checked against the SHA-256 the release
//! lists in its `app-hashes.json` and kept in `be`'s cache under that hash, so the next run
//! reads it from there. With `BE_BUNDLE` set (tests, local builds) the apps are read from
//! that directory instead, as be-host reads its own, and nothing is downloaded.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use sha2::{Digest, Sha256};

/// The mirror deploys publish releases to (guides/hosting.md, "Deploying").
const MIRROR: &str = "https://be3.pfg.pw";

/// The file a release lists its apps' hashes in, beside them.
pub const HASHES: &str = "app-hashes.json";

/// Serves the apps to `be_host_stack::discovery`: from the release `be` was built for, cached
/// under `cache`, unless `BE_BUNDLE` names a directory to read them from. Called before the
/// apps are first discovered; later calls change nothing.
pub fn serve(cache: &Path) -> Result<(), String> {
    if std::env::var_os("BE_BUNDLE").is_some_and(|list| !list.is_empty()) {
        return Ok(());
    }
    let build = be_lib_build_id::current()?;
    let mirror = std::env::var("BE3_MIRROR").unwrap_or_else(|_| MIRROR.to_owned());
    let release = Release::new(
        format!("{mirror}/data/{build}/web"),
        cache.join("apps"),
        download,
    );
    be_host_stack::discovery::use_assets(move |path| release.read(path));
    Ok(())
}

/// A release's app files, downloaded as they are read and cached by hash.
pub struct Release<F> {
    base: String,
    cache: PathBuf,
    fetch: F,
    hashes: OnceLock<Result<HashMap<String, String>, String>>,
    /// Serializes downloads, so two readers of one file fetch it once.
    fetching: Mutex<()>,
}

impl<F: Fn(&str) -> Result<Vec<u8>, String>> Release<F> {
    /// The release at `base` (a URL without a trailing slash), its files cached in `cache`,
    /// each fetched by URL with `fetch`.
    pub fn new(base: String, cache: PathBuf, fetch: F) -> Self {
        Self {
            base,
            cache,
            fetch,
            hashes: OnceLock::new(),
            fetching: Mutex::new(()),
        }
    }

    /// The file at `path` in the release, which must hash to what the release lists.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let expected = self.expected(path)?;
        let cached = self.cache.join(&expected);
        let _fetching = self.fetching.lock().unwrap_or_else(PoisonError::into_inner);
        if let Ok(bytes) = std::fs::read(&cached)
            && sha256(&bytes) == expected
        {
            return Ok(bytes);
        }
        let url = format!("{}/{path}", self.base);
        let bytes = (self.fetch)(&url).map_err(|error| format!("could not download {url}: {error}"))?;
        let found = sha256(&bytes);
        if found != expected {
            return Err(format!(
                "{url} has SHA-256 {found}, but its release lists {expected} for {path}"
            ));
        }
        keep(&cached, &bytes);
        Ok(bytes)
    }

    fn expected(&self, path: &str) -> Result<String, String> {
        let hashes = self.hashes.get_or_init(|| {
            let url = format!("{}/{HASHES}", self.base);
            let listed = (self.fetch)(&url).map_err(|error| {
                format!(
                    "could not download {url}, which lists the release's apps: {error} (a release \
                     deployed before it was published has none)"
                )
            })?;
            serde_json::from_slice(&listed).map_err(|error| format!("{url} does not read: {error}"))
        });
        let hashes = hashes.as_ref().map_err(Clone::clone)?;
        hashes
            .get(path)
            .cloned()
            .ok_or_else(|| format!("{}/{HASHES} lists no {path}", self.base))
    }
}

/// Writes `bytes` to `path` whole or not at all; a cache that cannot be written is only slower.
fn keep(path: &Path, bytes: &[u8]) {
    let Some(directory) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(directory).is_err() {
        return;
    }
    let partial = path.with_extension("partial");
    if std::fs::write(&partial, bytes).is_ok() && std::fs::rename(&partial, path).is_err() {
        let _ = std::fs::remove_file(&partial);
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .https_only(true)
        .try_proxy_from_env(true)
        .build();
    let response = agent.get(url).call().map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;
