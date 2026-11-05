//! Where be-server keeps object bytes (decisions/objects_live_in_s3_compatible_storage.md): a
//! `FileStore` under `DATA/objects` when no bucket is configured, or an S3 bucket behind a
//! bounded on-disk cache under `DATA/object-cache`.
//!
//! Only the S3 API is spoken, path-style (`ENDPOINT/BUCKET/KEY`), and only what every
//! S3-compatible store has: single-request PUT, GET, HEAD, DELETE and ListObjectsV2. An
//! object's key is its hash in lowercase hex, flat at the top of the bucket (S3 has no
//! directories to shard, and a flat key is what a client is handed in a presigned URL). Every
//! request, the server's own included, is a SigV4 presigned URL from `rusty-s3`, sent with
//! `ureq`; a PUT signs its `Content-Length` and its `x-amz-checksum-sha256` (the base64 of the
//! object's SHA-256, which is its `Hash`), so the bucket refuses any other bytes.

use std::{
    collections::{BTreeMap, HashMap},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime},
};

use base64::Engine;
use be_block_store::{FileStore, Hash, ObjectStore, StoreError};
use rusty_s3::{S3Action, UrlStyle};

/// The bytes the cache in front of a bucket keeps when the server is not told otherwise
/// (`--object-cache`).
pub const DEFAULT_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// How long the URLs the server signs for its own requests stay valid.
const OWN_REQUEST_TTL: Duration = Duration::from_secs(10 * 60);
/// Objects a bucket-backed store reads at once (`Objects::get_many`).
pub(crate) const PARALLEL_READS: usize = 16;
/// Keys asked for in one ListObjectsV2 page.
const LIST_PAGE: usize = 1000;

/// An S3 bucket, addressed path-style, and the credentials that sign for it.
#[derive(Clone)]
pub struct Bucket {
    bucket: rusty_s3::Bucket,
    credentials: rusty_s3::Credentials,
}

impl std::fmt::Debug for Bucket {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Bucket")
            .field("url", &self.bucket.base_url().as_str())
            .field("region", &self.bucket.region())
            .finish_non_exhaustive()
    }
}

/// A presigned request: the URL, and the headers that were signed with it, which must be sent
/// with exactly these values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Presigned {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
}

impl Bucket {
    /// `endpoint` is the S3 API's base URL (`https://storage.bunnycdn.com`, or a test server's
    /// `http://127.0.0.1:PORT`), `name` the bucket (a Bunny storage zone).
    pub fn new(
        endpoint: &str,
        region: &str,
        name: &str,
        access_key: &str,
        secret_key: &str,
    ) -> Result<Self, String> {
        let url = endpoint
            .parse()
            .map_err(|error| format!("the S3 endpoint {endpoint} is not a URL: {error}"))?;
        let bucket =
            rusty_s3::Bucket::new(url, UrlStyle::Path, name.to_owned(), region.to_owned())
                .map_err(|error| format!("the S3 bucket {name} at {endpoint}: {error}"))?;
        Ok(Self {
            bucket,
            credentials: rusty_s3::Credentials::new(access_key, secret_key),
        })
    }

    /// The key an object is stored under: its hash in lowercase hex.
    pub fn key(hash: Hash) -> String {
        hash.to_hex()
    }

    /// A URL anyone holding it may GET the object from until `ttl` passes (at most 7 days).
    pub fn presign_get(&self, hash: Hash, ttl: Duration) -> String {
        let key = Self::key(hash);
        self.bucket
            .get_object(Some(&self.credentials), &key)
            .sign(ttl)
            .into()
    }

    /// A URL that uploads exactly the object `hash` of `length` bytes until `ttl` passes: the
    /// length and the checksum are signed headers, so the bucket refuses a request without
    /// them or with other values, and a body whose SHA-256 is not `hash`.
    pub fn presign_put(&self, hash: Hash, length: u64, ttl: Duration) -> Presigned {
        let key = Self::key(hash);
        let headers = vec![
            ("content-length", length.to_string()),
            ("x-amz-checksum-sha256", checksum(hash)),
        ];
        let mut action = self.bucket.put_object(Some(&self.credentials), &key);
        for (name, value) in &headers {
            action.headers_mut().insert(*name, value.clone());
        }
        Presigned {
            url: action.sign(ttl).into(),
            headers,
        }
    }

    fn presign_head(&self, hash: Hash) -> String {
        let key = Self::key(hash);
        self.bucket
            .head_object(Some(&self.credentials), &key)
            .sign(OWN_REQUEST_TTL)
            .into()
    }

    fn presign_delete(&self, hash: Hash) -> String {
        let key = Self::key(hash);
        self.bucket
            .delete_object(Some(&self.credentials), &key)
            .sign(OWN_REQUEST_TTL)
            .into()
    }
}

/// The `x-amz-checksum-sha256` of an object: the base64 of its SHA-256, which is its hash.
pub fn checksum(hash: Hash) -> String {
    base64::engine::general_purpose::STANDARD.encode(hash.as_bytes())
}

/// Objects in a bucket. `get` checks each object's hash as a `FileStore` does, so a tampered
/// object reads as missing; `remove` of an object already gone succeeds; `sync` has nothing to
/// do, since a PUT is durable once the bucket answers it.
#[derive(Clone)]
pub struct S3Store {
    bucket: Bucket,
    agent: ureq::Agent,
}

impl S3Store {
    pub fn new(bucket: Bucket) -> Self {
        Self {
            bucket,
            agent: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(10))
                .timeout(Duration::from_secs(120))
                .build(),
        }
    }

    pub fn bucket(&self) -> &Bucket {
        &self.bucket
    }

    /// Every object the bucket holds, in no order, skipping keys that are not a hash. It lists
    /// the whole bucket, so it is for one-time maintenance, never a request.
    pub fn hashes(&self) -> Result<Vec<Hash>, StoreError> {
        let mut hashes = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut action = self
                .bucket
                .bucket
                .list_objects_v2(Some(&self.bucket.credentials));
            action.with_max_keys(LIST_PAGE);
            if let Some(token) = &token {
                action.with_continuation_token(token.clone());
            }
            let url = action.sign(OWN_REQUEST_TTL);
            let body = self
                .agent
                .get(url.as_str())
                .call()
                .map_err(failed)?
                .into_string()?;
            let listed = rusty_s3::actions::ListObjectsV2::parse_response(&body)
                .map_err(|error| io::Error::other(format!("the bucket's listing: {error}")))?;
            hashes.extend(
                listed
                    .contents
                    .iter()
                    .filter_map(|object| Hash::from_hex(&object.key)),
            );
            match listed.next_continuation_token {
                Some(next) => token = Some(next),
                None => return Ok(hashes),
            }
        }
    }
}

fn failed(error: ureq::Error) -> StoreError {
    StoreError::Io(io::Error::other(match error {
        ureq::Error::Status(status, response) => {
            let body = response.into_string().unwrap_or_default();
            format!("the bucket answered {status}: {body}")
        }
        ureq::Error::Transport(transport) => format!("the bucket is unreachable: {transport}"),
    }))
}

impl ObjectStore for S3Store {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        let hash = Hash::of(bytes);
        let presigned = self
            .bucket
            .presign_put(hash, bytes.len() as u64, OWN_REQUEST_TTL);
        let mut request = self.agent.put(&presigned.url);
        for (name, value) in &presigned.headers {
            request = request.set(name, value);
        }
        request.send_bytes(bytes).map_err(failed)?;
        Ok(hash)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        let url = self.bucket.presign_get(hash, OWN_REQUEST_TTL);
        let response = match self.agent.get(&url).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(404, _)) => return Ok(None),
            Err(error) => return Err(failed(error)),
        };
        let mut bytes = Vec::new();
        response.into_reader().read_to_end(&mut bytes)?;
        Ok((Hash::of(&bytes) == hash).then_some(bytes))
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        Ok(self.len(hash)?.is_some())
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        match self.agent.delete(&self.bucket.presign_delete(hash)).call() {
            Ok(_) | Err(ureq::Error::Status(404, _)) => Ok(()),
            Err(error) => Err(failed(error)),
        }
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        match self.agent.head(&self.bucket.presign_head(hash)).call() {
            Ok(response) => Ok(Some(
                response
                    .header("content-length")
                    .and_then(|length| length.parse().ok())
                    .ok_or_else(|| io::Error::other("the bucket's HEAD had no Content-Length"))?,
            )),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }
}

/// A bucket for the server's objects and how many bytes of them to keep on its own disk.
#[derive(Clone, Debug)]
pub struct BucketConfig {
    pub bucket: Bucket,
    pub cache_bytes: u64,
}

/// The server's object store: `ServerConfig::bucket` chooses it, and everything that reads or
/// writes objects (the store, usage, backups) goes through it.
#[derive(Clone)]
pub enum Objects {
    /// A `FileStore` under `DATA/objects`, when no bucket is configured.
    Local(FileStore),
    /// A bucket, with the objects read or written most recently kept on disk.
    Bucket(Cached),
}

impl Objects {
    pub fn open(data_dir: &Path, bucket: Option<&BucketConfig>) -> Result<Self, StoreError> {
        Ok(match bucket {
            None => Self::Local(FileStore::open(data_dir.join("objects"))?),
            Some(config) => Self::Bucket(Cached::open(
                S3Store::new(config.bucket.clone()),
                data_dir.join("object-cache"),
                config.cache_bytes,
            )?),
        })
    }

    /// The bucket objects live in, when there is one.
    pub fn bucket(&self) -> Option<&Bucket> {
        match self {
            Self::Local(_) => None,
            Self::Bucket(cached) => Some(cached.remote.bucket()),
        }
    }

    /// Every object the store holds, in no order: for maintenance, never a request.
    pub fn hashes(&self) -> Result<Vec<Hash>, StoreError> {
        match self {
            Self::Local(store) => store.hashes(),
            Self::Bucket(cached) => cached.remote.hashes(),
        }
    }

    /// Reads `hashes`, answering each in order. A bucket's are read `PARALLEL_READS` at a
    /// time, so a batch costs a few round trips rather than one each.
    pub fn get_many(&self, hashes: &[Hash]) -> Vec<Result<Option<Vec<u8>>, StoreError>> {
        match self {
            Self::Local(store) => hashes.iter().map(|hash| store.get(*hash)).collect(),
            Self::Bucket(cached) => parallel(hashes, |hash| cached.get(*hash)),
        }
    }

    /// Writes `objects`, answering their hashes in order; a bucket's are written
    /// `PARALLEL_READS` at a time. Like `put`, they still need a `sync`.
    pub fn put_many(&self, objects: &[Vec<u8>]) -> Result<Vec<Hash>, StoreError> {
        match self {
            Self::Local(store) => objects.iter().map(|bytes| store.put(bytes)).collect(),
            Self::Bucket(cached) => parallel(objects, |bytes| cached.put(bytes))
                .into_iter()
                .collect(),
        }
    }
}

/// `work` on each of `items`, `PARALLEL_READS` at a time on threads of their own, answered in
/// order. For requests to a bucket, which wait on the network rather than the processor.
fn parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    items
        .chunks(PARALLEL_READS)
        .flat_map(|chunk| {
            std::thread::scope(|scope| {
                let running: Vec<_> = chunk
                    .iter()
                    .map(|item| scope.spawn(|| work(item)))
                    .collect();
                running
                    .into_iter()
                    .map(|done| done.join().expect("an object request never panics"))
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

impl ObjectStore for Objects {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        match self {
            Self::Local(store) => store.put(bytes),
            Self::Bucket(cached) => cached.put(bytes),
        }
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        match self {
            Self::Local(store) => store.get(hash),
            Self::Bucket(cached) => cached.get(hash),
        }
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        match self {
            Self::Local(store) => store.has(hash),
            Self::Bucket(cached) => cached.has(hash),
        }
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        match self {
            Self::Local(store) => store.remove(hash),
            Self::Bucket(cached) => cached.remove(hash),
        }
    }

    fn sync(&self) -> Result<(), StoreError> {
        match self {
            Self::Local(store) => store.sync(),
            Self::Bucket(_) => Ok(()),
        }
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        match self {
            Self::Local(store) => store.len(hash),
            Self::Bucket(cached) => cached.len(hash),
        }
    }
}

/// A bucket behind a cache on disk: a `FileStore` (`FileStore::open_cache`, which never syncs)
/// holding at most `limit` bytes, letting go of the least recently used object first. A put
/// is written to the bucket and then the cache; a read the cache misses is fetched and
/// cached. Recency is kept in memory; on opening, files' modification times order what the
/// cache already holds.
#[derive(Clone)]
pub struct Cached {
    remote: S3Store,
    cache: FileStore,
    limit: u64,
    recency: Arc<Mutex<Recency>>,
}

impl Cached {
    pub fn open(remote: S3Store, root: PathBuf, limit: u64) -> Result<Self, StoreError> {
        let cache = FileStore::open_cache(&root)?;
        let mut held = Vec::new();
        for hash in cache.hashes()? {
            let hex = hash.to_hex();
            let metadata = std::fs::metadata(root.join(&hex[..2]).join(&hex[2..]))?;
            held.push((
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                hash,
                metadata.len(),
            ));
        }
        held.sort_unstable();
        let cached = Self {
            remote,
            cache,
            limit,
            recency: Arc::default(),
        };
        {
            let mut recency = cached.recency();
            for (_, hash, bytes) in held {
                recency.touch(hash, bytes);
            }
        }
        cached.evict()?;
        Ok(cached)
    }

    pub fn remote(&self) -> &S3Store {
        &self.remote
    }

    /// The bytes the cache holds now.
    pub fn cached_bytes(&self) -> u64 {
        self.recency().bytes
    }

    fn recency(&self) -> std::sync::MutexGuard<'_, Recency> {
        self.recency.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn keep(&self, bytes: &[u8]) -> Result<(), StoreError> {
        let length = bytes.len() as u64;
        if length > self.limit {
            return Ok(());
        }
        let hash = self.cache.put(bytes)?;
        self.recency().touch(hash, length);
        self.evict()
    }

    fn evict(&self) -> Result<(), StoreError> {
        let mut recency = self.recency();
        while recency.bytes > self.limit {
            let Some(oldest) = recency.oldest() else {
                break;
            };
            recency.forget(oldest);
            self.cache.remove(oldest)?;
        }
        Ok(())
    }
}

impl ObjectStore for Cached {
    fn put(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        let hash = self.remote.put(bytes)?;
        self.keep(bytes)?;
        Ok(hash)
    }

    fn get(&self, hash: Hash) -> Result<Option<Vec<u8>>, StoreError> {
        if let Some(bytes) = self.cache.get(hash)? {
            self.recency().touch(hash, bytes.len() as u64);
            return Ok(Some(bytes));
        }
        self.recency().forget(hash);
        let Some(bytes) = self.remote.get(hash)? else {
            return Ok(None);
        };
        self.keep(&bytes)?;
        Ok(Some(bytes))
    }

    fn has(&self, hash: Hash) -> Result<bool, StoreError> {
        Ok(self.cache.has(hash)? || self.remote.has(hash)?)
    }

    fn remove(&self, hash: Hash) -> Result<(), StoreError> {
        self.remote.remove(hash)?;
        self.recency().forget(hash);
        self.cache.remove(hash)
    }

    fn len(&self, hash: Hash) -> Result<Option<usize>, StoreError> {
        match self.cache.len(hash)? {
            Some(length) => Ok(Some(length)),
            None => self.remote.len(hash),
        }
    }
}

/// The cache's objects from least to most recently used, and their bytes.
#[derive(Default)]
struct Recency {
    bytes: u64,
    next: u64,
    by_hash: HashMap<Hash, (u64, u64)>,
    by_use: BTreeMap<u64, Hash>,
}

impl Recency {
    fn touch(&mut self, hash: Hash, bytes: u64) {
        self.forget(hash);
        let used = self.next;
        self.next += 1;
        self.by_hash.insert(hash, (used, bytes));
        self.by_use.insert(used, hash);
        self.bytes += bytes;
    }

    fn forget(&mut self, hash: Hash) {
        if let Some((used, bytes)) = self.by_hash.remove(&hash) {
            self.by_use.remove(&used);
            self.bytes -= bytes;
        }
    }

    fn oldest(&self) -> Option<Hash> {
        self.by_use.values().next().copied()
    }
}

#[cfg(test)]
pub(crate) mod tests;
