//! The two sides of a hosted block's private channel (plans/hosted_blocks.md, "The channel"):
//! a member checks that the account the server names as the block's host may host it, then
//! opens a Noise IK handshake to that account's identity key (`be_block_keys::noise`); the host
//! checks the member's grant names the key it spoke with before it answers. The messages travel
//! on the session's host channel (`Connection::send_host`, `Event::Hosted`), which the server
//! routes but cannot read.

use be_block_keys::noise::{Channel, Initiator, Responder, hosted_prologue};
use be_block_keys::{Level, Record, Step};
use be_block_protocol::ErrorCode;
use be_block_store::ObjectStore;
use uuid::Uuid;

use super::{Around, Peer, cites};
use crate::ClientError;

/// A member's side of the handshake, its first message sent: it waits for the host's answer
/// (`finish`).
pub struct Joining {
    block: Uuid,
    host: Uuid,
    initiator: Initiator,
}

impl Joining {
    /// The account it speaks to: the host the logs checked.
    pub fn host(&self) -> Uuid {
        self.host
    }

    /// Reads the host's answer: the channel, once it was made by the holder of the key the
    /// block's log names for the host. Anything else (another key, a message replayed from
    /// another handshake) is refused as forged.
    pub fn finish(self, answer: &[u8]) -> Result<Channel, ClientError> {
        self.initiator
            .finish(answer)
            .map(|(channel, _)| channel)
            .map_err(|_| ClientError::Forged(self.block))
    }
}

/// A member the host admitted: its account, its level in the block's log as the host checked
/// it, and the channel to it.
#[derive(Debug)]
pub struct Admitted {
    pub account: Uuid,
    pub level: Level,
    pub channel: Channel,
}

/// The head a first message carries in the clear before the handshake, which the prologue
/// binds: the host reads its log as of it.
const HEAD_LEN: usize = 32;

impl<S: ObjectStore> Peer<S> {
    /// Starts the handshake to `host`, the account the server names as holding `block`'s lease
    /// (`SessionHost`): checks `host` holds Edit or above in the block's access log, and opens
    /// to the identity key that log names for it. Answers the first message to send the host
    /// (`Connection::send_host`). The log is checked from the root this device pinned or, for a
    /// Join member, which holds no key that carries one, from the root a grant to this account
    /// fixes, signed by a contact it verified (`joined_log`). A host the log does not let host,
    /// or a log that does not check, is refused as forged: the server cannot stand in a host of
    /// its own, nor pass the lease to an account without Edit.
    pub async fn join_host(&self, block: Uuid, host: Uuid) -> Result<(Joining, Vec<u8>), ClientError> {
        let checked = self.joined_log(block).await?;
        let identity = match checked.grants.get(&host.into_bytes()) {
            Some((level, identity)) if *level >= Level::Edit => *identity,
            _ => return Err(ClientError::Forged(block)),
        };
        let prologue = hosted_prologue(block.into_bytes(), checked.head);
        let (initiator, message) = Initiator::start(
            self.keys.identity(),
            &identity.public,
            &prologue,
            self.account.as_bytes(),
        );
        let mut first = checked.head.to_vec();
        first.extend(message);
        Ok((
            Joining {
                block,
                host,
                initiator,
            },
            first,
        ))
    }

    /// The host's side: reads a member's first message, admits it if the block's whole log
    /// (which this device, holding Edit, is served) grants the account it names Join or above now,
    /// with the identity key it spoke with, and answers with the reply to send it back. A member
    /// revoked since, one speaking with a key its grant does not name, or one that checked a log
    /// this device's does not reach, is refused.
    pub async fn admit(
        &self,
        block: Uuid,
        message: &[u8],
    ) -> Result<(Admitted, Vec<u8>), ClientError> {
        let refused = |why: &str| {
            ClientError::Refused(
                ErrorCode::PermissionDenied,
                format!("this host does not admit that member to block {block}: {why}"),
            )
        };
        let (head, message) = message
            .split_at_checked(HEAD_LEN)
            .ok_or_else(|| refused("the message is cut short"))?;
        let head: [u8; HEAD_LEN] = head.try_into().map_err(|_| refused("no head"))?;
        let (_, checked) = self.checked_access(block).await?;
        if checked.partial {
            return Err(refused("this device is not served the block's whole log"));
        }
        if !checked.holds(&head) {
            return Err(refused("it checked a log this host's does not reach"));
        }
        let prologue = hosted_prologue(block.into_bytes(), head);
        let responder = Responder::read(self.keys.identity(), &prologue, message)
            .map_err(|_| refused("the handshake does not open"))?;
        let account: [u8; 16] = responder
            .payload
            .as_slice()
            .try_into()
            .map_err(|_| refused("it names no account"))?;
        let level = match checked.grants.get(&account) {
            Some((level, identity))
                if *level >= Level::Join && identity.public == responder.joiner =>
            {
                *level
            }
            _ => return Err(refused("the log grants it nothing with that key")),
        };
        let (channel, answer) = responder.accept(&[]);
        Ok((
            Admitted {
                account: Uuid::from_bytes(account),
                level,
                channel,
            },
            answer,
        ))
    }

    /// `block`'s log as a member checks a host by it: from the root this device pinned, as
    /// `checked_access` checks it; or, with none pinned (a Join member is sealed no key that
    /// would carry one), from the root of the log as served, taken only when that log holds a
    /// grant to this account signed with the key of a contact this device verified, whose hash
    /// chain fixes every entry before it, the root included. Such a log is not kept: it is
    /// read again for each handshake.
    async fn joined_log(&self, block: Uuid) -> Result<be_block_keys::Checked, ClientError> {
        if !matches!(self.keys.root_pin(block), crate::keys::RootPin::Unknown) {
            return Ok(self.checked_access(block).await?.1);
        }
        let entries = self.served_log(block).await?;
        let root = entries.first().ok_or(ClientError::Forged(block))?.hash();
        let around = match cites(&entries) {
            true => self.logs_around(block, &entries).await?,
            false => Around::default(),
        };
        let checked = around
            .check(self.organization, block, root, &entries)
            .ok_or(ClientError::Forged(block))?;
        let me = self.account.into_bytes();
        let vouched = entries.iter().filter_map(Step::entry).any(|entry| {
            matches!(&entry.record, Record::Grant(grant) if grant.grantee == me)
                && self
                    .verified_identity(Uuid::from_bytes(entry.signer))
                    .is_some_and(|signer| entry.signed_by(&signer.signing))
        });
        match vouched {
            true => Ok(checked),
            false => Err(ClientError::Forged(block)),
        }
    }
}
