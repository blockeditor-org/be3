//! Where `be` finds the apps whose host components it runs (guides/apps/runtime.md,
//! "Headless"), as be-host finds its own: a build that was not deployed reads the apps staged
//! beside it (`//crates/be-tool-cli:dev`, or `BE_BUNDLE`); a deployed `be` ships alone, so it
//! reads them from its release, the web bundle a deploy publishes at `data/BUILD/web/` on the
//! mirror, a file at a time and only as the runtime reads it. No app's wasm, list or hash is
//! compiled into `be` (its manifests and type modules still are, plans/headless_apps.md step
//! 1d): its `build-info.json`, which ships beside it as be-host's ships beside
//! be-host, names the release's apps and the SHA-256 of their `app-hashes.json`
//! (`be_lib_build_id::ReleaseApps`, written by `./scripts/deploy`). That list must hash to it,
//! and each file to what the list says, so `be` runs only the apps of the build it was
//! downloaded with, whatever the mirror serves later. Each is kept in `be`'s cache under its
//! hash, so the next run reads it from there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use be_lib_build_id::ReleaseApps;
use sha2::{Digest, Sha256};

/// The file a release lists its apps' hashes in, beside them.
pub const HASHES: &str = "app-hashes.json";

/// Serves the apps to `be_host_stack::discovery` from the release `be`'s `build-info.json`
/// names, cached under `cache`; a build that was not deployed reads the apps staged beside it,
/// as be-host does. Called before the apps are first discovered; later calls change nothing.
pub fn serve(cache: &Path) -> Result<(), String> {
    let metadata = be_lib_build_id::metadata()?;
    let Some(apps) = metadata.apps.clone() else {
        // Only a deploy names a default server, and a deploy names the apps unless it skipped
        // the web bundle they are published in.
        if metadata.default_server.is_some() {
            return Err(format!(
                "this be was deployed without its release's apps (`./scripts/deploy --skip-web`), \
                 so its {} names none and it runs no app's host component",
                be_lib_build_id::FILE
            ));
        }
        return Ok(());
    };
    let release = Release::new(apps, cache.join("apps"), download);
    be_host_stack::discovery::use_assets(move |path| release.read(path));
    Ok(())
}

/// A release's app files, downloaded as they are read and cached by hash.
pub struct Release<F> {
    apps: ReleaseApps,
    cache: PathBuf,
    fetch: F,
    hashes: OnceLock<Result<HashMap<String, String>, String>>,
    /// Serializes downloads, so two readers of one file fetch it once.
    fetching: Mutex<()>,
}

impl<F: Fn(&str) -> Result<Vec<u8>, String>> Release<F> {
    /// The release `apps` names, its files cached in `cache`, each fetched by URL with `fetch`.
    pub fn new(apps: ReleaseApps, cache: PathBuf, fetch: F) -> Self {
        Self {
            apps,
            cache,
            fetch,
            hashes: OnceLock::new(),
            fetching: Mutex::new(()),
        }
    }

    /// The file at `path` in the release, which must hash to what the release lists.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let expected = self.expected(path)?;
        self.fetch_checked(path, &expected)
            .map_err(|error| format!("{error}, which the release lists for {path}"))
    }

    /// The file at `path`, from the cache or downloaded, which must hash to `expected`.
    fn fetch_checked(&self, path: &str, expected: &str) -> Result<Vec<u8>, String> {
        let cached = self.cache.join(expected);
        let _fetching = self.fetching.lock().unwrap_or_else(PoisonError::into_inner);
        if let Ok(bytes) = std::fs::read(&cached)
            && sha256(&bytes) == expected
        {
            return Ok(bytes);
        }
        let url = format!("{}/{path}", self.apps.url);
        let bytes = (self.fetch)(&url).map_err(|error| format!("could not download {url}: {error}"))?;
        let found = sha256(&bytes);
        if found != expected {
            return Err(format!("{url} has SHA-256 {found}, not {expected}"));
        }
        keep(&cached, &bytes);
        Ok(bytes)
    }

    fn expected(&self, path: &str) -> Result<String, String> {
        let hashes = self.hashes.get_or_init(|| {
            let listed = self.fetch_checked(HASHES, &self.apps.sha256).map_err(|error| {
                format!("{error}, which this be's {} names", be_lib_build_id::FILE)
            })?;
            serde_json::from_slice(&listed)
                .map_err(|error| format!("{}/{HASHES} does not read: {error}", self.apps.url))
        });
        let hashes = hashes.as_ref().map_err(Clone::clone)?;
        hashes
            .get(path)
            .cloned()
            .ok_or_else(|| format!("{}/{HASHES} lists no {path}", self.apps.url))
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
