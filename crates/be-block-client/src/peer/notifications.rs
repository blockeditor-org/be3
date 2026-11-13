//! Notifications: sealing one for a block's members who are not connected, opening one a push
//! carried with the keys this device holds, and the server's endpoints and modes.
//! guides/blocks/server.md, "Notifications".

use be_block_commit::now_milliseconds;
use be_block_content::Announcement;
use be_block_protocol::{
    BlockNotifyMode, MAX_NOTIFICATION_BYTES, NotifyMode, PushEndpoint, PushMessage, client,
};
use be_block_store::ObjectStore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Peer, locked};
use crate::{
    ClientError,
    refs::{Announcing, Made, Until},
};

/// What a notification's sender wrote, sealed under the block's data key, with the
/// organization, the block, the sender's account and the time it was sent sealed in, so a
/// server that resends it later, under another block or organization, or as another member's
/// is found out (`Peer::open_notification`). The
/// server vouches for the sender too (`PushMessage::sender`), so a member cannot claim to be
/// another, nor the server without a member's help. The device names both itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub organization: Uuid,
    pub block: Uuid,
    pub sender: Uuid,
    /// Milliseconds since the epoch, by the sender's clock.
    pub sent: i64,
    pub text: String,
}

/// A notification a push carried, opened: who sent it and about which block, as the server
/// vouches, and what they wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedNotification {
    pub organization: Uuid,
    pub block: Uuid,
    pub sender: Uuid,
    pub sent: i64,
    pub text: String,
}

/// Put before a notification's bytes before sealing, so no other bytes sealed under a block's
/// key open as one.
const DOMAIN: &[u8] = b"be3 notification\0";
/// The most of the text a notification carries; the rest is cut at a character.
pub const MAX_NOTIFICATION_TEXT: usize = 1024;
/// How old a notification may be when it opens: a little past how long a push service keeps
/// one for a device that is away. Older, it is a replay.
pub const MAX_NOTIFICATION_AGE_MILLISECONDS: i64 = 2 * 24 * 60 * 60 * 1000;
/// How far ahead of this device's clock a sender's may be.
const CLOCK_SKEW_MILLISECONDS: i64 = 60 * 60 * 1000;

fn cut(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

impl Notification {
    fn encode(&self) -> Vec<u8> {
        let cut = Notification {
            text: cut(&self.text, MAX_NOTIFICATION_TEXT).to_owned(),
            ..self.clone()
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
    /// Seals `text` about `block` under its data key and has the server push it to the block's
    /// members who are not connected; `mentions` names the accounts it mentions, and `keys` what
    /// it tells, which the server tells once however often it is sent.
    pub async fn notify(
        &self,
        block: Uuid,
        text: &str,
        mentions: Vec<Uuid>,
        keys: Vec<be_block_store::Hash>,
    ) -> Result<(), ClientError> {
        let notification = Notification {
            organization: self.organization,
            block,
            sender: self.account,
            sent: now_milliseconds(),
            text: text.to_owned(),
        };
        let sealed = self.seal_notification(&notification).await?;
        client::notifications::notify(&self.connection(), block, sealed, mentions, keys).await??;
        Ok(())
    }

    /// Keeps announcements of edits of `block` the server already holds (a follower's, once the
    /// owner accepted them) for `send_announcements`; one whose key this device knows is not
    /// kept again.
    pub async fn announce_held(
        &self,
        block: Uuid,
        announcements: Vec<Announcement>,
    ) -> Result<(), ClientError> {
        let key = self.sealing_key(block).await?;
        let made = announcements
            .into_iter()
            .map(|announcement| self.made(key, announcement))
            .collect::<Result<Vec<_>, ClientError>>()?;
        Ok(self.refs.add_announcements(block, Until::Held, made)?)
    }

    /// An announcement as this device keeps it, its text sealed under `key`.
    pub(super) fn made(&self, key: Uuid, announcement: Announcement) -> Result<Made, ClientError> {
        Ok(Made {
            sealed: self
                .commits
                .vault()
                .seal(key, announcement.text.as_bytes())?,
            key: announcement.key,
            several: announcement.several,
            mentions: announcement.mentions,
        })
    }

    /// Sends what every edit the server now holds announces, once: one notification per block,
    /// the text of one alone or, for several (a drained outbox), the newest's `several` with
    /// `{count}` replaced by how many, mentioning everyone they mention. While the server cannot
    /// be reached they wait for the next call; one it refuses (past its rate) is dropped, since
    /// it is about now. Nothing is sent while the outbox drains, so a burst it publishes goes as
    /// one: the drain sends at its end.
    pub async fn send_announcements(&self) -> Result<(), ClientError> {
        if !self.is_attached()
            || self.draining.load(std::sync::atomic::Ordering::SeqCst) > 0
            || !self.refs.has_held_announcements()
        {
            return Ok(());
        }
        let taken = self.refs.take_held_announcements()?;
        let mut blocks: Vec<(Uuid, Vec<Announcing>)> = Vec::new();
        for announcing in taken {
            match blocks
                .iter_mut()
                .find(|(block, _)| *block == announcing.block)
            {
                Some((_, held)) => held.push(announcing),
                None => blocks.push((announcing.block, vec![announcing])),
            }
        }
        let mut failed = None;
        let mut blocks = blocks.into_iter();
        while let Some((block, held)) = blocks.next() {
            let text = match held.as_slice() {
                [one] => self
                    .commits
                    .vault()
                    .open(&one.made.sealed)
                    .map_err(locked)
                    .map(|text| String::from_utf8_lossy(&text).into_owned()),
                several => Ok(several
                    .last()
                    .map(|newest| {
                        newest
                            .made
                            .several
                            .replace("{count}", &several.len().to_string())
                    })
                    .unwrap_or_default()),
            };
            let mut mentions: Vec<Uuid> = Vec::new();
            for mention in held.iter().flat_map(|announcing| &announcing.made.mentions) {
                if !mentions.contains(mention) {
                    mentions.push(*mention);
                }
            }
            // The newest the server takes, oldest first: it remembers the last it is told.
            let keys = held[held.len().saturating_sub(be_block_protocol::MAX_TOLD_KEYS)..]
                .iter()
                .map(|announcing| be_block_protocol::told_key(block, &announcing.made.key))
                .collect();
            let sent = match text {
                Ok(text) => self.notify(block, &text, mentions, keys).await,
                Err(error) => Err(error),
            };
            match sent {
                Ok(()) | Err(ClientError::Refused(..)) => {}
                Err(error) if error.is_unreachable() => {
                    let unsent: Vec<Announcing> = held
                        .into_iter()
                        .chain(blocks.flat_map(|(_, held)| held))
                        .collect();
                    self.refs.unsent_announcements(&unsent)?;
                    return Err(error);
                }
                Err(error) => failed = Some(error),
            }
        }
        failed.map_or(Ok(()), Err)
    }

    /// `notification` sealed under its block's newest data key, which names that key.
    pub async fn seal_notification(
        &self,
        notification: &Notification,
    ) -> Result<Vec<u8>, ClientError> {
        let key = self.sealing_key(notification.block).await?;
        let sealed = self.commits.vault().seal(key, &notification.encode())?;
        match sealed.len() <= MAX_NOTIFICATION_BYTES {
            true => Ok(sealed),
            false => Err(ClientError::Encoding("the notification is too long".into())),
        }
    }

    /// Opens what a push carried with the keys this device already holds: no network. Refuses
    /// one sealed about another block or by another sender than the push names, or sent more
    /// than
    /// `MAX_NOTIFICATION_AGE_MILLISECONDS` before `now`, which a server resending it would be.
    /// A device that cannot open one shows a notification naming nothing.
    pub fn open_notification(
        &self,
        message: &PushMessage,
        now: i64,
    ) -> Result<OpenedNotification, ClientError> {
        let notification = Notification::decode(&self.commits.vault().open(&message.sealed)?)?;
        if notification.organization != message.organization {
            return Err(ClientError::Encoding(
                "the notification was sealed about another organization".into(),
            ));
        }
        if notification.block != message.block {
            return Err(ClientError::Encoding(
                "the notification was sealed about another block".into(),
            ));
        }
        if notification.sender != message.sender {
            return Err(ClientError::Encoding(
                "the notification was sealed by another member than the server names".into(),
            ));
        }
        let age = now.saturating_sub(notification.sent);
        if !(-CLOCK_SKEW_MILLISECONDS..=MAX_NOTIFICATION_AGE_MILLISECONDS).contains(&age) {
            return Err(ClientError::Encoding(
                "the notification is too old to be the one sent".into(),
            ));
        }
        Ok(OpenedNotification {
            organization: message.organization,
            block: message.block,
            sender: message.sender,
            sent: notification.sent,
            text: notification.text,
        })
    }

    /// The blocks whose mode the account set to other than `all`.
    pub async fn notify_modes(&self) -> Result<Vec<BlockNotifyMode>, ClientError> {
        Ok(client::notifications::notify_modes(&self.connection()).await??)
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
