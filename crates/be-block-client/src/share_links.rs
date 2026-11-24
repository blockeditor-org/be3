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
/// block's scope key as it travels, with the block and its log's root (none for a Join link,
/// whose opener reads nothing stored); the hash of the block's log root, which its maker
/// pinned, for every link, so an opener checks the log by it whatever the server serves; the
/// root of the organization's log, so a guest checks records an administrator signed; and the
/// block's name as its maker saw it, which the opener is shown before opening it. The server
/// stores it and never opens it.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct Handed {
    pub scope: Option<HandedKey>,
    pub block_root: EntryHash,
    pub organization_root: Option<EntryHash>,
    pub name: String,
}

/// The block's scope key a share link carries: its id and the key as it travels.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct HandedKey {
    pub key: Uuid,
    #[serde(with = "serde_bytes")]
    pub carried: Vec<u8>,
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
    pub fn open(
        code: &InviteCode,
        link: &be_block_protocol::ShareLink,
    ) -> Result<Self, ClientError> {
        let handed = code
            .open_share(link.block.into_bytes(), &link.sealed)
            .ok()
            .and_then(|bytes| Self::decode(&bytes))
            .ok_or_else(tampered)?;
        // A Join link carries no key, and any other the block's own.
        match (&handed.scope, link.access) {
            (None, Access::Join) => {}
            (Some(scope), access) if access != Access::Join => {
                let carried = CarriedKey::decode(&scope.carried).map_err(|_| tampered())?;
                if carried.block != link.block.into_bytes() {
                    return Err(tampered());
                }
            }
            _ => return Err(tampered()),
        }
        Ok(handed)
    }
}

/// A share link as the account's block keeps it: one this account made, until it is revoked,
/// and one it opened, with the root of the block's log the link handed it, by which it checks
/// that log while it holds no key that carries a root (a Join member, `Peer::joined_log`).
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct KeptShare {
    pub code: Vec<u8>,
    pub organization: Uuid,
    pub block: Uuid,
    pub root: EntryHash,
    /// Whether this account made it, rather than opened it.
    pub made: bool,
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
        LinkAdmits::One => Admits::One,
    }
}

pub fn wire_admits(admits: Admits) -> LinkAdmits {
    match admits {
        Admits::Members => LinkAdmits::Members,
        Admits::Anyone => LinkAdmits::Anyone,
        Admits::One => LinkAdmits::One,
    }
}

fn level_of(access: Access) -> Result<Level, ClientError> {
    match access {
        Access::Join => Ok(Level::Join),
        Access::View => Ok(Level::View),
        Access::Edit => Ok(Level::Edit),
        _ => Err(ClientError::Refused(
            ErrorCode::InvalidRequest,
            "a share link grants joining, viewing or editing".into(),
        )),
    }
}

/// The share link with `code`, as this account reads it to open it. Whether opening it makes
/// this account a guest is judged here, by whether the organization is among this account's
/// own, not by the server's word.
pub async fn read_share_link(
    connection: &Arc<Connection>,
    code: &InviteCode,
) -> Result<be_block_protocol::ShareLink, ClientError> {
    let mut link = served_link(connection, code).await?;
    let organizations = be_block_protocol::read::<Vec<be_block_protocol::Organization>, _>(
        connection,
        None,
        be_block_protocol::Query::Organizations,
    )
    .await??;
    link.guest = !organizations.iter().any(|organization| {
        organization.id == link.organization
            && organization.role != be_block_protocol::OrganizationRole::Guest
    });
    Ok(link)
}

/// The share link with `code` as the server serves it, its `guest` the server's word.
async fn served_link(
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

/// The share link with `code` as this account reads it before opening it, with what it hands,
/// opened with the code: the block's name and whether opening it joins the organization as a
/// guest are what the person is asked to open it on.
pub async fn preview_share_link(
    connection: &Arc<Connection>,
    code: &InviteCode,
) -> Result<(be_block_protocol::ShareLink, Handed), ClientError> {
    let link = read_share_link(connection, code).await?;
    let handed = Handed::open(code, &link)?;
    Ok((link, handed))
}

/// How often `open_share_link` signs its grant again after the block's log moved under it.
const OPEN_TRIES: usize = 3;

/// Opens the share link with `code` as `account`: takes the scope key it carries, sealing it to
/// this account, signs this account's grant with the key the code derives after the head of the
/// block's log, and, for a guest of the organization, pins the root of its log the link carries
/// when this account pinned none. Refused when the link no longer shares the block, or grants
/// the level, that `expected` (what the person was shown) names. The account's block keeps the
/// root of the block's log the link handed it, before the grant is sent: `Peer::joined_log`
/// takes the block's log only from that root, which is what keeps a log the server made up from
/// being trusted. Reading the log once opened and refusing one rooted elsewhere
/// (`ClientError::Forged`) only says so at once. Answers the link and what it handed.
pub async fn open_share_link(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    account: Uuid,
    code: &InviteCode,
    expected: Option<(Uuid, Access)>,
) -> Result<(be_block_protocol::ShareLink, Handed), ClientError> {
    // The record names these keys; the server holds them first, so it checks.
    contacts::register_identity(connection, identity).await?;
    let mut link = read_share_link(connection, code).await?;
    let handed = Handed::open(code, &link)?;
    if expected.is_some_and(|expected| expected != (link.block, link.access)) {
        return Err(ClientError::Refused(
            ErrorCode::InvalidRequest,
            "the link no longer shares what it showed: open it again to see what it does".into(),
        ));
    }
    let level = level_of(link.access)?;
    let mine = Party {
        account: account.into_bytes(),
        public: identity.public(),
    };
    let (key, sealed) = match &handed.scope {
        None => (None, Vec::new()),
        Some(scope) => (
            Some(scope.key),
            identity
                .seal_for(
                    account.into_bytes(),
                    &mine,
                    scope.key.as_bytes(),
                    &scope.carried,
                )
                .map_err(|error| {
                    ClientError::Encoding(format!(
                        "this device could not seal the link's key to this account: {error:?}"
                    ))
                })?,
        ),
    };
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
    // Kept before the grant, so the root the link handed is never lost.
    if let Some(records) = AccountRecords::open(connection, account, identity).await? {
        let kept = KeptShare {
            code: code.to_bytes().to_vec(),
            organization: link.organization,
            block: link.block,
            root: handed.block_root,
            made: false,
        }
        .encode();
        let id = link.id;
        records
            .update(move |_| vec![be_block::AccountBlock::set_share(id, Some(kept.clone()))])
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
                key,
                sealed: sealed.clone(),
            },
        )
        .await?;
        let Err(error) = written else {
            break;
        };
        tries += 1;
        if tries >= OPEN_TRIES {
            return Err(error.into());
        }
        // Only the head moved: whether this account is a guest is as judged once above.
        let again = served_link(connection, code).await?;
        if again.head == link.head {
            return Err(error.into());
        }
        link = be_block_protocol::ShareLink {
            guest: link.guest,
            ..again
        };
    }
    // The log the grant went into must start at the root the link handed.
    let served = be_block_protocol::read::<be_block_protocol::AccessLogPage, _>(
        connection,
        Some(link.organization),
        be_block_protocol::Query::Log {
            log: Some(link.block),
            held: None,
        },
    )
    .await??;
    let root = served
        .entries
        .first()
        .filter(|_| served.from == 0)
        .and_then(|bytes| be_block_keys::Step::decode(bytes).ok())
        .map(|step| step.hash());
    if root != Some(handed.block_root) {
        return Err(ClientError::Forged(link.block));
    }
    Ok((link, handed))
}

impl<S: ObjectStore> Peer<S> {
    /// Makes a share link on `block` granting `access` (Join, View or Edit) to whoever opens it,
    /// as `admits` allows: for View or Edit, makes the block a scope of its own, keeps the link's code in the
    /// account's block, and opens the link in the block's log. Answers the code; the link is
    /// `{server}/#share/{CODE}`.
    pub async fn make_share_link(
        &self,
        block: Uuid,
        access: Access,
        admits: Admits,
    ) -> Result<InviteCode, ClientError> {
        let code = InviteCode::generate();
        self.make_share_link_with(&code, block, access, admits)
            .await?;
        Ok(code)
    }

    /// An invitation to one new person (guides/blocks/keys.md, "An invitation"): a share
    /// link on `block` at `access` that admits the first to open it, and a plain contact link,
    /// both with one code, which is all the person is sent. Opening it makes the contact, then
    /// opens the share link, as each does alone. Answers the code; the invitation is the
    /// contact link, `{server}/#contact/{CODE}`.
    pub async fn make_invitation(
        &self,
        me: &be_block_keys::ContactCard,
        block: Uuid,
        access: Access,
    ) -> Result<InviteCode, ClientError> {
        let code = InviteCode::generate();
        // The share link first: one the organization's policy refuses leaves no contact link.
        self.make_share_link_with(&code, block, access, Admits::One)
            .await?;
        let made =
            crate::contacts::make_link_with(&self.connection(), self.keys().identity(), me, &code)
                .await;
        if let Err(error) = made {
            // Best effort: a share link left without its contact link is no invitation, and
            // would show as one not opened yet.
            let row = ShareLinkRow {
                id: link_id(&code),
                access,
                admits: LinkAdmits::One,
                maker: self.account(),
                maker_name: String::new(),
                created: 0,
                key: code.share_key(block.into_bytes()).0,
                opened_by: None,
                opened_by_name: String::new(),
            };
            let _ = self.revoke_share_link(block, &row).await;
            return Err(error);
        }
        Ok(code)
    }

    async fn make_share_link_with(
        &self,
        code: &InviteCode,
        block: Uuid,
        access: Access,
        admits: Admits,
    ) -> Result<(), ClientError> {
        let level = level_of(access)?;
        // A Join link carries no key: its opener reads nothing stored, and joins whoever hosts.
        let scope = match access {
            Access::Join => None,
            _ => {
                let offer = self.offer(block, access).await?;
                Some(HandedKey {
                    key: offer.key,
                    carried: offer.secret,
                })
            }
        };
        let link = link_id(code);
        // The root of the block's log as this account pinned it: the opener checks by it.
        let handed_root = match self.keys().root_pin(block) {
            crate::keys::RootPin::Pinned(root) => root,
            // Taken from a member's word alone: a link handing it on would vouch for a root
            // this device never verified.
            crate::keys::RootPin::Weak(_) => {
                return Err(ClientError::Refused(
                    ErrorCode::PermissionDenied,
                    "this device knows who made the block only from another member's word: make the link on a device that made it, or holds a key of it from its manager".into(),
                ));
            }
            crate::keys::RootPin::Conflicting => return Err(ClientError::Forged(block)),
            crate::keys::RootPin::Unknown => return Err(ClientError::Locked(block)),
        };
        let handed = Handed {
            scope,
            block_root: handed_root,
            organization_root: self.keys().organization_root(),
            name: self.kept_name(block).unwrap_or_default(),
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
                root: handed_root,
                made: true,
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
        Ok(())
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
        let kept: Vec<_> = self
            .kept_shares()
            .await?
            .into_iter()
            .filter(|(_, kept)| kept.made)
            .collect();
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
            .next(
                self.keys().identity(),
                me,
                block.into_bytes(),
                make(authority),
            )
            .map(|entry| entry.encode())
            .ok_or_else(|| {
                ClientError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("this account is not served the whole log of block {block}"),
                )
            })
    }

    async fn records(&self) -> Result<Arc<AccountRecords>, ClientError> {
        AccountRecords::open(
            &self.connection(),
            self.keys().account(),
            self.keys().identity(),
        )
        .await?
        .ok_or_else(|| {
            ClientError::Refused(
                ErrorCode::PermissionDenied,
                "an account in no personal organization makes no share links".into(),
            )
        })
    }

    pub(crate) async fn keep_share(
        &self,
        link: Uuid,
        kept: Option<KeptShare>,
    ) -> Result<(), ClientError> {
        let bytes = kept.map(|kept| kept.encode());
        self.records()
            .await?
            .update(move |current| match (current.share(link), &bytes) {
                (None, None) => Vec::new(),
                _ => vec![be_block::AccountBlock::set_share(link, bytes.clone())],
            })
            .await
    }

    pub(crate) async fn kept_shares(&self) -> Result<Vec<(Uuid, KeptShare)>, ClientError> {
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
