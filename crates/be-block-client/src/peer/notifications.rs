//! Notifications: sealing one for a block's members who are not connected, opening one a push
//! carried with the keys this device holds, and the server's endpoints and modes.
//! guides/blocks/server.md, "Notifications".

use be_block_protocol::{MAX_NOTIFICATION_BYTES, NotifyMode, PushEndpoint, client};
use be_block_store::ObjectStore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Peer;
use crate::ClientError;

/// What a notification shows: the block's name, who sent it, and what they wrote. Sealed under
/// the block's data key, so only its members read it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub author: String,
    pub text: String,
}

/// Put before a notification's bytes before sealing, so no other bytes sealed under a block's
/// key open as one.
const DOMAIN: &[u8] = b"be3 notification\0";
/// The most of the text a notification carries; the rest is cut at a character.
pub const MAX_NOTIFICATION_TEXT: usize = 1024;
/// The most of the title or author a notification carries.
const MAX_NAME: usize = 200;

fn cut(text: &str, bytes: usize) -> String {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

impl Notification {
    fn encode(&self) -> Vec<u8> {
        let cut = Notification {
            title: cut(&self.title, MAX_NAME),
            author: cut(&self.author, MAX_NAME),
            text: cut(&self.text, MAX_NOTIFICATION_TEXT),
        };
        let mut bytes = DOMAIN.to_vec();
        bytes.extend(postcard::to_stdvec(&cut).expect("a notification encodes"));
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, ClientError> {
        let body = bytes
            .strip_prefix(DOMAIN)
            .ok_or_else(|| ClientError::Encoding("the bytes are not a notification".into()))?;
        postcard::from_bytes(body).map_err(|error| ClientError::Encoding(error.to_string()))
    }
}

impl<S: ObjectStore> Peer<S> {
    /// Seals `notification` under `block`'s data key and has the server push it to the block's
    /// members who are not connected; `mentions` names the accounts it mentions.
    pub async fn notify(
        &self,
        block: Uuid,
        notification: &Notification,
        mentions: Vec<Uuid>,
    ) -> Result<(), ClientError> {
        let sealed = self.seal_notification(block, notification).await?;
        client::notifications::notify(&self.connection(), block, sealed, mentions).await??;
        Ok(())
    }

    /// `notification` sealed under `block`'s newest data key, which names that key.
    pub async fn seal_notification(
        &self,
        block: Uuid,
        notification: &Notification,
    ) -> Result<Vec<u8>, ClientError> {
        let key = self.sealing_key(block).await?;
        let sealed = self.commits.vault().seal(key, &notification.encode())?;
        match sealed.len() <= MAX_NOTIFICATION_BYTES {
            true => Ok(sealed),
            false => Err(ClientError::Encoding("the notification is too long".into())),
        }
    }

    /// Opens a notification a push carried, with the keys this device already holds: no
    /// network. A device missing the key shows a notification naming nothing.
    pub fn open_notification(&self, sealed: &[u8]) -> Result<Notification, ClientError> {
        Notification::decode(&self.commits.vault().open(sealed)?)
    }

    pub async fn notify_mode(&self, block: Uuid) -> Result<NotifyMode, ClientError> {
        Ok(client::notifications::notify_mode_of(&self.connection(), block).await??)
    }

    pub async fn set_notify_mode(&self, block: Uuid, mode: NotifyMode) -> Result<(), ClientError> {
        client::notifications::set_notify_mode(&self.connection(), block, mode).await??;
        Ok(())
    }

    /// The server's VAPID public key, which a browser subscribes with.
    pub async fn push_key(&self) -> Result<Vec<u8>, ClientError> {
        Ok(client::notifications::push_key(&self.connection()).await??)
    }

    pub async fn register_endpoint(&self, endpoint: PushEndpoint) -> Result<(), ClientError> {
        client::notifications::register_endpoint(&self.connection(), endpoint).await??;
        Ok(())
    }

    pub async fn unregister_endpoint(&self, address: String) -> Result<(), ClientError> {
        client::notifications::unregister_endpoint(&self.connection(), address).await??;
        Ok(())
    }
}
