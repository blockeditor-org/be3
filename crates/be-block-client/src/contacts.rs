use be_block_graph::Access;
use be_block_keys::{
    CardRole, ContactCard, InviteCode, Own, Party, PublicIdentity, RecoveryPublic, RecoverySecret,
    SigningPublic,
};
use std::collections::HashMap;

use be_block_protocol::{
    ErrorCode, EscrowKey, Invitation, KeyChange, Link, LinkKind, PendingGrant, SealedKey,
};
use uuid::Uuid;

use std::sync::{Arc, Mutex};

use crate::{AccountRecords, ClientError, connection::Connection, peer::account_block::answered};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Contact {
    pub account: Uuid,
    /// The name and email on the card this account verified, never the server's row, which
    /// could name one contact as another (guides/blocks/keys.md, "Contacts"). A contact whose card has
    /// not opened keeps the row's, and has no key to seal to.
    pub email: String,
    pub display_name: String,
    /// The identity key verified through a contact link, or a key change sealed with the one
    /// before; `None` while the card has not opened.
    pub public: Option<RecoveryPublic>,
    /// The signing key verified with it.
    pub signing: Option<SigningPublic>,
    /// When this account verified the card or last followed a key change (`OwnContact`); 0
    /// while unverified.
    pub verified: u64,
    /// The newest mailbox message its record reflects (`OwnContact::through`).
    pub through: u64,
    /// How the server's record of this contact differed from the newest this device had
    /// seen, which it kept and seals to; `None` when it served that one or a newer one.
    pub mismatch: Option<Mismatch>,
}

/// What was wrong with the record the server served for a contact this device had pinned.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Mismatch {
    /// An older record, as a server bringing back a key the contact replaced would serve.
    Older,
    /// One that does not open: changed, or sealed under a new phrase this account took on
    /// another device, which this one then needs too.
    Unreadable,
}

impl Contact {
    /// The record this device pins for the contact, when verified.
    pub fn record(&self) -> Option<OwnContact> {
        Some(OwnContact {
            card: ContactCard {
                account: self.account.into_bytes(),
                public: self.public?,
                signing: self.signing?,
                display_name: self.display_name.clone(),
                email: self.email.clone(),
            },
            verified: self.verified,
            through: self.through,
        })
    }
}

/// A contact as this account verified it, kept in the account's block (`AccountRecords`): the
/// card, and when it was verified or its key last changed, a sequence that only grows. The server
/// cannot read or make one; what it can do is serve an older version of the block, so each device
/// pins the newest record it has seen and refuses any older (guides/blocks/keys.md, "Contacts").
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct OwnContact {
    pub card: ContactCard,
    pub verified: u64,
    /// The newest mailbox message the record reflects: the letter that made it (`contact`,
    /// `accepted`) or the last key change it followed. A letter up to it is taken already, and
    /// deleted unread, so a device that stops between writing the record and deleting what it
    /// followed never follows a change twice nor takes back a removal.
    pub through: u64,
}

impl OwnContact {
    /// A record of `card` verified now, after any record of it with `verified` `after`.
    pub fn new(card: ContactCard, after: u64) -> Self {
        let now = Uuid::now_v7()
            .get_timestamp()
            .map_or(0, |time| time.to_unix().0);
        Self {
            card,
            verified: now.max(after.saturating_add(1)),
            through: 0,
        }
    }

    /// Sealed for this account alone, as a letter carries it (`letter.contact`) to its own
    /// mailbox.
    pub fn seal(&self, identity: &RecoverySecret) -> Vec<u8> {
        identity.seal_own(Own::Contact, &self.encode())
    }

    /// The record sealed in `sealed`, if it opens and names `account`.
    pub fn open(identity: &RecoverySecret, sealed: &[u8], account: Uuid) -> Option<Self> {
        Self::decode(&identity.open_own(Own::Contact, sealed).ok()?, account)
    }

    /// As the account's block holds it, sealed by the block's own keys.
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a contact record always encodes")
    }

    /// The record in `bytes`, if it names `account`.
    pub fn decode(bytes: &[u8], account: Uuid) -> Option<Self> {
        let record: Self = postcard::from_bytes(bytes).ok()?;
        (record.card.account == account.into_bytes()).then_some(record)
    }
}

impl OwnContact {
    /// As the account's block keeps it (`KeptContact::Record`).
    pub fn kept(&self) -> Vec<u8> {
        KeptContact::Record(self.clone()).encode()
    }
}

/// What the account's block keeps of a contact: its record, or that it was removed, with the
/// newest `verified` and letter the record had reached. A removal is kept rather than forgotten,
/// so a record the server replays afterwards, however it was once sealed, is taken only if newer
/// than any the account held, as a fresh link's always is: the server can make an account forget
/// a contact (a made-up `letter.removed`), but never take back an older card.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum KeptContact {
    Record(OwnContact),
    Removed { verified: u64, through: u64 },
}

impl KeptContact {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a kept contact always encodes")
    }

    /// The kept contact in `bytes`, if a record in it names `account`.
    pub fn decode(bytes: &[u8], account: Uuid) -> Option<Self> {
        let kept: Self = postcard::from_bytes(bytes).ok()?;
        match &kept {
            Self::Record(record) if record.card.account != account.into_bytes() => None,
            _ => Some(kept),
        }
    }

    pub fn verified(&self) -> u64 {
        match self {
            Self::Record(record) => record.verified,
            Self::Removed { verified, .. } => *verified,
        }
    }

    pub fn through(&self) -> u64 {
        match self {
            Self::Record(record) => record.through,
            Self::Removed { through, .. } => *through,
        }
    }

    pub fn record(self) -> Option<OwnContact> {
        match self {
            Self::Record(record) => Some(record),
            Self::Removed { .. } => None,
        }
    }
}

/// What the account's block keeps of each contact, removed ones included.
fn kept_of(block: &be_block::AccountBlock) -> HashMap<Uuid, KeptContact> {
    block
        .contacts
        .iter()
        .filter_map(|(account, bytes)| Some((*account, KeptContact::decode(bytes, *account)?)))
        .collect()
}

/// The records the account's block holds, by contact.
fn records_of(block: &be_block::AccountBlock) -> HashMap<Uuid, OwnContact> {
    kept_of(block)
        .into_iter()
        .filter_map(|(account, kept)| Some((account, kept.record()?)))
        .collect()
}

/// This account's identity secret and those it replaced with a new phrase
/// (`AccountBlock::retired`): a contact that sealed its key change to a key this account
/// replaced before it followed the new one (it took its own new phrase meanwhile, say) is
/// followed all the same.
struct Secrets<'a> {
    current: &'a RecoverySecret,
    retired: Vec<RecoverySecret>,
}

impl<'a> Secrets<'a> {
    fn new(current: &'a RecoverySecret, block: Option<&be_block::AccountBlock>) -> Self {
        let retired = block
            .map(|block| {
                block
                    .retired_secrets()
                    .filter_map(|bytes| Some(RecoverySecret::from_bytes(bytes.try_into().ok()?)))
                    .collect()
            })
            .unwrap_or_default();
        Self { current, retired }
    }

    /// The secrets of `account`'s block, as `identity` reads it: none retired when it has no
    /// block or it cannot be read, so taking a link never fails for want of them.
    async fn held(
        connection: &Arc<Connection>,
        account: Uuid,
        identity: &'a RecoverySecret,
    ) -> Self {
        let block = match AccountRecords::open(connection, account, identity).await {
            Ok(Some(block)) => block,
            Ok(None) | Err(_) => return Self::new(identity, None),
        };
        match block.read().await {
            Ok(content) => Self::new(identity, Some(&content.root())),
            Err(_) => Self::new(identity, None),
        }
    }

    fn open_key_change(
        &self,
        from: &Party,
        to: [u8; 16],
        sealed: &[u8],
    ) -> Result<PublicIdentity, be_block_keys::KeyError> {
        let current = self.current.open_key_change(from, to, sealed);
        if current.is_ok() {
            return current;
        }
        // When none opens it, why the current one did not.
        self.retired
            .iter()
            .map(|secret| secret.open_key_change(from, to, sealed))
            .find(Result::is_ok)
            .unwrap_or(current)
    }
}

/// Where `public`'s secret is kept once retired (`AccountBlock::retired`).
fn retired_key(public: &be_block_keys::RecoveryPublic) -> Uuid {
    Uuid::from_slice(&public.0[..16]).expect("sixteen bytes make an id")
}

#[derive(Clone)]
pub struct GrantOffer {
    pub organization: Uuid,
    pub block: Uuid,
    pub access: Access,
    pub key: Uuid,
    /// The scope key as it travels (`be_block_keys::CarriedKey`), with the block and its root.
    pub secret: Vec<u8>,
}

impl std::fmt::Debug for GrantOffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GrantOffer")
            .field("organization", &self.organization)
            .field("block", &self.block)
            .field("access", &self.access)
            .field("key", &self.key)
            .field("secret", &"..")
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct Finished {
    pub card: ContactCard,
    pub grant: Option<PendingGrant>,
    /// For an invitation, the organization its accepter joined, signing themselves in as they
    /// accepted it: the link is finished in that organization.
    pub invited: Option<Uuid>,
    /// The link, which `Peer::finish_links` deletes once it signed the grant, if any,
    /// with the mailbox message that brought its accepter's card (`letter`), so a device that
    /// stops before then finishes it again.
    pub(crate) link: Uuid,
    pub(crate) letter: u64,
}

pub fn card(
    account: Uuid,
    identity: &RecoverySecret,
    display_name: &str,
    email: &str,
) -> ContactCard {
    ContactCard {
        account: account.into_bytes(),
        public: identity.public(),
        signing: identity.signing(),
        display_name: display_name.to_owned(),
        email: email.to_owned(),
    }
}

/// What an account pins for a company organization from somewhere the server cannot write
/// (making it, an invitation, an agent's maker), sealed to itself (`seal_pin`): the hash of the
/// organization's access log root and its escrow key, and when it was pinned (`made`), a
/// sequence that only grows: the server keeps every pin a letter brought and can send an old one
/// again, so a device takes a pin only over an older one.
#[derive(Clone, Debug, Default, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct Pinned {
    pub root: Option<be_block_keys::EntryHash>,
    pub escrow: Option<EscrowKey>,
    /// When it was pinned, in milliseconds since the epoch, as `KeptLink::made`.
    pub made: u64,
}

impl Pinned {
    /// This pin made now, after any pin made at `after`.
    pub fn stamped(self, after: u64) -> Self {
        Self {
            made: now_milliseconds().max(after.saturating_add(1)),
            ..self
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a pin always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none() && self.escrow.is_none()
    }
}

/// A link this account made, as its block keeps it until the link is finished: its code, and
/// what accepting it does.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct KeptLink {
    pub code: Vec<u8>,
    pub kind: LinkKind,
    /// When it was made, in milliseconds since the epoch.
    pub made: u64,
}

/// Leaves out the code, a bearer secret, so no log ever holds it.
impl std::fmt::Debug for KeptLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeptLink")
            .field("code", &"..")
            .field("kind", &self.kind)
            .field("made", &self.made)
            .finish()
    }
}

/// The most links the account's block keeps: the server keeps at most a thousand contact links
/// open per account (`MAX_CONTACT_LINKS_PER_ACCOUNT`), so past it the oldest goes.
pub const MAX_KEPT_LINKS: usize = 1_000;

/// How long the block keeps an invitation's code: its link lasts a week, and an accepted one
/// waits for its inviter's device to finish it, which this leaves weeks for.
pub const KEPT_INVITATION: u64 = 30 * 86_400_000;

fn now_milliseconds() -> u64 {
    Uuid::now_v7().get_timestamp().map_or(0, |time| {
        let (seconds, nanos) = time.to_unix();
        seconds * 1000 + u64::from(nanos) / 1_000_000
    })
}

impl KeptLink {
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a kept link always encodes")
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }

    pub fn code(&self) -> Option<InviteCode> {
        InviteCode::from_bytes(&self.code).ok()
    }
}

/// The links this account made that its block keeps, by link; none for an account with no block
/// or one this device cannot read.
pub async fn kept_links(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
) -> Result<HashMap<Uuid, KeptLink>, ClientError> {
    let Some(block) = AccountRecords::open(connection, account, identity).await? else {
        return Ok(HashMap::new());
    };
    Ok(match block.read().await {
        Ok(content) => content
            .root()
            .links
            .iter()
            .filter_map(|(link, bytes)| Some((*link, KeptLink::decode(bytes)?)))
            .collect(),
        Err(ClientError::Locked(_)) => HashMap::new(),
        Err(error) => return Err(error),
    })
}

/// Keeps `link` in the account's block before the server holds it, so its code is never lost:
/// a link the server then refuses leaves a code nothing reaches.
async fn keep_link(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    link: Uuid,
    kept: KeptLink,
) -> Result<(), ClientError> {
    let Some(block) = AccountRecords::open(connection, account, identity).await? else {
        return Err(ClientError::Refused(
            ErrorCode::PermissionDenied,
            "an account in no personal organization makes no links".into(),
        ));
    };
    let bytes = kept.encode();
    block
        .update(move |content| {
            let current = content.root();
            let mut edits = vec![be_block::AccountBlock::set_link(link, Some(bytes.clone()))];
            // Invitations long past their life go, and past the bound the oldest links, so the
            // block holds no codes for links the server no longer keeps.
            let now = now_milliseconds();
            let mut held: Vec<(u64, Uuid)> = current
                .links
                .iter()
                .filter(|(id, _)| **id != link)
                .filter_map(|(id, bytes)| Some((*id, KeptLink::decode(bytes)?)))
                .filter_map(|(id, kept)| {
                    let stale = matches!(kept.kind, LinkKind::Invitation(_))
                        && kept.made.saturating_add(KEPT_INVITATION) < now;
                    match stale {
                        true => {
                            edits.push(be_block::AccountBlock::set_link(id, None));
                            None
                        }
                        false => Some((kept.made, id)),
                    }
                })
                .collect();
            held.sort();
            let over = (held.len() + 1).saturating_sub(MAX_KEPT_LINKS);
            edits.extend(
                held[..over]
                    .iter()
                    .map(|(_, id)| be_block::AccountBlock::set_link(*id, None)),
            );
            answered(edits)
        })
        .await
}

/// What the account pins for `organization`: the newest of what its block keeps and the letters
/// not yet taken into it (an agent, which has no block, keeps the one its maker left), by when
/// each was pinned, never by mailbox order, so an old letter the server sends again is not taken.
pub async fn pinned(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    organization: Uuid,
) -> Result<Option<Pinned>, ClientError> {
    let mut found: Vec<Pinned> = mailbox(connection)
        .await?
        .into_iter()
        .filter_map(|message| match message.letter {
            be_block_protocol::Letter::Pinned {
                organization: of,
                sealed,
            } if of == organization => open_pin(identity, organization, &sealed),
            _ => None,
        })
        .collect();
    if let Some(block) = AccountRecords::open(connection, account, identity).await? {
        match block.read().await {
            Ok(content) => found.extend(content.root().pin(organization).and_then(Pinned::decode)),
            Err(ClientError::Locked(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(found.into_iter().max_by_key(|pinned| pinned.made))
}

/// Keeps `pinned` for `organization` in the account's block; an account with no block keeps
/// what its letter carried.
pub async fn keep_pin(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    organization: Uuid,
    pinned: &Pinned,
) -> Result<(), ClientError> {
    let Some(block) = AccountRecords::open(connection, account, identity).await? else {
        return Ok(());
    };
    block
        .update(move |content| {
            let after = content
                .root()
                .pin(organization)
                .and_then(Pinned::decode)
                .map_or(0, |held| held.made);
            let pinned = pinned.clone().stamped(after);
            answered(vec![be_block::AccountBlock::set_pin(
                organization,
                Some(pinned.encode()),
            )])
        })
        .await
}

/// Makes a company organization with this account as its administrator, rooting its access
/// log at the account's keys (guides/blocks/keys.md) and pinning that root for it. The server
/// takes a root only as the caller's current keys, so they are registered first when the
/// server holds none yet.
pub async fn create_organization(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    account: Uuid,
    name: String,
) -> Result<be_block_protocol::Organization, ClientError> {
    register_identity(connection, identity).await?;
    let id = Uuid::new_v4();
    let root = be_block_keys::Entry::root(
        identity,
        account.into_bytes(),
        be_block_keys::LogKind::Organization,
        id.into_bytes(),
    );
    let pinned = Pinned {
        root: Some(root.hash()),
        escrow: None,
        made: 0,
    }
    .stamped(0);
    crate::ask(
        be_block_protocol::write::<be_block_protocol::Organization, _>(
            connection,
            None,
            be_block_protocol::Op::CreateOrganization {
                id: id,
                name: name,
                root: root.encode(),
                pinned: seal_pin(identity, id, &pinned),
            },
        ),
    )
    .await
}

/// Registers `identity` as the account's keys when the server holds none yet; answers the
/// recovery key as the server held it (`query.recovery-key`), what the account sealed for itself
/// in it.
pub async fn register_identity(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
) -> Result<be_block_protocol::RecoveryKey, ClientError> {
    let held = crate::ask(
        be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
            connection,
            None,
            be_block_protocol::Query::RecoveryKey,
        ),
    )
    .await?;
    if held.public.is_some() {
        return Ok(held);
    }
    crate::ask(be_block_protocol::write::<(), _>(
        connection,
        None,
        be_block_protocol::Op::SetRecoveryKey {
            public: identity.public().0,
            signing: identity.signing().0,
            sealed: Vec::new(),
            changes: Vec::new(),
            retirements: Vec::new(),
        },
    ))
    .await?;
    Ok(held)
}

pub fn seal_pin(identity: &RecoverySecret, organization: Uuid, pinned: &Pinned) -> Vec<u8> {
    match pinned.is_empty() {
        true => Vec::new(),
        false => identity.seal_own(Own::Pin(organization.into_bytes()), &pinned.encode()),
    }
}

pub fn open_pin(identity: &RecoverySecret, organization: Uuid, sealed: &[u8]) -> Option<Pinned> {
    if sealed.is_empty() {
        return None;
    }
    Pinned::decode(
        &identity
            .open_own(Own::Pin(organization.into_bytes()), sealed)
            .ok()?,
    )
}

fn link_id(code: &InviteCode) -> Uuid {
    Uuid::from_bytes(code.lookup())
}

fn tampered() -> ClientError {
    ClientError::Encoding(
        "the contact card did not open with this link, so it was not the one its maker sent"
            .to_owned(),
    )
}

/// The account's mailbox (`query.mailbox`), oldest first.
pub async fn mailbox(
    connection: &Arc<Connection>,
) -> Result<Vec<be_block_protocol::Message>, ClientError> {
    crate::ask(
        be_block_protocol::read::<Vec<be_block_protocol::Message>, _>(
            connection,
            None,
            be_block_protocol::Query::Mailbox,
        ),
    )
    .await
}

/// The account's contacts, from its records in the account's block (`AccountRecords`), each
/// checked against `pinned`, the newest record of it this device has seen: an older record the
/// server serves is refused for the pinned one, and the contact listed with the mismatch for as
/// long as the server serves it (only a server that misbehaves does). Records arrive as letters
/// (`letter.contact`, made by the write that made two contacts; `letter.removed`), which a pass
/// keeps in the block in mailbox order and then deletes. Key changes in the mailbox are followed
/// from the record kept, in order (`follow`), and deleted once the record that follows them is
/// written, with any before the last followed that did not open (not the contact's). One after it
/// that does not open breaks the chain: the server let older ones go (`MAX_LETTERS_FROM_ONE`), or
/// it is sealed to a phrase this account took since. The contact is then listed as not
/// confirmed, with nothing sealed to it, and those changes are kept, until the two connect again
/// by a link, which deletes them (`accept_link`, `finish_links`). Nothing is written or deleted
/// while this device cannot read the block, as after a new phrase taken on another device, whose
/// identity those changes may be sealed to. A record is written only over the block as read, so a
/// device that read fewer changes than another never rolls the other's record back: refused, it
/// reads again. An account in no personal organization (an agent) has no block: it keeps its
/// letters and reads its contacts from them each time.
pub async fn contacts(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    pinned: &HashMap<Uuid, OwnContact>,
) -> Result<Vec<Contact>, ClientError> {
    let block = AccountRecords::open(connection, account, identity).await?;
    if let Some(block) = block.as_deref() {
        // The pass over the block as last read, whose letters go once the block keeps them.
        let passed: Mutex<Option<Pass>> = Mutex::new(None);
        let updated = block
            .update(|content| {
                let passed = &passed;
                Box::pin(async move {
                    let (pass, edits) =
                        pass(connection, account, identity, pinned, Some(&content), true).await?;
                    *passed.lock().unwrap() = Some(pass);
                    Ok(edits)
                })
            })
            .await;
        match (updated, passed.into_inner().unwrap()) {
            (Ok(()), Some(pass)) => {
                if !pass.taken.is_empty() {
                    be_block_protocol::write_all::<(), _>(
                        connection,
                        None,
                        pass.taken
                            .into_iter()
                            .map(be_block_protocol::Op::DeleteMessage)
                            .collect(),
                    )
                    .await??;
                }
                return Ok(pass.listed);
            }
            // Not readable on this device, as after a new phrase taken on another: listed from
            // the pins, and nothing written or deleted.
            (Err(ClientError::Locked(_)), None) => {}
            (Err(error), _) => return Err(error),
            (Ok(()), None) => return Err(ClientError::Unexpected),
        }
    }
    let (pass, _) = pass(connection, account, identity, pinned, None, block.is_none()).await?;
    Ok(pass.listed)
}

/// What a pass over the account's block and mailbox lists, and the letters it took into the
/// block.
struct Pass {
    listed: Vec<Contact>,
    taken: Vec<u64>,
}

/// One pass of `contacts` over `held`, the block as read (none when it was not), answering the
/// edits that keep what it took. `readable` when the account's contacts are read from `held`
/// or, for an account with no block, from its letters alone.
async fn pass(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    pinned: &HashMap<Uuid, OwnContact>,
    held: Option<&be_block::AccountBlockContent>,
    readable: bool,
) -> Result<(Pass, Vec<be_block_model::Edit>), ClientError> {
    // Letters are taken, and deleted, only into a block this device reads.
    let keeps = held.is_some();
    let root = held.map(|held| held.root());
    let kept = root.as_ref().map(kept_of).unwrap_or_default();
    let secrets = Secrets::new(identity, root.as_ref());
    let mut removed: HashMap<Uuid, (u64, u64)> = kept
        .iter()
        .filter_map(|(account, kept)| match kept {
            KeptContact::Removed { verified, through } => Some((*account, (*verified, *through))),
            KeptContact::Record(_) => None,
        })
        .collect();
    let read: HashMap<Uuid, OwnContact> = kept
        .iter()
        .filter_map(|(account, kept)| Some((*account, kept.clone().record()?)))
        .collect();
    let mut records = match readable {
        true => read.clone(),
        false => pinned.clone(),
    };
    let mailbox = mailbox(connection).await?;
    let mut taken = Vec::new();
    let mut pins: HashMap<Uuid, Pinned> = HashMap::new();
    let mut newest_pin: HashMap<Uuid, u64> = root
        .as_ref()
        .map(|root| {
            root.pins
                .iter()
                .filter_map(|(organization, bytes)| {
                    Some((*organization, Pinned::decode(bytes)?.made))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut changes: HashMap<Uuid, Vec<(u64, Vec<u8>)>> = HashMap::new();
    for message in &mailbox {
        let Some(from) = message.from else {
            continue;
        };
        // The newest the account held of the contact, kept or removed.
        let (verified, through) = match (records.get(&from), removed.get(&from)) {
            (Some(record), _) => (record.verified, record.through),
            (None, Some(held)) => *held,
            (None, None) => (0, 0),
        };
        match &message.letter {
            be_block_protocol::Letter::Contact(sealed) => {
                if message.id > through
                    && let Some(mut record) = OwnContact::open(identity, sealed, from)
                    && verified < record.verified
                {
                    record.through = message.id;
                    records.insert(from, record);
                    removed.remove(&from);
                }
                taken.push(message.id);
            }
            be_block_protocol::Letter::Removed => {
                if message.id > through && records.remove(&from).is_some() {
                    removed.insert(from, (verified, message.id));
                }
                taken.push(message.id);
            }
            be_block_protocol::Letter::Pinned {
                organization,
                sealed,
            } => {
                // Taken only over an older pin, whatever its place in the mailbox.
                if let Some(pinned) = open_pin(identity, *organization, sealed)
                    && newest_pin
                        .get(organization)
                        .is_none_or(|made| *made < pinned.made)
                {
                    newest_pin.insert(*organization, pinned.made);
                    pins.insert(*organization, pinned);
                }
                taken.push(message.id);
            }
            be_block_protocol::Letter::KeyChange(sealed) => {
                changes
                    .entry(from)
                    .or_default()
                    .push((message.id, sealed.clone()));
            }
            _ => {}
        }
    }
    let mut held_keys = None;
    let mut listed = Vec::with_capacity(records.len());
    let mut accounts: Vec<Uuid> = records.keys().copied().collect();
    accounts.sort();
    for contact in accounts {
        let mismatch = match (pinned.get(&contact), readable) {
            (None, _) => None,
            (Some(_), false) => Some(Mismatch::Unreadable),
            (Some(pin), true) => {
                (records[&contact].verified < pin.verified).then_some(Mismatch::Older)
            }
        };
        if mismatch == Some(Mismatch::Older) {
            records.insert(contact, pinned[&contact].clone());
        }
        let record = records.get_mut(&contact).expect("listed from the records");
        let letters = changes.remove(&contact).unwrap_or_default();
        let (already, letters): (Vec<_>, Vec<_>) = letters
            .into_iter()
            .partition(|(id, _)| *id <= record.through);
        taken.extend(already.iter().map(|(id, _)| *id));
        let sealed: Vec<Vec<u8>> = letters.iter().map(|(_, sealed)| sealed.clone()).collect();
        let (changed, followed) = follow(account, &secrets, &record.card, &sealed);
        let broken = readable && followed < letters.len();
        if let Some(changed) = changed {
            if held_keys.is_none() {
                held_keys = Some(sealed_to_me(connection).await?);
            }
            reseal_from(
                connection,
                account,
                identity,
                &record.card,
                held_keys.as_deref().unwrap_or_default(),
            )
            .await?;
            let mut card = record.card.clone();
            card.public = changed.public;
            card.signing = changed.signing;
            *record = OwnContact {
                through: letters[followed - 1].0,
                ..OwnContact::new(card, record.verified)
            };
        }
        taken.extend(letters[..followed].iter().map(|(id, _)| *id));
        let record = record.clone();
        listed.push(match broken {
            // Its key changed past what this device can follow: not confirmed, as a contact
            // whose card never opened, though named as it was verified.
            true => Contact {
                account: contact,
                email: record.card.email,
                display_name: record.card.display_name,
                public: None,
                signing: None,
                verified: 0,
                through: record.through,
                mismatch: None,
            },
            false => Contact {
                account: contact,
                email: record.card.email,
                display_name: record.card.display_name,
                public: Some(record.card.public),
                signing: Some(record.card.signing),
                verified: record.verified,
                through: record.through,
                mismatch,
            },
        });
    }
    let mut edits = Vec::new();
    if keeps {
        // Each contact whose state, kept or removed, differs from what the block held: a record
        // taken and removed in one pass still leaves its tombstone.
        edits.extend(pins.into_iter().map(|(organization, pinned)| {
            be_block::AccountBlock::set_pin(organization, Some(pinned.encode()))
        }));
        let mut ended: HashMap<Uuid, KeptContact> = removed
            .iter()
            .map(|(contact, (verified, through))| {
                (
                    *contact,
                    KeptContact::Removed {
                        verified: *verified,
                        through: *through,
                    },
                )
            })
            .collect();
        ended.extend(
            records
                .iter()
                .map(|(contact, record)| (*contact, KeptContact::Record(record.clone()))),
        );
        for (contact, now) in &ended {
            if kept.get(contact) != Some(now) {
                edits.push(be_block::AccountBlock::set_contact(
                    *contact,
                    Some(now.encode()),
                ));
            }
        }
    }
    taken.sort_unstable();
    taken.dedup();
    Ok((Pass { listed, taken }, edits))
}

/// Follows `changes` in order from `card`'s key: the key they lead to, if another, and how many
/// of them lie up to the last that opened, which the device may delete.
fn follow(
    account: Uuid,
    secrets: &Secrets,
    card: &ContactCard,
    changes: &[Vec<u8>],
) -> (Option<PublicIdentity>, usize) {
    let held = PublicIdentity {
        public: card.public,
        signing: card.signing,
    };
    let mut current = held;
    let mut followed = 0;
    for (index, sealed) in changes.iter().enumerate() {
        let from = Party {
            account: card.account,
            public: current.public,
        };
        if let Ok(next) = secrets.open_key_change(&from, account.into_bytes(), sealed) {
            current = next;
            followed = index + 1;
        }
    }
    ((current != held).then_some(current), followed)
}

/// Deletes of the key changes `card`'s account left in the mailbox that a record made from
/// `card` by a link outdates: those before the first that opens from its key (all, when none
/// does), as a chain broken before the link no longer counts. Any it leads on to stay, for
/// `contacts` to follow.
fn outdated(
    mailbox: &[be_block_protocol::Message],
    account: Uuid,
    secrets: &Secrets,
    card: &ContactCard,
) -> Vec<be_block_protocol::Op> {
    let from = Party {
        account: card.account,
        public: card.public,
    };
    mailbox
        .iter()
        .filter_map(|message| match &message.letter {
            be_block_protocol::Letter::KeyChange(sealed)
                if message.from == Some(Uuid::from_bytes(card.account)) =>
            {
                Some((message.id, sealed))
            }
            _ => None,
        })
        .take_while(|(_, sealed)| {
            secrets
                .open_key_change(&from, account.into_bytes(), sealed)
                .is_err()
        })
        .map(|(id, _)| be_block_protocol::Op::DeleteMessage(id))
        .collect()
}

async fn sealed_to_me(connection: &Arc<Connection>) -> Result<Vec<SealedKey>, ClientError> {
    Ok(
        be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
            connection,
            None,
            be_block_protocol::Query::RecoveryKey,
        )
        .await??
        .sealed,
    )
}

async fn reseal_from(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    card: &ContactCard,
    held: &[SealedKey],
) -> Result<(), ClientError> {
    let from = Party {
        account: card.account,
        public: card.public,
    };
    let mine = Party {
        account: account.into_bytes(),
        public: identity.public(),
    };
    let keys: Vec<SealedKey> = held
        .iter()
        .filter(|key| key.sender.into_bytes() == card.account)
        .filter_map(|key| {
            let plain = identity
                .open_from(&from, account.into_bytes(), key.key.as_bytes(), &key.sealed)
                .ok()?;
            Some(SealedKey {
                sealed: identity
                    .seal_for(account.into_bytes(), &mine, key.key.as_bytes(), &plain)
                    .ok()?,
                sender: account,
                ..key.clone()
            })
        })
        .collect();
    if !keys.is_empty() {
        be_block_protocol::write::<(), _>(
            connection,
            None,
            be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Reseal(keys)),
        )
        .await??;
    }
    Ok(())
}

pub struct NewIdentity {
    pub sealed: Vec<SealedKey>,
    pub changes: Vec<KeyChange>,
    /// An entry retiring the old keys for the new ones in each access log the account signed
    /// in (guides/blocks/keys.md, "Identity, new phrases and devices").
    pub retirements: Vec<be_block_protocol::Retirement>,
}

/// What moving the account from `old` to `fresh` sends: every key in `held` sealed to it by
/// itself or a contact it holds a record of (`records`, from the account's block) sealed again,
/// a key change for each contact, and a retirement in each of `logs`. None when a log's head is
/// malformed.
pub fn new_identity(
    account: Uuid,
    old: &RecoverySecret,
    fresh: &RecoverySecret,
    held: Vec<SealedKey>,
    records: &HashMap<Uuid, OwnContact>,
    logs: Vec<be_block_protocol::SignedLog>,
) -> Option<NewIdentity> {
    let me = Party {
        account: account.into_bytes(),
        public: old.public(),
    };
    let contacts: HashMap<Uuid, Party> = records
        .iter()
        .map(|(contact, record)| {
            (
                *contact,
                Party {
                    account: record.card.account,
                    public: record.card.public,
                },
            )
        })
        .collect();
    let mine = Party {
        account: account.into_bytes(),
        public: fresh.public(),
    };
    let sealed = held
        .into_iter()
        .filter_map(|key| {
            let sender = match key.sender == account {
                true => &me,
                false => contacts.get(&key.sender)?,
            };
            let plain = old
                .open_from(
                    sender,
                    account.into_bytes(),
                    key.key.as_bytes(),
                    &key.sealed,
                )
                .ok()?;
            Some(SealedKey {
                sealed: fresh
                    .seal_for(account.into_bytes(), &mine, key.key.as_bytes(), &plain)
                    .ok()?,
                sender: account,
                ..key
            })
        })
        .collect();
    // A contact whose verified key no change can be sealed to (a low-order point) is told none.
    let changes = contacts
        .iter()
        .filter_map(|(contact, party)| {
            Some(KeyChange {
                contact: *contact,
                sealed: old
                    .seal_key_change(account.into_bytes(), party, fresh.identity())
                    .ok()?,
            })
        })
        .collect();
    let retirements = logs
        .into_iter()
        .map(|log| {
            let head = log.head.try_into().ok()?;
            let entry = be_block_keys::Entry::retirement(
                old,
                fresh,
                account.into_bytes(),
                log.log.into_bytes(),
                head,
            );
            Some(be_block_protocol::Retirement {
                organization: log.organization,
                kind: log.kind,
                log: log.log,
                entry: entry.encode(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(NewIdentity {
        sealed,
        changes,
        retirements,
    })
}

/// Why a device cannot take a new phrase (`set_recovery_key`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CannotReseal {
    /// It has no old phrase, and the account sealed something to it already.
    NoOldPhrase,
    /// Something the old phrase sealed does not open on this device, or the account's block
    /// does not: it was sealed to a newer phrase taken on another device.
    Unreadable,
}

/// Moves the account to `fresh`, resealing with `old` everything sealed to it or for itself
/// (`new_identity`); with no `old`, only an account with nothing sealed yet. The contacts'
/// waiting key changes are followed first, checked against the device's `pins`, so the changes
/// this sends seal to each contact's newest key, which the contact's devices can open. That is
/// best-effort: a contact's mailbox that will not read never holds up a new phrase. In the same
/// write the account's block starts a new key epoch sealed to `fresh` alone
/// (`AccountRecords::epoch_for`), so the old phrase reads nothing written to it after; its
/// earlier epochs are sealed again to `fresh` with the rest. The server seals again what the
/// account sealed to it (`ServerKey::reseal_from`).
pub async fn set_recovery_key(
    connection: &Arc<Connection>,
    account: Uuid,
    old: Option<&RecoverySecret>,
    fresh: &RecoverySecret,
    pins: &HashMap<Uuid, OwnContact>,
) -> Result<Result<(), CannotReseal>, ClientError> {
    set_recovery_key_in(connection, account, old, fresh, pins, RECOVERY_PART_BYTES).await
}

/// About how many bytes of keys sealed again and retirements one write of a new phrase
/// carries: what more there is goes ahead in parts (`stage-recovery-key`), so an account that
/// signed in more logs than one frame holds still takes a new phrase.
const RECOVERY_PART_BYTES: usize = be_block_protocol::MAX_FRAME_BYTES / 4;

/// `set_recovery_key`, with at most about `part_bytes` of what it seals again and retires in
/// each write.
pub(crate) async fn set_recovery_key_in(
    connection: &Arc<Connection>,
    account: Uuid,
    old: Option<&RecoverySecret>,
    fresh: &RecoverySecret,
    pins: &HashMap<Uuid, OwnContact>,
    part_bytes: usize,
) -> Result<Result<(), CannotReseal>, ClientError> {
    if let Some(old) = old
        && let Err(error) = contacts(connection, account, old, pins).await
    {
        eprintln!(
            "be-block-client: a new phrase goes on without following the contacts' key changes: {error}"
        );
    }
    let be_block_protocol::RecoveryKey { sealed, logs, .. } = crate::ask(
        be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
            connection,
            None,
            be_block_protocol::Query::RecoveryKey,
        ),
    )
    .await?;
    let Some(old) = old else {
        if !(sealed.is_empty() && logs.is_empty()) {
            return Ok(Err(CannotReseal::NoOldPhrase));
        }
        crate::ask(be_block_protocol::write::<(), _>(
            connection,
            None,
            be_block_protocol::Op::SetRecoveryKey {
                public: fresh.public().0,
                signing: fresh.signing().0,
                sealed: Vec::new(),
                changes: Vec::new(),
                retirements: Vec::new(),
            },
        ))
        .await?;
        return Ok(Ok(()));
    };
    let block = AccountRecords::open(connection, account, old).await?;
    let records = match &block {
        Some(block) => match block.read().await {
            Ok(content) => records_of(&content.root()),
            Err(ClientError::Locked(_)) => return Ok(Err(CannotReseal::Unreadable)),
            Err(error) => return Err(error),
        },
        None => HashMap::new(),
    };
    let Some(resealed) = new_identity(account, old, fresh, sealed, &records, logs) else {
        return Ok(Err(CannotReseal::Unreadable));
    };
    // The old secret, kept where only the new phrase reads it once the block's keys are sealed
    // again: a contact's key change sealed to the old key before it follows this one opens.
    if let Some(block) = &block {
        let (key, secret) = (retired_key(&old.public()), old.to_bytes().to_vec());
        block
            .update(move |_| {
                answered(vec![be_block::AccountBlock::set_retired(
                    key,
                    Some(secret.clone()),
                )])
            })
            .await?;
    }
    // What one write cannot carry goes ahead in parts, which the last write takes as one.
    let mut parts = recovery_parts(resealed.sealed, resealed.retirements, part_bytes);
    let (sealed, retirements) = parts.pop().unwrap_or_default();
    for (sealed, retirements) in parts {
        crate::ask(be_block_protocol::write::<(), _>(
            connection,
            None,
            be_block_protocol::Op::StageRecoveryKey(be_block_protocol::RecoveryKeyPart {
                public: fresh.public().0,
                signing: fresh.signing().0,
                sealed,
                retirements,
            }),
        ))
        .await?;
    }
    let mut ops = vec![be_block_protocol::Op::SetRecoveryKey {
        public: fresh.public().0,
        signing: fresh.signing().0,
        sealed,
        changes: resealed.changes,
        retirements,
    }];
    let mut organization = None;
    if let Some(block) = &block
        && let Some(keys) = block.epoch_for(fresh).await?
    {
        organization = Some(block.organization());
        ops.push(be_block_protocol::Op::PutKeys(
            be_block_protocol::PutKeys::Rotate {
                roots: vec![block.block()],
                keys,
            },
        ));
    }
    let written = crate::ask(be_block_protocol::write_all::<(), _>(
        connection,
        organization,
        ops,
    ))
    .await;
    // Opened with the old phrase, the block's peer holds the keys this write sealed elsewhere.
    AccountRecords::forget(connection);
    written?;
    Ok(Ok(()))
}

/// One write's share of a new phrase: keys sealed again, and retirements.
type RecoveryPart = (
    Vec<be_block_protocol::SealedKey>,
    Vec<be_block_protocol::Retirement>,
);

/// `sealed` and `retirements` in parts of at most about `budget` bytes each, at least one.
fn recovery_parts(
    sealed: Vec<be_block_protocol::SealedKey>,
    retirements: Vec<be_block_protocol::Retirement>,
    budget: usize,
) -> Vec<RecoveryPart> {
    // Room for each item's ids and framing beside its bytes.
    const OVERHEAD: usize = 64;
    let mut parts: Vec<RecoveryPart> = vec![(Vec::new(), Vec::new())];
    let mut size = 0;
    let start = |size: &mut usize, bytes: usize, parts: &mut Vec<RecoveryPart>| {
        if *size > 0 && *size + bytes > budget {
            parts.push((Vec::new(), Vec::new()));
            *size = 0;
        }
        *size += bytes;
    };
    for key in sealed {
        start(&mut size, key.sealed.len() + OVERHEAD, &mut parts);
        parts.last_mut().expect("one part at least").0.push(key);
    }
    for retirement in retirements {
        start(&mut size, retirement.entry.len() + OVERHEAD, &mut parts);
        parts
            .last_mut()
            .expect("one part at least")
            .1
            .push(retirement);
    }
    parts
}

pub async fn make_link(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    offer: Option<&GrantOffer>,
) -> Result<InviteCode, ClientError> {
    let code = InviteCode::generate();
    make_link_with(connection, identity, me, offer, &code).await?;
    Ok(code)
}

/// `make_link` with `code`, which an invitation shares with the share link it carries
/// (`Peer::make_invitation`).
pub(crate) async fn make_link_with(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    offer: Option<&GrantOffer>,
    code: &InviteCode,
) -> Result<(), ClientError> {
    let grant = offer.map(|offer| PendingGrant {
        organization: offer.organization,
        block: offer.block,
        access: offer.access,
        key: offer.key,
        sealed: code.seal_grant(offer.key.into_bytes(), &offer.secret),
    });
    let kind = LinkKind::Contact(grant);
    let account = Uuid::from_bytes(me.account);
    keep_link(
        connection,
        account,
        identity,
        link_id(code),
        KeptLink {
            code: code.to_bytes().to_vec(),
            kind: kind.clone(),
            made: now_milliseconds(),
        },
    )
    .await?;
    be_block_protocol::write::<(), _>(
        connection,
        None,
        be_block_protocol::Op::CreateLink {
            link: link_id(code),
            card: me.seal(code, CardRole::Owner),
            kind,
            signed: None,
        },
    )
    .await??;
    Ok(())
}

/// Which kind of link a caller opens: the host's Contacts dialog and its join dialog each ask
/// consent for one kind, so a code of the other kind is refused before anything is accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expected {
    Contact,
    Invitation,
}

impl Expected {
    fn refusal(self) -> ClientError {
        ClientError::Refused(
            ErrorCode::InvalidRequest,
            match self {
                Self::Contact => {
                    "That is an invitation into an organization, not a contact link: join it from Organizations."
                }
                Self::Invitation => {
                    "That is a contact link, not an invitation: open it from Contacts."
                }
            }
            .into(),
        )
    }
}

/// A link of the kind `expected`, and its owner's card opened with the code.
pub async fn open_link(
    connection: &Arc<Connection>,
    code: &InviteCode,
    expected: Expected,
) -> Result<(Link, ContactCard), ClientError> {
    let link = be_block_protocol::read::<be_block_protocol::Link, _>(
        connection,
        None,
        be_block_protocol::Query::Link(link_id(code)),
    )
    .await??;
    let kind = match link.kind {
        LinkKind::Contact(_) => Expected::Contact,
        LinkKind::Invitation(_) => Expected::Invitation,
    };
    if kind != expected {
        return Err(expected.refusal());
    }
    let card = ContactCard::open(code, CardRole::Owner, &link.card).map_err(|_| tampered())?;
    if card.account != link.owner.into_bytes() {
        return Err(tampered());
    }
    Ok((link, card))
}

/// The `verified` of `contact`'s record in the account's block, 0 for none or a block that
/// does not open here. A record a link makes follows it, so a contact verified again by a new
/// link outranks every record of it any device of this account pinned, whatever the devices'
/// clocks say: each pinned one was read from the block or written back there (`contacts`).
async fn held_verified(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    contact: Uuid,
) -> Result<u64, ClientError> {
    let Some(block) = AccountRecords::open(connection, account, identity).await? else {
        return Ok(0);
    };
    Ok(match block.read().await {
        Ok(content) => kept_of(&content.root())
            .get(&contact)
            .map_or(0, KeptContact::verified),
        Err(ClientError::Locked(_)) => 0,
        Err(error) => return Err(error),
    })
}

/// Accepts a link of the kind `expected`: a contact link's grant, or an invitation's escrow
/// key, is sealed to `identity` as it is taken. The record it makes of the link's owner follows
/// the newest of what the account's block holds and `pins`, what this device pinned (as
/// `contacts` takes it), so a server that serves this device an older block makes no record it
/// would read as a rollback. Accepting an invitation signs this account into
/// its organization's log, with the key the code derives, after the head the link names; read
/// again and signed again if the log moved meanwhile.
pub async fn accept_link(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    code: &InviteCode,
    expected: Expected,
    pins: &HashMap<Uuid, OwnContact>,
) -> Result<(Link, ContactCard), ClientError> {
    // The card names these keys; the server holds them first, so a record the link's owner
    // signs naming them (a member signed in) checks.
    register_identity(connection, identity).await?;
    let (mut link, card) = open_link(connection, code, expected).await?;
    let after = held_verified(
        connection,
        Uuid::from_bytes(me.account),
        identity,
        Uuid::from_bytes(card.account),
    )
    .await?
    .max(
        pins.get(&Uuid::from_bytes(card.account))
            .map_or(0, |pin| pin.verified),
    );
    let key = match &link.kind {
        LinkKind::Contact(Some(grant)) => {
            let secret = code
                .open_grant(grant.key.into_bytes(), &grant.sealed)
                .map_err(|_| tampered())?;
            let mine = Party {
                account: me.account,
                public: identity.public(),
            };
            identity
                .seal_for(me.account, &mine, grant.key.as_bytes(), &secret)
                .map_err(|_| tampered())?
        }
        LinkKind::Contact(None) => Vec::new(),
        LinkKind::Invitation(invitation) if invitation.escrow.is_empty() => Vec::new(),
        LinkKind::Invitation(invitation) => {
            let opened = code
                .open_escrow(invitation.organization.into_bytes(), &invitation.escrow)
                .ok()
                .and_then(|bytes| Pinned::decode(&bytes))
                .ok_or_else(tampered)?;
            // Pinned as it is taken, after any pin this account held for the organization, so it
            // outranks every older one whatever this device's clock says.
            let after = pinned(
                connection,
                Uuid::from_bytes(me.account),
                identity,
                invitation.organization,
            )
            .await?
            .map_or(0, |held| held.made);
            seal_pin(identity, invitation.organization, &opened.stamped(after))
        }
    };
    let changes = outdated(
        &mailbox(connection).await?,
        Uuid::from_bytes(me.account),
        &Secrets::held(connection, Uuid::from_bytes(me.account), identity).await,
        &card,
    );
    // The log may move between reading the link and accepting it: a few tries, each after the
    // head read last.
    let mut tries = 0;
    loop {
        let member = match (&link.kind, link.head) {
            (LinkKind::Invitation(invitation), Some(head)) => Some(
                be_block_keys::Entry::accepted(
                    code,
                    invitation.organization.into_bytes(),
                    head,
                    me.account,
                    identity,
                )
                .encode(),
            ),
            _ => None,
        };
        let mut ops = vec![be_block_protocol::Op::AcceptLink {
            link: link.id,
            card: me.seal(code, CardRole::Accepter),
            contact: OwnContact::new(card.clone(), after).seal(identity),
            key: key.clone(),
            member,
        }];
        ops.extend(changes.iter().cloned());
        let written = be_block_protocol::write_all::<(), _>(connection, None, ops).await?;
        let Err(error) = written else {
            return Ok((link, card));
        };
        tries += 1;
        let signed_at = link.head;
        if tries >= ACCEPT_TRIES || signed_at.is_none() {
            return Err(error.into());
        }
        let (again, _) = open_link(connection, code, expected).await?;
        if again.head == signed_at {
            return Err(error.into());
        }
        link = again;
    }
}

/// How often `accept_link` signs an invitation's acceptance again after the organization's log
/// moved under it.
const ACCEPT_TRIES: usize = 3;

/// The links this account made that were accepted, each with its accepter's card opened with
/// the code the account's block keeps, recorded in the block as a contact. One whose card does
/// not open, or whose code the block no longer keeps, goes with its letter. A link for another
/// organization waits for a peer of that one. Each record follows the newest of what the block
/// holds and `pins`, as `accept_link`'s does.
pub(crate) async fn finish_links(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    organization: Uuid,
    pins: &HashMap<Uuid, OwnContact>,
) -> Result<Vec<Finished>, ClientError> {
    let held = mailbox(connection).await?;
    let accepted: Vec<(u64, Uuid, Uuid, Vec<u8>)> = held
        .iter()
        .filter_map(|message| match (&message.letter, message.from) {
            (be_block_protocol::Letter::Accepted { link, card }, Some(from)) => {
                Some((message.id, from, *link, card.clone()))
            }
            _ => None,
        })
        .collect();
    if accepted.is_empty() {
        return Ok(Vec::new());
    }
    let Some(block) = AccountRecords::open(connection, account, identity).await? else {
        return Ok(Vec::new());
    };
    let kept: HashMap<Uuid, KeptLink> = match block.read().await {
        Ok(content) => content
            .root()
            .links
            .iter()
            .filter_map(|(link, bytes)| Some((*link, KeptLink::decode(bytes)?)))
            .collect(),
        // A block sealed to a phrase this device lacks: another device finishes them.
        Err(ClientError::Locked(_)) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut finished = Vec::new();
    for (letter, accepter, link, card) in accepted {
        let Some(own) = kept.get(&link) else {
            // Its link was finished already, or was never this account's.
            be_block_protocol::write_all::<(), _>(
                connection,
                None,
                vec![be_block_protocol::Op::DeleteMessage(letter)],
            )
            .await??;
            continue;
        };
        // A card from anyone but the account the letter is from is no card for this link.
        let opened = own
            .code()
            .and_then(|code| ContactCard::open(&code, CardRole::Accepter, &card).ok())
            .filter(|card| card.account == accepter.into_bytes());
        let Some(card) = opened else {
            be_block_protocol::write_all::<(), _>(
                connection,
                None,
                vec![
                    be_block_protocol::Op::DeleteLink { link, signed: None },
                    be_block_protocol::Op::DeleteMessage(letter),
                ],
            )
            .await??;
            forget_link(&block, link).await?;
            continue;
        };
        // Kept in the account's block before the link and its letter go, so a device that stops
        // between keeps the card and finishes the link again.
        let recorded = block
            .update(|content| {
                let before = content
                    .root()
                    .contact(accepter)
                    .and_then(|bytes| KeptContact::decode(bytes, accepter));
                if let Some(KeptContact::Record(before)) = &before
                    && before.through >= letter
                    && before.card == card
                {
                    return answered(Vec::new());
                }
                // Newer than whatever the block held, a removal included.
                let pin = pins.get(&accepter).map_or(0, |pin| pin.verified);
                let record = OwnContact {
                    through: letter,
                    ..OwnContact::new(
                        card.clone(),
                        before.map_or(0, |before| before.verified()).max(pin),
                    )
                };
                answered(vec![be_block::AccountBlock::set_contact(
                    accepter,
                    Some(record.kept()),
                )])
            })
            .await;
        match recorded {
            Ok(()) => {}
            Err(ClientError::Locked(_)) => continue,
            Err(error) => return Err(error),
        }
        let secrets = Secrets::held(connection, account, identity).await;
        let ops = outdated(&held, account, &secrets, &card);
        if !ops.is_empty() {
            be_block_protocol::write_all::<(), _>(connection, None, ops).await??;
        }
        let (grant, invited) = match own.kind.clone() {
            LinkKind::Contact(grant) => (grant, None),
            LinkKind::Invitation(invitation) => (None, Some(invitation.organization)),
        };
        // Finished in the organization it is for: there its grant is sealed, or its accepter
        // signed in.
        let elsewhere = grant
            .as_ref()
            .map(|grant| grant.organization)
            .or(invited)
            .is_some_and(|of| of != organization);
        if elsewhere {
            continue;
        }
        finished.push(Finished {
            card,
            grant,
            invited,
            link,
            letter,
        });
    }
    Ok(finished)
}

/// Drops a finished link's code from the account's block.
pub(crate) async fn forget_link(block: &AccountRecords, link: Uuid) -> Result<(), ClientError> {
    block
        .update(move |content| match content.root().link(link) {
            Some(_) => answered(vec![be_block::AccountBlock::set_link(link, None)]),
            None => answered(Vec::new()),
        })
        .await
}

/// Invites whoever opens the link into `organization`, as an editor: a link any account on
/// the server accepts, once, before it expires. Its code is kept in the account's block, so the
/// organization's invite list can show the link again (`kept_links`). The invitation opens in
/// the organization's log, `checked` as this administrator's device last read it, by the key
/// the code derives, which signs its accepter in (`accept_link`).
pub async fn invite(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    organization: Uuid,
    pinned: &Pinned,
    checked: &be_block_keys::Checked,
) -> Result<InviteCode, ClientError> {
    let code = InviteCode::generate();
    let signed = checked
        .next(
            identity,
            me.account,
            organization.into_bytes(),
            be_block_keys::Record::Invitation(be_block_keys::Invitation {
                key: code.invitation_key(organization.into_bytes()),
                open: true,
            }),
        )
        .ok_or_else(|| {
            ClientError::Refused(
                ErrorCode::PermissionDenied,
                "this account is not served the organization's whole log".into(),
            )
        })?
        .encode();
    let escrow = match pinned.is_empty() {
        true => Vec::new(),
        false => code.seal_escrow(organization.into_bytes(), &pinned.encode()),
    };
    let kind = LinkKind::Invitation(Invitation {
        organization,
        escrow,
    });
    keep_link(
        connection,
        Uuid::from_bytes(me.account),
        identity,
        link_id(&code),
        KeptLink {
            code: code.to_bytes().to_vec(),
            kind: kind.clone(),
            made: now_milliseconds(),
        },
    )
    .await?;
    be_block_protocol::write::<(), _>(
        connection,
        Some(organization),
        be_block_protocol::Op::CreateLink {
            link: link_id(&code),
            card: me.seal(&code, CardRole::Owner),
            kind,
            signed: Some(signed),
        },
    )
    .await??;
    Ok(code)
}

/// An invitation as the holder of its code reads it, signed in or not (a one-shot call): where
/// it stands and, while it is open, its organization's name and its inviter's card opened with
/// the code; an ended one answers its state alone.
pub async fn preview_invitation(
    server_url: &str,
    code: &InviteCode,
) -> Result<(be_block_protocol::InvitationPreview, Option<ContactCard>), ClientError> {
    let id = link_id(code);
    let preview = crate::call_once(server_url, move |peer| {
        be_block_protocol::read::<be_block_protocol::InvitationPreview, _>(
            peer,
            None,
            be_block_protocol::Query::Invitation(id),
        )
    })
    .await?;
    let card = match preview.card.is_empty() {
        true => None,
        false => {
            Some(ContactCard::open(code, CardRole::Owner, &preview.card).map_err(|_| tampered())?)
        }
    };
    Ok((preview, card))
}
