//! The tools' blocking client for a Bunny storage zone and the CDN in front of it, which
//! be-tool-local's `deploy` and be-tool-build-helper upload with. Bunny's storage API takes the
//! zone's password as an `AccessKey` header on every call and checks a `Checksum` header (the
//! uppercase SHA-256 of the body) on a `PUT`. Its CDN answers `HEAD` without one.
//!
//! Calls through `https://` go through the environment's proxy; plain `http://`, which only tests
//! use, does not. A call that fails to connect, or that the server answers 429 or 5xx, is tried
//! up to three times more.

use std::{fs::File, io::Read, path::Path, time::Duration};

use be_lib_sha256::sha256_read;

/// What a `PUT` sends.
#[derive(Clone, Copy)]
pub enum Body<'a> {
    Bytes(&'a [u8]),
    /// A file, streamed from disk rather than read into memory.
    File(&'a Path),
}

/// What [`Storage::put_if_absent`] found.
#[derive(Debug, PartialEq, Eq)]
pub enum Put {
    /// Nothing was there, and now the body is.
    Stored,
    /// The same bytes were already there, so nothing was sent.
    Present,
}

pub struct Storage {
    key: Option<String>,
    proxied: ureq::Agent,
    direct: ureq::Agent,
}

const RETRIES: u32 = 3;

impl Storage {
    /// A client that sends `key`, the storage zone's password, with every `get` and `put`. Without
    /// one, only [`Storage::head`] and [`Storage::agent`] can be of use.
    pub fn new(key: Option<String>) -> Self {
        let builder = || {
            ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(30))
                .timeout_read(Duration::from_secs(600))
                .timeout_write(Duration::from_secs(600))
        };
        Self {
            key,
            proxied: builder().try_proxy_from_env(true).build(),
            direct: builder().build(),
        }
    }

    /// Whether it was given a key, without which `get` and `put` fail before sending anything.
    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    /// The agent for other calls to `url`, such as reading a file back through the CDN.
    pub fn agent(&self, url: &str) -> &ureq::Agent {
        match url.starts_with("https://") {
            true => &self.proxied,
            false => &self.direct,
        }
    }

    /// Whether `url` answers a `HEAD`, which is sent without the key, as the CDN takes it.
    pub fn head(&self, url: &str) -> Result<bool, String> {
        match self.call(url, || self.agent(url).head(url).call()) {
            Ok(_) => Ok(true),
            Err(ureq::Error::Status(404, _)) => Ok(false),
            Err(error) => Err(format!("asking for {url} failed: {error}")),
        }
    }

    /// What the storage zone holds at `url`, or `None` when it holds nothing there.
    pub fn get(&self, url: &str) -> Result<Option<Box<dyn Read + Send + Sync>>, String> {
        let key = self.key()?;
        match self.call(url, || self.agent(url).get(url).set("AccessKey", key).call()) {
            Ok(response) => Ok(Some(response.into_reader())),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(error) => Err(format!("reading {url} failed: {error}")),
        }
    }

    /// Uploads `body`, whose SHA-256 is `sha256`, to `url`, replacing whatever was there.
    pub fn put(
        &self,
        url: &str,
        body: Body<'_>,
        sha256: &str,
        content_type: &str,
    ) -> Result<(), String> {
        let key = self.key()?;
        let checksum = sha256.to_ascii_uppercase();
        if let Body::File(path) = body {
            File::open(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        }
        self.call(url, || {
            let request = self
                .agent(url)
                .put(url)
                .set("AccessKey", key)
                .set("Checksum", &checksum)
                .set("Content-Type", content_type);
            match body {
                Body::Bytes(bytes) => request.send_bytes(bytes),
                // With its Content-Length, so the file goes as it is rather than chunked.
                Body::File(path) => {
                    let file = File::open(path)?;
                    let size = file.metadata()?.len();
                    request.set("Content-Length", &size.to_string()).send(file)
                }
            }
        })
        .map_err(|error| format!("uploading to {url} failed: {error}"))?;
        Ok(())
    }

    /// Uploads `body` to `url` unless the zone already holds it there, for a path that names one
    /// set of bytes and never changes: it refuses to replace different bytes.
    pub fn put_if_absent(
        &self,
        url: &str,
        body: Body<'_>,
        sha256: &str,
        content_type: &str,
    ) -> Result<Put, String> {
        let Some(stored) = self.get(url)? else {
            self.put(url, body, sha256, content_type)?;
            return Ok(Put::Stored);
        };
        let (existing, _) =
            sha256_read(stored).map_err(|error| format!("reading {url} failed: {error}"))?;
        if existing != sha256 {
            return Err(format!(
                "refusing to overwrite {url}, which holds other bytes (SHA-256 {existing}, not {sha256})"
            ));
        }
        Ok(Put::Present)
    }

    fn key(&self) -> Result<&str, String> {
        self.key
            .as_deref()
            .ok_or_else(|| "no storage key was given".to_owned())
    }

    fn call(
        &self,
        url: &str,
        mut send: impl FnMut() -> Result<ureq::Response, ureq::Error>,
    ) -> Result<ureq::Response, ureq::Error> {
        let mut wait = Duration::from_secs(1);
        let mut tries = 0;
        loop {
            let result = send();
            let transient = match &result {
                Err(ureq::Error::Transport(_)) => true,
                Err(ureq::Error::Status(status, _)) => *status == 429 || *status >= 500,
                Ok(_) => false,
            };
            if !transient || tries == RETRIES {
                return result;
            }
            tries += 1;
            eprintln!("{url}: trying again in {}s", wait.as_secs());
            std::thread::sleep(wait);
            wait *= 2;
        }
    }
}

#[cfg(test)]
mod tests;
