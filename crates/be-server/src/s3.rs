//! be-server's own S3 endpoint: objects read and written with presigned S3 URLs, over the
//! server's object store, so clients move object bytes one way whether or not a bucket is
//! configured (guides/blocks/server.md, "Objects over S3").
//!
//! The endpoint is path-style with one bucket, [`BUCKET`], mounted at [`PATH`] below wherever the
//! server is reached (`http://127.0.0.1:9090/s3/objects/HASH`, or
//! `https://api.be3.pfg.pw/api/be/s3/objects/HASH` through the proxy). An object's key is its
//! hash in lowercase hex. Only GetObject (with Range), HeadObject and PutObject are served; every
//! request is signed with the server's own credentials ([`S3Endpoint`]).

use std::{sync::Arc, time::Duration};

use base64::Engine;
use be_block_store::{Hash, ObjectStore};
use bytes::Bytes;
use futures_util::StreamExt;
use s3s::{
    S3, S3Error, S3ErrorCode, S3Request, S3Response, S3Result,
    auth::SimpleAuth,
    config::{S3Config, StaticConfigProvider},
    dto::{
        ETag, GetObjectInput, GetObjectOutput, HeadObjectInput, HeadObjectOutput, PutObjectInput,
        PutObjectOutput, StreamingBlob,
    },
    host::{S3Host, VirtualHost},
    service::{S3Service, S3ServiceBuilder},
};

use crate::store::MAX_OBJECT_BYTES;

/// Where the endpoint is mounted, below the server's URL.
pub const PATH: &str = "s3/";
/// The one bucket.
pub const BUCKET: &str = "objects";
/// The region requests are signed for.
pub const REGION: &str = "be3";

/// What signs requests to the server's S3 endpoint: one credential pair the server owns,
/// derived from its own secret (`ServerKey`), so it lasts as long as the data directory and
/// survives a restore.
#[derive(Clone)]
pub struct S3Endpoint {
    pub access_key: String,
    pub secret_key: String,
}

impl std::fmt::Debug for S3Endpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3Endpoint")
            .field("access_key", &self.access_key)
            .finish_non_exhaustive()
    }
}

impl S3Endpoint {
    /// The bucket as reached at `server`, the URL the server is reached at (with its path
    /// behind a proxy, such as `https://api.be3.pfg.pw/api/be/`).
    pub fn bucket(&self, server: &url::Url) -> rusty_s3::Bucket {
        let mut endpoint = server.clone();
        if !endpoint.path().ends_with('/') {
            let path = format!("{}/", endpoint.path());
            endpoint.set_path(&path);
        }
        let endpoint = endpoint.join(PATH).expect("the mount is a relative path");
        rusty_s3::Bucket::new(endpoint, rusty_s3::UrlStyle::Path, BUCKET, REGION)
            .expect("the server's URL has a scheme and a host")
    }

    pub fn credentials(&self) -> rusty_s3::Credentials {
        rusty_s3::Credentials::new(self.access_key.clone(), self.secret_key.clone())
    }
}

/// A URL that reads the object `hash` from `bucket` until `expires` from now.
pub fn presign_get(
    bucket: &rusty_s3::Bucket,
    credentials: &rusty_s3::Credentials,
    hash: Hash,
    expires: Duration,
) -> url::Url {
    use rusty_s3::S3Action;
    let key = hash.to_hex();
    bucket.get_object(Some(credentials), &key).sign(expires)
}

/// A URL that uploads `length` bytes hashing to `hash` into `bucket` until `expires` from now.
/// The length and `x-amz-checksum-sha256` are signed, so the bucket takes nothing else; the
/// uploader sends that header with [`checksum`]'s value.
pub fn presign_put(
    bucket: &rusty_s3::Bucket,
    credentials: &rusty_s3::Credentials,
    hash: Hash,
    length: usize,
    expires: Duration,
) -> url::Url {
    use rusty_s3::S3Action;
    let key = hash.to_hex();
    let length = length.to_string();
    let checksum = checksum(hash);
    let mut action = bucket.put_object(Some(credentials), &key);
    let headers = action.headers_mut();
    headers.insert("content-length", length.as_str());
    headers.insert("x-amz-checksum-sha256", checksum.as_str());
    action.sign(expires)
}

/// The `x-amz-checksum-sha256` of the object `hash`: an object's hash is its SHA-256.
pub fn checksum(hash: Hash) -> String {
    base64::engine::general_purpose::STANDARD.encode(hash.as_bytes())
}

/// The S3 service the server answers at [`PATH`], over `objects`.
pub(crate) fn service(endpoint: &S3Endpoint, objects: Arc<dyn ObjectStore>) -> S3Service {
    let mut config = S3Config::default();
    config.expected_region = Some(REGION.parse().expect("the region is valid"));
    config.put_object_max_size = Some(MAX_OBJECT_BYTES as u64);
    let mut builder = S3ServiceBuilder::new(Objects { objects });
    builder.set_config(Arc::new(StaticConfigProvider::new(Arc::new(config))));
    builder.set_auth(SimpleAuth::from_single(
        endpoint.access_key.clone(),
        endpoint.secret_key.clone(),
    ));
    builder.set_host(OneBucket);
    builder.build()
}

/// Takes every request as one for [`BUCKET`], so s3s reads the whole path as the key and checks
/// signatures over the path the client signed, mount and all.
struct OneBucket;

impl S3Host for OneBucket {
    fn parse_host_header<'a>(&'a self, host: &'a str) -> S3Result<VirtualHost<'a>> {
        Ok(VirtualHost::new(host).with_bucket(BUCKET))
    }
}

/// GetObject, HeadObject and PutObject over an object store; every other operation is
/// s3s's NotImplemented.
struct Objects {
    objects: Arc<dyn ObjectStore>,
}

/// The hash a key names: its last segment (what precedes it is the mount), in lowercase hex.
fn named(key: &str) -> Option<Hash> {
    let name = key.rsplit('/').next().unwrap_or(key);
    Hash::from_hex(name).filter(|hash| hash.to_hex() == name)
}

fn tag(hash: Hash) -> ETag {
    ETag::Strong(hash.to_hex())
}

impl Objects {
    async fn blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce(&dyn ObjectStore) -> Result<T, be_block_store::StoreError> + Send + 'static,
    ) -> S3Result<T> {
        let objects = Arc::clone(&self.objects);
        tokio::task::spawn_blocking(move || work(objects.as_ref()))
            .await
            .map_err(S3Error::internal_error)?
            .map_err(S3Error::internal_error)
    }
}

#[async_trait::async_trait]
impl S3 for Objects {
    async fn get_object(
        &self,
        request: S3Request<GetObjectInput>,
    ) -> S3Result<S3Response<GetObjectOutput>> {
        let input = request.input;
        let hash = named(&input.key).ok_or_else(|| S3Error::new(S3ErrorCode::NoSuchKey))?;
        let bytes = self
            .blocking(move |objects| objects.get(hash))
            .await?
            .ok_or_else(|| S3Error::new(S3ErrorCode::NoSuchKey))?;
        let total = bytes.len() as u64;
        let mut body = Bytes::from(bytes);
        let mut content_range = None;
        if let Some(range) = input.range {
            let range = range
                .check(total)
                .map_err(|_| S3Error::new(S3ErrorCode::InvalidRange))?;
            content_range = Some(format!("bytes {}-{}/{total}", range.start, range.end - 1));
            body = body.slice(range.start as usize..range.end as usize);
        }
        Ok(S3Response::new(GetObjectOutput {
            accept_ranges: Some("bytes".to_owned()),
            content_length: Some(body.len() as i64),
            content_range,
            e_tag: Some(tag(hash)),
            body: Some(StreamingBlob::from(body)),
            ..GetObjectOutput::default()
        }))
    }

    async fn head_object(
        &self,
        request: S3Request<HeadObjectInput>,
    ) -> S3Result<S3Response<HeadObjectOutput>> {
        let hash = named(&request.input.key).ok_or_else(|| S3Error::new(S3ErrorCode::NoSuchKey))?;
        let length = self
            .blocking(move |objects| objects.len(hash))
            .await?
            .ok_or_else(|| S3Error::new(S3ErrorCode::NoSuchKey))?;
        Ok(S3Response::new(HeadObjectOutput {
            accept_ranges: Some("bytes".to_owned()),
            content_length: Some(length as i64),
            e_tag: Some(tag(hash)),
            ..HeadObjectOutput::default()
        }))
    }

    /// Stores the body only if the key, `x-amz-checksum-sha256` and the body all name the same
    /// hash, and answers once it is synced.
    async fn put_object(
        &self,
        request: S3Request<PutObjectInput>,
    ) -> S3Result<S3Response<PutObjectOutput>> {
        let input = request.input;
        let hash = named(&input.key).ok_or_else(|| {
            S3Error::with_message(
                S3ErrorCode::InvalidArgument,
                "an object's key is its SHA-256 in lowercase hex",
            )
        })?;
        let Some(sent) = input.checksum_sha256 else {
            return Err(S3Error::with_message(
                S3ErrorCode::InvalidRequest,
                "send x-amz-checksum-sha256",
            ));
        };
        if sent != checksum(hash) {
            return Err(S3Error::with_message(
                S3ErrorCode::BadDigest,
                "x-amz-checksum-sha256 is not the key's hash",
            ));
        }
        let mut bytes = Vec::new();
        if let Some(mut body) = input.body {
            while let Some(chunk) = body.next().await {
                let chunk = chunk.map_err(|error| {
                    S3Error::with_message(S3ErrorCode::IncompleteBody, error.to_string())
                })?;
                if bytes.len() + chunk.len() > MAX_OBJECT_BYTES {
                    return Err(S3Error::new(S3ErrorCode::EntityTooLarge));
                }
                bytes.extend_from_slice(&chunk);
            }
        }
        if Hash::of(&bytes) != hash {
            return Err(S3Error::with_message(
                S3ErrorCode::BadDigest,
                "the body is not the object its key names",
            ));
        }
        self.blocking(move |objects| {
            objects.put(&bytes)?;
            objects.sync()
        })
        .await?;
        Ok(S3Response::new(PutObjectOutput {
            e_tag: Some(tag(hash)),
            checksum_sha256: Some(sent),
            ..PutObjectOutput::default()
        }))
    }
}

impl crate::ServerStore {
    /// The credentials that sign requests to this server's S3 endpoint.
    pub fn s3_endpoint(&self) -> S3Endpoint {
        let hex =
            |bytes: &[u8]| -> String { bytes.iter().map(|byte| format!("{byte:02x}")).collect() };
        let key = self.server_key();
        S3Endpoint {
            access_key: hex(&key.derived("s3 access key")[..10]).to_ascii_uppercase(),
            secret_key: hex(&key.derived("s3 secret key")),
        }
    }
}
