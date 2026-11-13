//! Moving object bytes (guides/blocks/client.md, "Objects"): the server checks access and
//! answers presigned URLs, and the bytes go straight to and from the store behind them
//! (`crate::http`), a bucket or the server's own S3 endpoint.

use std::collections::HashSet;

use be_block_protocol::{ErrorCode, MAX_OBJECT_BYTES, MAX_OBJECT_URLS, ObjectLength, client};
use be_block_store::{Hash, ObjectStore};
use futures_util::StreamExt;

use super::Peer;
use crate::{ClientError, http};

/// Object requests a peer has in flight at once. Natively each takes a connection of its own.
const TRANSFERS: usize = 8;

impl<S: ObjectStore> Peer<S> {
    /// Fetches the objects among `hashes` this device lacks, and fails with `MissingObject` for
    /// one the server has no URL for or the store no bytes. Checks that every object hashes to
    /// the hash asked for: the server is not trusted with content, and this is what stops it
    /// substituting any.
    pub async fn fetch(&self, hashes: &[Hash]) -> Result<(), ClientError> {
        let store = self.commits.vault().store();
        let mut seen = HashSet::new();
        let mut missing = Vec::new();
        for hash in hashes {
            if seen.insert(*hash) && !store.has(*hash)? {
                missing.push(*hash);
            }
        }
        for batch in missing.chunks(MAX_OBJECT_URLS) {
            let urls = self.download_urls(batch).await?;
            let mut downloads =
                futures_util::stream::iter(batch.iter().zip(urls).map(|(hash, url)| async move {
                    let url = url.ok_or(ClientError::MissingObject(*hash))?;
                    download(*hash, &url)
                        .await?
                        .ok_or(ClientError::MissingObject(*hash))
                }))
                .buffer_unordered(TRANSFERS);
            while let Some(bytes) = downloads.next().await {
                store.put(&bytes?)?;
            }
        }
        Ok(())
    }

    /// Fetches one object unless this device has it; answers whether it has it now.
    pub async fn try_fetch(&self, hash: Hash) -> Result<bool, ClientError> {
        let store = self.commits.vault().store();
        if store.has(hash)? {
            return Ok(true);
        }
        let Some(Some(url)) = self.download_urls(&[hash]).await?.pop() else {
            return Ok(false);
        };
        let Some(bytes) = download(hash, &url).await? else {
            return Ok(false);
        };
        store.put(&bytes)?;
        Ok(true)
    }

    /// Uploads the objects among `hashes` the server lacks.
    pub async fn push(&self, hashes: &[Hash]) -> Result<(), ClientError> {
        self.commits.vault().store().sync()?;
        self.upload(hashes).await
    }

    /// Uploads `hashes` from this device's store, those the server answers it holds already
    /// left out (`upload-urls`). An object this device does not hold (a part it reuses by its
    /// ref alone) is asked for with length 0, so nothing is reserved for it: the server must
    /// hold it, and answering a URL for it means neither does, `MissingObject`.
    pub(super) async fn upload(&self, hashes: &[Hash]) -> Result<(), ClientError> {
        let store = self.commits.vault().store();
        for batch in hashes.chunks(MAX_OBJECT_URLS) {
            let lengths = batch
                .iter()
                .map(|hash| {
                    Ok(ObjectLength {
                        hash: *hash,
                        length: store.len(*hash)?.unwrap_or(0) as u64,
                    })
                })
                .collect::<Result<Vec<_>, ClientError>>()?;
            let urls =
                crate::ask(client::objects::upload_urls(&self.connection(), lengths)).await?;
            if urls.len() != batch.len() {
                return Err(ClientError::Unexpected);
            }
            let wanted = batch
                .iter()
                .zip(urls)
                .filter_map(|(hash, url)| Some((hash, url?)));
            let mut uploads = futures_util::stream::iter(wanted.map(|(hash, url)| async move {
                let bytes = store.get(*hash)?.ok_or(ClientError::MissingObject(*hash))?;
                let headers: Vec<(String, String)> = url
                    .headers
                    .into_iter()
                    .map(|header| (header.name, header.value))
                    .collect();
                http::put(&url.url, &headers, bytes)
                    .await
                    .map_err(transfer_failed)
            }))
            .buffer_unordered(TRANSFERS);
            while let Some(sent) = uploads.next().await {
                sent?;
            }
        }
        Ok(())
    }

    async fn download_urls(&self, hashes: &[Hash]) -> Result<Vec<Option<String>>, ClientError> {
        let urls = crate::ask(client::objects::download_urls(
            &self.connection(),
            hashes.to_vec(),
        ))
        .await?;
        match urls.len() == hashes.len() {
            true => Ok(urls),
            false => Err(ClientError::Unexpected),
        }
    }
}

/// The object `hash` from `url`, checked against its hash; None when the store does not have
/// it.
async fn download(hash: Hash, url: &str) -> Result<Option<Vec<u8>>, ClientError> {
    match http::get(url, MAX_OBJECT_BYTES).await {
        Ok(bytes) if Hash::of(&bytes) == hash => Ok(Some(bytes)),
        Ok(_) | Err(http::Error::TooLarge { .. }) => Err(ClientError::Tampered(hash)),
        Err(http::Error::NotFound) => Ok(None),
        Err(error) => Err(transfer_failed(error)),
    }
}

/// What a failed transfer is to the peer's callers: one that may succeed later is a lost
/// connection, which they retry or keep for later; a refusal is the store's.
fn transfer_failed(error: http::Error) -> ClientError {
    match error {
        http::Error::Failed(reason) => {
            ClientError::Disconnected(format!("moving an object failed: {reason}"))
        }
        error => ClientError::Refused(ErrorCode::Storage, error.to_string()),
    }
}
