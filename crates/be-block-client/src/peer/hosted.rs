//! The two sides of a hosted block's private channel (plans/hosted_blocks.md, "The channel"):
//! a member checks that the account the server names as the block's host may host it, then
//! opens a Noise IK handshake to that account's identity key (`be_block_keys::noise`); the host
//! checks the member's grant names the key it spoke with before it answers, and counts the
//! member as admitted only once the member's first message on the channel opens (`Admitting`),
//! since the handshake's first message can be replayed. The messages travel on the session's
//! host channel (`Connection::send_host`, `Event::Hosted`), which the server routes but cannot
//! read.

use be_block_keys::noise::{Channel, Initiator, Responder, hosted_prologue};
use be_block_keys::{Checked, Level, LogKind, Placement, PublicIdentity, Record, Step};
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
    /// block's log names for the host. Anything else (another key, an answer to another
    /// handshake) is refused as forged. The member's first message on the channel is what
    /// shows the host it is there (`Admitting::confirm`), so it sends one at once.
    pub fn finish(self, answer: &[u8]) -> Result<Channel, ClientError> {
        self.initiator
            .finish(answer)
            .map(|(channel, _)| channel)
            .map_err(|_| ClientError::Forged(self.block))
    }
}

/// A member the host answered, not admitted yet: the handshake's first message can be replayed,
/// so it is admitted only once its first message on the channel opens (`confirm`).
#[derive(Debug)]
pub struct Admitting {
    block: Uuid,
    account: Uuid,
    level: Level,
    public: be_block_keys::RecoveryPublic,
    channel: Channel,
}

impl Admitting {
    /// The account the member says it is, which its first message on the channel proves.
    pub fn account(&self) -> Uuid {
        self.account
    }

    /// The level the signed logs give that account, as the host checked them.
    pub fn level(&self) -> Level {
        self.level
    }

    /// The identity key it spoke with.
    pub fn public(&self) -> be_block_keys::RecoveryPublic {
        self.public
    }

    /// Opens the member's first message on the channel: the member admitted, and what it said.
    /// One that does not open (a replayed handshake, whose sender holds no key of this one) is
    /// refused, and the member is not admitted.
    pub fn confirm(mut self, first: &[u8]) -> Result<(Admitted, Vec<u8>), ClientError> {
        let said = self.channel.open(first).map_err(|_| {
            ClientError::Refused(
                ErrorCode::PermissionDenied,
                format!(
                    "the member of block {} that answered holds no key of this handshake",
                    self.block
                ),
            )
        })?;
        Ok((
            Admitted {
                account: self.account,
                level: self.level,
                public: self.public,
                channel: self.channel,
            },
            said,
        ))
    }
}

/// A member the host admitted: its account, its level as the host checked it in the signed
/// logs, and the channel to it.
#[derive(Debug)]
pub struct Admitted {
    pub account: Uuid,
    pub level: Level,
    /// The identity key it spoke with, which `Peer::still_admits` checks the logs name still.
    pub public: be_block_keys::RecoveryPublic,
    pub channel: Channel,
}

/// The head a first message carries in the clear before the handshake, which the prologue
/// binds: the host reads its log as of it.
const HEAD_LEN: usize = 32;

impl<S: ObjectStore> Peer<S> {
    /// Starts the handshake to `host`, the account the server names as holding `block`'s lease
    /// (`SessionHost`): checks `host` holds Edit or above in the signed logs (the block's, a
    /// block above it as its parent records place it, or administering the organization:
    /// `held_with_key`), and opens to the identity key the record giving it names. Answers the first
    /// message to send the host (`Connection::send_host`). The block's log is checked from the
    /// root this device pinned or, for a Join member, which holds no key that carries one, from
    /// the root a grant to this account fixes, signed by a contact it verified (`joined_log`). A
    /// host the logs do not let host, or a log that does not check, is refused as forged: the
    /// server cannot stand in a host of its own, nor pass the lease to an account without Edit.
    pub async fn join_host(
        &self,
        block: Uuid,
        host: Uuid,
    ) -> Result<(Joining, Vec<u8>), ClientError> {
        let checked = self.joined_log(block).await?;
        let identity = match self
            .held_with_key(&checked, host.into_bytes(), Level::Edit)
            .await?
        {
            Some((_, identity)) => identity,
            None => return Err(ClientError::Forged(block)),
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

    /// The host's side: reads a member's first message and answers it if the signed logs give
    /// the account it names Join on the block itself, or View or above from anywhere up its
    /// path (View holds Join), now, with the identity key it spoke with (`held_with_key`). The block's
    /// log, which this device (holding Edit) is served whole, must reach the head the member
    /// checked. Answers the reply to send back, and the member as `Admitting`: admitted only
    /// once its first message on the channel opens. A member revoked since, or one speaking
    /// with a key no record names for the account it names, is refused.
    pub async fn admit(
        &self,
        block: Uuid,
        message: &[u8],
    ) -> Result<(Admitting, Vec<u8>), ClientError> {
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
        let level = match self.member_held(&checked, account).await? {
            Some((level, identity)) if identity.public == responder.joiner => level,
            _ => {
                return Err(refused(
                    "the logs grant it nothing with that key, or bar it",
                ));
            }
        };
        let public = responder.joiner;
        let (channel, answer) = responder.accept(&[]);
        Ok((
            Admitting {
                block,
                account: Uuid::from_bytes(account),
                level,
                public,
                channel,
            },
            answer,
        ))
    }

    /// Whether a member the host admitted to `block`'s channel as `account`, speaking with the
    /// identity key `public`, may stay: the level the signed logs give it now, as `admit`
    /// checks it, or none once a revocation, a ban or a new key ends it. What a host checks its
    /// admitted members by again on each access change (plans/hosted_blocks.md, "Hosting").
    pub async fn still_admits(
        &self,
        block: Uuid,
        account: Uuid,
        public: &be_block_keys::RecoveryPublic,
    ) -> Result<Option<Level>, ClientError> {
        let (_, checked) = self.checked_access(block).await?;
        Ok(self
            .member_held(&checked, account.into_bytes())
            .await?
            .filter(|(_, identity)| identity.public == *public)
            .map(|(level, _)| level))
    }

    /// The level the signed logs give `account` as a member of the host channel of the block
    /// `checked` is the log of, with the key the record giving it names: Join on the block
    /// itself, or View or above from anywhere up its path (`held_with_key`); none for one below
    /// Edit the log bans, since a ban binds no editor.
    async fn member_held(
        &self,
        checked: &Checked,
        account: [u8; 16],
    ) -> Result<Option<(Level, PublicIdentity)>, ClientError> {
        let held = match checked.grants.get(&account) {
            Some((level, identity)) if *level >= Level::Join => Some((*level, *identity)),
            _ => self.held_with_key(checked, account, Level::View).await?,
        };
        Ok(held.filter(|(level, _)| *level >= Level::Edit || !checked.banned.contains(&account)))
    }

    /// Bars `account` from joining `block`'s host (plans/hosted_blocks.md, "Hosting"): signs a
    /// ban record into the block's log, which this account may do with Edit or above (from the
    /// log or from outside it). A host refuses the account at the handshake and the server at
    /// joining, until a grant to it signed after the ban lifts it. A ban binds no editor.
    pub async fn ban(&self, block: Uuid, account: Uuid) -> Result<(), ClientError> {
        use be_block_keys::{Authority, Ban};
        let (_, checked) = self.checked_access(block).await?;
        let me = self.account.into_bytes();
        let authority = match checked.holding(LogKind::Block, me, Level::Edit) {
            Some(_) => Authority::Log,
            None => self.authority_outside(block, &checked, Level::Edit).await?,
        };
        let signed = checked
            .next(
                self.keys.identity(),
                me,
                block.into_bytes(),
                Record::Ban(Ban {
                    account: account.into_bytes(),
                    authority,
                }),
            )
            .ok_or_else(|| {
                ClientError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("this account is not served the whole log of block {block}"),
                )
            })?
            .encode();
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization),
            be_block_protocol::Op::AppendLog(be_block_protocol::LogRecord::Ban {
                block,
                account,
                signed,
            }),
        )
        .await??;
        Ok(())
    }

    /// The level `account` holds at or above `level` on the block `checked` is the log of, as
    /// the signed logs give it now, with the identity key the record giving it names: its record
    /// in that log; else, as `held_above` finds it, the nearest block above, by the signed
    /// parent records; else administering the organization, by its log checked from the root
    /// this account pinned. None when nothing gives it.
    async fn held_with_key(
        &self,
        checked: &Checked,
        account: [u8; 16],
        level: Level,
    ) -> Result<Option<(Level, PublicIdentity)>, ClientError> {
        let named = |log: &Checked| match log.grants.get(&account) {
            Some((held, identity)) if *held >= level => Some((*held, *identity)),
            _ => None,
        };
        if let Some(held) = named(checked) {
            return Ok(Some(held));
        }
        let mut placement = checked.placement();
        for _ in 0..be_block_keys::MAX_ANCESTRY {
            let Some(Placement::Block { block: up, root }) = placement else {
                break;
            };
            let above = match self.checked_from(Uuid::from_bytes(up), root).await {
                Ok(above) => above,
                Err(error) if error.is_unreachable() => return Err(error),
                Err(_) => break,
            };
            if above.holding(LogKind::Block, account, level).is_some()
                && let Some(held) = named(&above)
            {
                return Ok(Some(held));
            }
            placement = above.placement();
        }
        let Some(pinned) = self.keys.organization_root() else {
            return Ok(None);
        };
        let entries = match self.organization_log().await {
            Ok(entries) => entries,
            Err(error) if error.is_unreachable() => return Err(error),
            Err(_) => return Ok(None),
        };
        let Ok(organization) = be_block_keys::check_log(
            LogKind::Organization,
            self.organization.into_bytes(),
            pinned,
            &entries,
            &|_| None,
        ) else {
            return Ok(None);
        };
        Ok(organization
            .administrators
            .get(&account)
            .filter(|_| {
                organization
                    .holding(LogKind::Organization, account, level)
                    .is_some()
            })
            .map(|identity| (Level::Manage, *identity)))
    }

    /// `block`'s log as a member checks a host by it: from the root this device pinned, as
    /// `checked_access` checks it; or, with none pinned (a Join member is sealed no key that
    /// would carry one), from the root of the log as served, taken only when that log holds a
    /// grant to this account signed with a key this device verified for the contact that
    /// signed it, whose hash chain fixes every entry before it, the root included; or when the
    /// root is the one a share link this account opened handed it (`KeptShare::root`), which
    /// the link's maker pinned. That root
    /// is then pinned as weakly as a member's escrow wrap pins one (`KeyBook::pin_vouched`), so
    /// a log rooted elsewhere later is refused for the rest of the run.
    async fn joined_log(&self, block: Uuid) -> Result<Checked, ClientError> {
        let held = match self.vouched_log(block).await {
            // The server shows the block no more: this account holds nothing there.
            Err(
                refused @ ClientError::Refused(
                    ErrorCode::BlockNotFound | ErrorCode::PermissionDenied,
                    _,
                ),
            ) => {
                self.forget_fallen_opens(block, None).await?;
                return Err(refused);
            }
            held => held?,
        };
        self.forget_fallen_opens(block, Some(&held)).await?;
        Ok(held)
    }

    /// `block`'s log, checked from a root this device pinned, or that a contact's grant or a
    /// share link this account opened vouches for (`joined_log`).
    async fn vouched_log(&self, block: Uuid) -> Result<Checked, ClientError> {
        if !matches!(self.keys.root_pin(block), crate::keys::RootPin::Unknown) {
            return Ok(self.checked_access(block).await?.1);
        }
        let entries = self.served_log(block).await?;
        let root = entries.first().ok_or(ClientError::Forged(block))?.hash();
        // The root a share link this account opened handed it, which its maker pinned: the log
        // must start there, whatever the server serves.
        let linked = self
            .kept_shares()
            .await
            .unwrap_or_default()
            .into_iter()
            .any(|(_, kept)| kept.block == block && !kept.made && kept.root == root);
        let around = match cites(&entries) {
            true => self.logs_around(block, &entries).await?,
            false => Around::default(),
        };
        around
            .check(self.organization, block, root, None, &entries)
            .ok_or(ClientError::Forged(block))?;
        let me = self.account.into_bytes();
        let vouched = linked
            || entries.iter().filter_map(Step::entry).any(|entry| {
                matches!(&entry.record, Record::Grant(grant) if grant.grantee == me)
                    && self
                        .refs
                        .signers(Uuid::from_bytes(entry.signer))
                        .iter()
                        .filter(|signer| signer.verified)
                        .any(|signer| entry.signed_by(&be_block_keys::SigningPublic(signer.key)))
            });
        if !vouched {
            return Err(ClientError::Forged(block));
        }
        self.keys.pin_vouched(block, root);
        Ok(self.checked_access(block).await?.1)
    }

    /// Forgets the share links this account opened on `block` once it holds nothing there: the
    /// log, checked, grants it nothing, or the server shows the block no more (`checked` none).
    /// The grant a link gave fell (revoked, or the link with it), so the root it handed vouches
    /// for nothing more.
    async fn forget_fallen_opens(
        &self,
        block: Uuid,
        checked: Option<&Checked>,
    ) -> Result<(), ClientError> {
        if checked.is_some_and(|checked| {
            checked
                .holding(LogKind::Block, self.account.into_bytes(), Level::Join)
                .is_some()
        }) {
            return Ok(());
        }
        for (link, kept) in self.kept_shares().await? {
            if kept.block == block && !kept.made {
                self.keep_share(link, None).await?;
            }
        }
        Ok(())
    }
}
