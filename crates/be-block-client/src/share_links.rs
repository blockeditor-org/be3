//! Share links (guides/blocks/keys.md, "Share links"): "anyone with the link" on a block. A
//! manager makes one with a code it keeps in the account's block; whoever opens it signs its own
//! grant with the key the code derives, as the link's record in the block's log allows, and takes
//! the block's scope key the link carries, sealed with the code.

use std::sync::Arc;

use be_block_graph::Access;
use be_block_keys::{
    Admits, Authority, CarriedKey, Entry, EntryHash, InviteCode, Level, LogKind, Party, Record,
    RecoverySecret, ShareLink,
};
use be_block_protocol::{ErrorCode, LinkAdmits, ShareLinkRow};
use be_block_store::ObjectStore;
use uuid::Uuid;

use crate::{AccountRecords, ClientError, Connection, Peer, contacts};

/// What a share link hands its opener, sealed with its code (`InviteCode::seal_share`): the
/// block's scope key as it travels, with the block and its log's root, and the root of the
/// organization's log, so a guest checks records an administrator signed. The server stores it
/// and never opens it.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct Handed {
    pub key: Uuid,
    #[serde(with = "serde_bytes")]
    pub scope: Vec<u8>,
    pub organization_root: Option<EntryHash>,
}

impl Handed {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("what a share link hands always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }

    /// What `link` hands, opened with `code`: refused when it does not open, or carries a key of
    /// another block than the one the server names.
    pub fn open(code: &InviteCode, link: &be_block_protocol::ShareLink) -> Result<Self, ClientError> {
        let handed = code
            .open_share(link.block.into_bytes(), &link.sealed)
            .ok()
            .and_then(|bytes| Self::decode(&bytes))
            .ok_or_else(tampered)?;
        let carried = CarriedKey::decode(&handed.scope).map_err(|_| tampered())?;
        if carried.block != link.block.into_bytes() {
            return Err(tampered());
        }
        Ok(handed)
    }
}

/// A share link this account made, as its block keeps it until the link is revoked.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct KeptShare {
    pub code: Vec<u8>,
    pub organization: Uuid,
    pub block: Uuid,
}

impl KeptShare {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a kept share link always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }

    pub fn code(&self) -> Option<InviteCode> {
        InviteCode::from_bytes(&self.code).ok()
    }
}

fn tampered() -> ClientError {
    ClientError::Encoding(
        "the share link did not open with its code, so it was not the one its maker sent"
            .to_owned(),
    )
}

/// The id a share link with `code` is found by.
pub fn link_id(code: &InviteCode) -> Uuid {
    Uuid::from_bytes(code.lookup())
}

pub fn admits_of(admits: LinkAdmits) -> Admits {
    match admits {
        LinkAdmits::Members => Admits::Members,
        LinkAdmits::Anyone => Admits::Anyone,
    }
}

pub fn wire_admits(admits: Admits) -> LinkAdmits {
    match admits {
        Admits::Members => LinkAdmits::Members,
        Admits::Anyone => LinkAdmits::Anyone,
    }
}

fn level_of(access: Access) -> Result<Level, ClientError> {
    match access {
        Access::View => Ok(Level::View),
        Access::Edit => Ok(Level::Edit),
        _ => Err(ClientError::Refused(
            ErrorCode::InvalidRequest,
            "a share link grants viewing or editing".into(),
        )),
    }
}

/// The share link with `code`, as this account reads it to open it.
pub async fn read_share_link(
    connection: &Arc<Connection>,
    code: &InviteCode,
) -> Result<be_block_protocol::ShareLink, ClientError> {
    Ok(be_block_protocol::read::<be_block_protocol::ShareLink, _>(
        connection,
        None,
        be_block_protocol::Query::ShareLink(link_id(code)),
    )
    .await??)
}

/// How often `open_share_link` signs its grant again after the block's log moved under it.
const OPEN_TRIES: usize = 3;

/// Opens the share link with `code` as `account`: takes the scope key it carries, sealing it to
/// this account, signs this account's grant with the key the code derives after the head of the
/// block's log, and, for a guest of the organization, pins the root of its log the link carries
/// when this account pinned none. Answers the link.
pub async fn open_share_link(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    account: Uuid,
    code: &InviteCode,
) -> Result<be_block_protocol::ShareLink, ClientError> {
    // The record names these keys; the server holds them first, so it checks.
    contacts::register_identity(connection, identity).await?;
    let mut link = read_share_link(connection, code).await?;
    let handed = Handed::open(code, &link)?;
    let level = level_of(link.access)?;
    let mine = Party {
        account: account.into_bytes(),
        public: identity.public(),
    };
    let sealed = identity.seal_for(account.into_bytes(), &mine, handed.key.as_bytes(), &handed.scope);
    if link.guest
        && let Some(root) = handed.organization_root
        && contacts::pinned(connection, account, identity, link.organization)
            .await?
            .is_none_or(|held| held.root.is_none())
    {
        contacts::keep_pin(
            connection,
            account,
            identity,
            link.organization,
            &contacts::Pinned {
                root: Some(root),
                escrow: None,
                made: 0,
            },
        )
        .await?;
    }
    let mut tries = 0;
    loop {
        let signed = Entry::opened(
            code,
            link.block.into_bytes(),
            link.head,
            account.into_bytes(),
            identity,
            level,
        )
        .encode();
        let written = be_block_protocol::write::<(), _>(
            connection,
            None,
            be_block_protocol::Op::OpenShareLink {
                link: link.id,
                signed,
                key: handed.key,
                sealed: sealed.clone(),
            },
        )
        .await?;
        let Err(error) = written else {
            return Ok(link);
        };
        tries += 1;
        if tries >= OPEN_TRIES {
            return Err(error.into());
        }
        let again = read_share_link(connection, code).await?;
        if again.head == link.head {
            return Err(error.into());
        }
        link = again;
    }
}

impl<S: ObjectStore> Peer<S> {
    /// Makes a share link on `block` granting `access` (View or Edit) to whoever opens it, as
    /// `admits` allows: makes the block a scope of its own, keeps the link's code in the
    /// account's block, and opens the link in the block's log. Answers the code; the link is
    /// `{server}/#share/{CODE}`.
    pub async fn make_share_link(
        &self,
        block: Uuid,
        access: Access,
        admits: Admits,
    ) -> Result<InviteCode, ClientError> {
        let level = level_of(access)?;
        let offer = self.offer(block, access).await?;
        let code = InviteCode::generate();
        let link = link_id(&code);
        let handed = Handed {
            key: offer.key,
            scope: offer.secret,
            organization_root: self.keys().organization_root(),
        };
        let sealed = code.seal_share(block.into_bytes(), &handed.encode());
        let key = code.share_key(block.into_bytes());
        let signed = self
            .signed_as_manager(block, |authority| {
                Record::ShareLink(ShareLink {
                    key,
                    level,
                    admits,
                    open: true,
                    authority,
                })
            })
            .await?;
        // Kept before the server holds it, so its code is never lost.
        self.keep_share(
            link,
            Some(KeptShare {
                code: code.to_bytes().to_vec(),
                organization: self.organization(),
                block,
            }),
        )
        .await?;
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization()),
            be_block_protocol::Op::CreateShareLink {
                link,
                block,
                sealed,
                signed,
            },
        )
        .await??;
        Ok(code)
    }

    /// The share links open on `block`, for a manager of it, each with its code when this
    /// account made it and its block keeps it.
    pub async fn share_links(
        &self,
        block: Uuid,
    ) -> Result<Vec<(ShareLinkRow, Option<InviteCode>)>, ClientError> {
        let rows = be_block_protocol::read::<Vec<ShareLinkRow>, _>(
            &self.connection(),
            Some(self.organization()),
            be_block_protocol::Query::ShareLinks(block),
        )
        .await??;
        let kept = self.kept_shares().await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let code = kept
                    .iter()
                    .find(|(link, _)| *link == row.id)
                    .and_then(|(_, kept)| kept.code());
                (row, code)
            })
            .collect())
    }

    /// Revokes the share link `row` on `block`: every grant made through it falls, and the block
    /// rotates as for any lost access.
    pub async fn revoke_share_link(
        &self,
        block: Uuid,
        row: &ShareLinkRow,
    ) -> Result<(), ClientError> {
        let signed = self
            .signed_as_manager(block, |authority| {
                Record::ShareLink(ShareLink {
                    key: be_block_keys::SigningPublic(row.key),
                    level: level_of(row.access).unwrap_or(Level::View),
                    admits: admits_of(row.admits),
                    open: false,
                    authority,
                })
            })
            .await?;
        be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization()),
            be_block_protocol::Op::RevokeShareLink {
                link: row.id,
                signed,
            },
        )
        .await??;
        self.keep_share(row.id, None).await
    }

    /// `make`'s record on `block`, signed by this account as a manager of it at the head of its
    /// log: on Manage from the log, or from the organization or a block above.
    async fn signed_as_manager(
        &self,
        block: Uuid,
        make: impl FnOnce(Authority) -> Record,
    ) -> Result<Vec<u8>, ClientError> {
        let (_, checked) = self.checked_access(block).await?;
        let me = self.keys().account().into_bytes();
        let authority = match checked.manager(LogKind::Block, me).is_some() {
            true => Authority::Log,
            false => {
                self.authority_outside(block, &checked, Level::Manage)
                    .await?
            }
        };
        checked
            .next(self.keys().identity(), me, block.into_bytes(), make(authority))
            .map(|entry| entry.encode())
            .ok_or_else(|| {
                ClientError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("this account is not served the whole log of block {block}"),
                )
            })
    }

    async fn records(&self) -> Result<Arc<AccountRecords>, ClientError> {
        AccountRecords::open(&self.connection(), self.keys().account(), self.keys().identity())
            .await?
            .ok_or_else(|| {
                ClientError::Refused(
                    ErrorCode::PermissionDenied,
                    "an account in no personal organization makes no share links".into(),
                )
            })
    }

    async fn keep_share(&self, link: Uuid, kept: Option<KeptShare>) -> Result<(), ClientError> {
        let bytes = kept.map(|kept| kept.encode());
        self.records()
            .await?
            .update(move |current| match (current.share(link), &bytes) {
                (None, None) => Vec::new(),
                _ => vec![be_block::AccountBlock::set_share(link, bytes.clone())],
            })
            .await
    }

    async fn kept_shares(&self) -> Result<Vec<(Uuid, KeptShare)>, ClientError> {
        let records = self.records().await?;
        Ok(match records.read().await {
            Ok(held) => held
                .content
                .root()
                .shares
                .iter()
                .filter_map(|(link, bytes)| Some((*link, KeptShare::decode(bytes)?)))
                .collect(),
            Err(ClientError::Locked(_)) => Vec::new(),
            Err(error) => return Err(error),
        })
    }
}
