use be_block_graph::Access;
use be_block_keys::{
    CardRole, ContactCard, InviteCode, Party, PublicIdentity, RecoveryPublic, RecoverySecret,
    SigningPublic,
};
use std::collections::HashMap;

use be_block_protocol::{
    ErrorCode, EscrowKey, Invitation, KeyChange, Link, LinkKind, PendingGrant, SealedKey,
};
use uuid::Uuid;

use std::sync::Arc;

use crate::{AccountRecords, ClientError, connection::Connection};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Contact {
    pub account: Uuid,
    /// The name and email on the card this account verified, never the server's row, which
    /// could name one contact as another (plans/crypto_audit.md, 5). A contact whose card has
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
/// pins the newest record it has seen and refuses any older (plans/crypto_audit.md, 4).
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
        identity.seal_own(&self.encode())
    }

    /// The record sealed in `sealed`, if it opens and names `account`.
    pub fn open(identity: &RecoverySecret, sealed: &[u8], account: Uuid) -> Option<Self> {
        Self::decode(&identity.open_own(sealed).ok()?, account)
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

/// The records the account's block holds, by contact.
fn records_of(block: &be_block::AccountBlock) -> HashMap<Uuid, OwnContact> {
    block
        .contacts
        .iter()
        .filter_map(|(account, bytes)| Some((*account, OwnContact::decode(bytes, *account)?)))
        .collect()
}

#[derive(Clone, Debug)]
pub struct GrantOffer {
    pub organization: Uuid,
    pub block: Uuid,
    pub access: Access,
    pub key: Uuid,
    /// The scope key as it travels (`be_block_keys::CarriedKey`), with the block and its root.
    pub secret: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Finished {
    pub card: ContactCard,
    pub grant: Option<PendingGrant>,
    /// For an invitation, the organization its accepter joined, which an administrator's device
    /// signs them into (`Peer::finish_links`).
    pub invited: Option<Uuid>,
    /// The link, which `Peer::finish_links` deletes once it signed the grant or the member in,
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
/// organization's access log root and its escrow key.
#[derive(Clone, Debug, Default, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct Pinned {
    pub root: Option<be_block_keys::EntryHash>,
    pub escrow: Option<EscrowKey>,
}

impl Pinned {
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
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct KeptLink {
    pub code: Vec<u8>,
    pub kind: LinkKind,
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
        Ok(held) => held
            .content
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
        .update(move |_| vec![be_block::AccountBlock::set_link(link, Some(bytes.clone()))])
        .await
}

/// What the account pins for `organization`: from its block, or from a letter not yet taken
/// into it (an agent, which has no block, keeps the one its maker left). The newest letter wins
/// over the block, as it came with the account joining or making the organization.
pub async fn pinned(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    organization: Uuid,
) -> Result<Option<Pinned>, ClientError> {
    let lettered = mailbox(connection)
        .await?
        .into_iter()
        .rev()
        .find_map(|message| match message.letter {
            be_block_protocol::Letter::Pinned {
                organization: of,
                sealed,
            } if of == organization => open_pin(identity, organization, &sealed),
            _ => None,
        });
    if lettered.is_some() {
        return Ok(lettered);
    }
    let Some(block) = AccountRecords::open(connection, account, identity).await? else {
        return Ok(None);
    };
    Ok(match block.read().await {
        Ok(held) => held
            .content
            .root()
            .pin(organization)
            .and_then(Pinned::decode),
        Err(ClientError::Locked(_)) => None,
        Err(error) => return Err(error),
    })
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
    let bytes = pinned.encode();
    block
        .update(move |_| vec![be_block::AccountBlock::set_pin(organization, Some(bytes.clone()))])
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
    };
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
        false => identity.seal_pin(organization.into_bytes(), &pinned.encode()),
    }
}

pub fn open_pin(identity: &RecoverySecret, organization: Uuid, sealed: &[u8]) -> Option<Pinned> {
    if sealed.is_empty() {
        return None;
    }
    Pinned::decode(&identity.open_pin(organization.into_bytes(), sealed).ok()?)
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
    let mut tries = 0;
    loop {
        match contacts_once(connection, account, identity, pinned, block.as_ref()).await {
            Err(ClientError::Contended) if tries < 3 => tries += 1,
            listed => return listed,
        }
    }
}

async fn contacts_once(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    pinned: &HashMap<Uuid, OwnContact>,
    block: Option<&AccountRecords>,
) -> Result<Vec<Contact>, ClientError> {
    let held = match block {
        Some(block) => match block.read().await {
            Ok(held) => Some(held),
            Err(ClientError::Locked(_)) => None,
            Err(error) => return Err(error),
        },
        None => None,
    };
    let readable = held.is_some() || block.is_none();
    // Letters are taken, and deleted, only into a block this device reads.
    let keeps = held.is_some();
    let read = held
        .as_ref()
        .map(|held| records_of(&held.content.root()))
        .unwrap_or_default();
    let mut records = match readable {
        true => read.clone(),
        false => pinned.clone(),
    };
    let mailbox = mailbox(connection).await?;
    let mut taken = Vec::new();
    let mut pins = Vec::new();
    let mut changes: HashMap<Uuid, Vec<(u64, Vec<u8>)>> = HashMap::new();
    for message in &mailbox {
        let Some(from) = message.from else {
            continue;
        };
        let through = records.get(&from).map_or(0, |record| record.through);
        match &message.letter {
            be_block_protocol::Letter::Contact(sealed) => {
                if message.id > through
                    && let Some(mut record) = OwnContact::open(identity, sealed, from)
                    && records
                        .get(&from)
                        .is_none_or(|held| held.verified < record.verified)
                {
                    record.through = message.id;
                    records.insert(from, record);
                }
                taken.push(message.id);
            }
            be_block_protocol::Letter::Removed => {
                if message.id > through {
                    records.remove(&from);
                }
                taken.push(message.id);
            }
            be_block_protocol::Letter::Pinned {
                organization,
                sealed,
            } => {
                if let Some(pinned) = open_pin(identity, *organization, sealed) {
                    pins.push(be_block::AccountBlock::set_pin(
                        *organization,
                        Some(pinned.encode()),
                    ));
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
            (Some(pin), true) => (records[&contact].verified < pin.verified).then_some(Mismatch::Older),
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
        let (changed, followed) = follow(account, identity, &record.card, &sealed);
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
    if let (Some(block), Some(held), true) = (block, held, keeps) {
        let mut edits = pins;
        for (contact, record) in &records {
            if read.get(contact) != Some(record) {
                edits.push(be_block::AccountBlock::set_contact(
                    *contact,
                    Some(record.encode()),
                ));
            }
        }
        for contact in read.keys().filter(|contact| !records.contains_key(contact)) {
            edits.push(be_block::AccountBlock::set_contact(*contact, None));
        }
        block.save(held, &edits).await?;
        if !taken.is_empty() {
            taken.sort_unstable();
            taken.dedup();
            be_block_protocol::write_all::<(), _>(
                connection,
                None,
                taken
                    .into_iter()
                    .map(be_block_protocol::Op::DeleteMessage)
                    .collect(),
            )
            .await??;
        }
    }
    Ok(listed)
}

/// Follows `changes` in order from `card`'s key: the key they lead to, if another, and how many
/// of them lie up to the last that opened, which the device may delete.
fn follow(
    account: Uuid,
    identity: &RecoverySecret,
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
        if let Ok(next) = identity.open_key_change(&from, account.into_bytes(), sealed) {
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
    identity: &RecoverySecret,
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
            identity
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
                sealed: identity.seal_for(account.into_bytes(), &mine, key.key.as_bytes(), &plain),
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
                sealed: fresh.seal_for(account.into_bytes(), &mine, key.key.as_bytes(), &plain),
                sender: account,
                ..key
            })
        })
        .collect();
    let changes = contacts
        .iter()
        .map(|(contact, party)| KeyChange {
            contact: *contact,
            sealed: old.seal_key_change(account.into_bytes(), party, fresh.identity()),
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
    if let Some(old) = old
        && let Err(error) = contacts(connection, account, old, pins).await
    {
        eprintln!(
            "be-block-client: a new phrase goes on without following the contacts' key changes: {error}"
        );
    }
    let be_block_protocol::RecoveryKey { sealed, logs, .. } = crate::ask(be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
        connection,
        None,
        be_block_protocol::Query::RecoveryKey,
    ))
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
            Ok(held) => records_of(&held.content.root()),
            Err(ClientError::Locked(_)) => return Ok(Err(CannotReseal::Unreadable)),
            Err(error) => return Err(error),
        },
        None => HashMap::new(),
    };
    let Some(resealed) = new_identity(account, old, fresh, sealed, &records, logs) else {
        return Ok(Err(CannotReseal::Unreadable));
    };
    let mut ops = vec![be_block_protocol::Op::SetRecoveryKey {
        public: fresh.public().0,
        signing: fresh.signing().0,
        sealed: resealed.sealed,
        changes: resealed.changes,
        retirements: resealed.retirements,
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
    crate::ask(be_block_protocol::write_all::<(), _>(
        connection,
        organization,
        ops,
    ))
    .await?;
    Ok(Ok(()))
}

pub async fn make_link(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    offer: Option<&GrantOffer>,
) -> Result<InviteCode, ClientError> {
    let code = InviteCode::generate();
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
        link_id(&code),
        KeptLink {
            code: code.to_bytes().to_vec(),
            kind: kind.clone(),
        },
    )
    .await?;
    be_block_protocol::write::<(), _>(
        connection,
        None,
        be_block_protocol::Op::CreateLink {
            link: link_id(&code),
            card: me.seal(&code, CardRole::Owner),
            kind,
        },
    )
    .await??;
    Ok(code)
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
        Ok(held) => records_of(&held.content.root())
            .get(&contact)
            .map_or(0, |record| record.verified),
        Err(ClientError::Locked(_)) => 0,
        Err(error) => return Err(error),
    })
}

/// Accepts a link of the kind `expected`: a contact link's grant, or an invitation's escrow
/// key, is sealed to `identity` as it is taken.
pub async fn accept_link(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    code: &InviteCode,
    expected: Expected,
) -> Result<(Link, ContactCard), ClientError> {
    // The card names these keys; the server holds them first, so a record the link's owner
    // signs naming them (a member signed in) checks.
    register_identity(connection, identity).await?;
    let (link, card) = open_link(connection, code, expected).await?;
    let after = held_verified(
        connection,
        Uuid::from_bytes(me.account),
        identity,
        Uuid::from_bytes(card.account),
    )
    .await?;
    let key = match &link.kind {
        LinkKind::Contact(Some(grant)) => {
            let secret = code
                .open_grant(grant.key.into_bytes(), &grant.sealed)
                .map_err(|_| tampered())?;
            let mine = Party {
                account: me.account,
                public: identity.public(),
            };
            identity.seal_for(me.account, &mine, grant.key.as_bytes(), &secret)
        }
        LinkKind::Contact(None) => Vec::new(),
        LinkKind::Invitation(invitation) if invitation.escrow.is_empty() => Vec::new(),
        LinkKind::Invitation(invitation) => {
            let opened = code
                .open_escrow(invitation.organization.into_bytes(), &invitation.escrow)
                .ok()
                .and_then(|bytes| Pinned::decode(&bytes))
                .ok_or_else(tampered)?;
            seal_pin(identity, invitation.organization, &opened)
        }
    };
    let mut ops = vec![be_block_protocol::Op::AcceptLink {
        link: link.id,
        card: me.seal(code, CardRole::Accepter),
        contact: OwnContact::new(card.clone(), after).seal(identity),
        key: key,
    }];
    ops.extend(outdated(
        &mailbox(connection).await?,
        Uuid::from_bytes(me.account),
        identity,
        &card,
    ));
    be_block_protocol::write_all::<(), _>(connection, None, ops).await??;
    Ok((link, card))
}

/// The links this account made that were accepted, each with its accepter's card opened with
/// the code the account's block keeps, recorded in the block as a contact. One whose card does
/// not open, or whose code the block no longer keeps, goes with its letter. A link for another
/// organization waits for a peer of that one.
pub(crate) async fn finish_links(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    organization: Uuid,
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
        Ok(held) => held
            .content
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
                    be_block_protocol::Op::DeleteLink(link),
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
            .update(|current| {
                let before = current
                    .contact(accepter)
                    .and_then(|bytes| OwnContact::decode(bytes, accepter));
                if before
                    .as_ref()
                    .is_some_and(|before| before.through >= letter && before.card == card)
                {
                    return Vec::new();
                }
                let record = OwnContact {
                    through: letter,
                    ..OwnContact::new(card.clone(), before.map_or(0, |before| before.verified))
                };
                vec![be_block::AccountBlock::set_contact(
                    accepter,
                    Some(record.encode()),
                )]
            })
            .await;
        match recorded {
            Ok(()) => {}
            Err(ClientError::Locked(_)) => continue,
            Err(error) => return Err(error),
        }
        let ops = outdated(&held, account, identity, &card);
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
        .update(move |current| match current.link(link) {
            Some(_) => vec![be_block::AccountBlock::set_link(link, None)],
            None => Vec::new(),
        })
        .await
}

/// Invites whoever opens the link into `organization`, as an editor: a link any account on
/// the server accepts, once, before it expires. Its code is sealed to the inviter too, so the
/// organization's invite list can show the link again (`invitation_link_code`).
pub async fn invite(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    organization: Uuid,
    pinned: &Pinned,
) -> Result<InviteCode, ClientError> {
    let code = InviteCode::generate();
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

