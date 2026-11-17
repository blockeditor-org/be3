use be_block_graph::Access;
use be_block_keys::{
    CardRole, ContactCard, InviteCode, Party, PublicIdentity, RecoveryPublic, RecoverySecret,
    SigningPublic,
};
use std::collections::HashMap;

use be_block_protocol::{
    ErrorCode, EscrowKey, Invitation, KeyChange, Link, LinkKind, OwnKind, OwnSealed, PendingGrant,
    SealedKey,
};
use uuid::Uuid;

use std::sync::Arc;

use crate::{ClientError, connection::Connection};

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
        })
    }
}

/// A contact as this account verified it, sealed to itself: the card, and when it was verified
/// or its key last changed, a sequence that only grows. The server keeps it but cannot make one;
/// what it can do is serve an older one, so each device pins the newest it has seen and refuses
/// any older (plans/crypto_audit.md, 4).
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct OwnContact {
    pub card: ContactCard,
    pub verified: u64,
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
        }
    }

    pub fn seal(&self, identity: &RecoverySecret) -> Vec<u8> {
        identity.seal_own(&postcard::to_stdvec(self).expect("a contact record always encodes"))
    }

    /// The record sealed in `sealed`, if it opens and names `account`.
    pub fn open(identity: &RecoverySecret, sealed: &[u8], account: Uuid) -> Option<Self> {
        let record: Self = postcard::from_bytes(&identity.open_own(sealed).ok()?).ok()?;
        (record.card.account == account.into_bytes()).then_some(record)
    }
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
            own: Vec::new(),
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
    crate::ask(be_block_protocol::read::<Vec<be_block_protocol::Message>, _>(
        connection,
        None,
        be_block_protocol::Query::Mailbox,
    ))
    .await
}

/// The account's contacts as the server lists them, each checked against `pinned`, the newest
/// record of it this device has seen: an older record the server serves is refused for the
/// pinned one, and the contact listed with the mismatch for as long as the server serves it (only
/// a server that misbehaves does, so writing the pinned one back would change nothing). Key
/// changes in the mailbox are followed from the record kept, in order, and deleted once the
/// record that follows them is written; one that does not open from it (followed already, or
/// not the contact's) goes too, unless this device could not read the served record, as after a
/// new phrase taken on another device, whose identity those changes may be sealed to. Each record
/// is written only over the one it was read as (`put-contact`'s `replaces`), so a device that read
/// fewer changes than another never rolls the other's record back: refused, it reads again.
pub async fn contacts(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    pinned: &HashMap<Uuid, OwnContact>,
) -> Result<Vec<Contact>, ClientError> {
    let mut tries = 0;
    loop {
        match contacts_once(connection, account, identity, pinned).await {
            Err(ClientError::Refused(ErrorCode::RecordChanged, _)) if tries < 3 => tries += 1,
            listed => return listed,
        }
    }
}

async fn contacts_once(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
    pinned: &HashMap<Uuid, OwnContact>,
) -> Result<Vec<Contact>, ClientError> {
    let contacts = be_block_protocol::read::<Vec<be_block_protocol::ContactRow>, _>(
        connection,
        None,
        be_block_protocol::Query::Contacts,
    )
    .await??;
    let mut changes: HashMap<Uuid, Vec<(u64, Vec<u8>)>> = HashMap::new();
    for message in mailbox(connection).await? {
        if let (Some(from), be_block_protocol::Letter::KeyChange(sealed)) =
            (message.from, message.letter)
        {
            changes.entry(from).or_default().push((message.id, sealed));
        }
    }
    let mut held = None;
    let mut listed = Vec::with_capacity(contacts.len());
    for row in contacts {
        let served = OwnContact::open(identity, &row.sealed, row.account);
        let pin = pinned.get(&row.account);
        let mismatch = match (&served, pin) {
            (_, None) => None,
            (None, Some(_)) => Some(Mismatch::Unreadable),
            (Some(served), Some(pin)) => {
                (served.verified < pin.verified).then_some(Mismatch::Older)
            }
        };
        let mut record = match mismatch {
            Some(_) => pin.cloned(),
            None => served,
        };
        let letters = changes.remove(&row.account).unwrap_or_default();
        let sealed: Vec<Vec<u8>> = letters.iter().map(|(_, sealed)| sealed.clone()).collect();
        let mut put = false;
        if let Some(record) = &mut record
            && let Some(changed) = changed_key(account, identity, &record.card, &sealed)
        {
            if held.is_none() {
                held = Some(sealed_to_me(connection).await?);
            }
            reseal_from(
                connection,
                account,
                identity,
                &record.card,
                held.as_deref().unwrap_or_default(),
            )
            .await?;
            let mut card = record.card.clone();
            card.public = changed.public;
            card.signing = changed.signing;
            *record = OwnContact::new(card, record.verified);
            put = true;
        }
        let mut ops = Vec::new();
        if let Some(record) = record.as_ref().filter(|_| put) {
            ops.push(be_block_protocol::Op::PutContact {
                contact: row.account,
                sealed: record.seal(identity),
                replaces: row.sealed.clone(),
            });
        }
        if record.is_some() && mismatch != Some(Mismatch::Unreadable) {
            ops.extend(
                letters
                    .iter()
                    .map(|(id, _)| be_block_protocol::Op::DeleteMessage(*id)),
            );
        }
        if !ops.is_empty() {
            be_block_protocol::write_all::<(), _>(connection, None, ops).await??;
        }
        listed.push(match record {
            Some(record) => Contact {
                account: row.account,
                email: record.card.email,
                display_name: record.card.display_name,
                public: Some(record.card.public),
                signing: Some(record.card.signing),
                verified: record.verified,
                mismatch,
            },
            None => Contact {
                account: row.account,
                email: row.email,
                display_name: row.display_name,
                public: None,
                signing: None,
                verified: 0,
                mismatch: None,
            },
        });
    }
    Ok(listed)
}

fn changed_key(
    account: Uuid,
    identity: &RecoverySecret,
    card: &ContactCard,
    changes: &[Vec<u8>],
) -> Option<PublicIdentity> {
    let held = PublicIdentity {
        public: card.public,
        signing: card.signing,
    };
    let mut current = held;
    for sealed in changes {
        let from = Party {
            account: card.account,
            public: current.public,
        };
        if let Ok(next) = identity.open_key_change(&from, account.into_bytes(), sealed) {
            current = next;
        }
    }
    (current != held).then_some(current)
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
    pub own: Vec<OwnSealed>,
    pub changes: Vec<KeyChange>,
    /// An entry retiring the old keys for the new ones in each access log the account signed
    /// in (guides/blocks/keys.md, "Identity, new phrases and devices").
    pub retirements: Vec<be_block_protocol::Retirement>,
}

pub fn new_identity(
    account: Uuid,
    old: &RecoverySecret,
    fresh: &RecoverySecret,
    held: Vec<SealedKey>,
    own: Vec<OwnSealed>,
    logs: Vec<be_block_protocol::SignedLog>,
) -> Option<NewIdentity> {
    let me = Party {
        account: account.into_bytes(),
        public: old.public(),
    };
    let contacts: HashMap<Uuid, Party> = own
        .iter()
        .filter(|held| held.kind == OwnKind::Contact)
        .filter_map(|held| {
            let card = OwnContact::open(old, &held.sealed, held.id)?.card;
            Some((
                held.id,
                Party {
                    account: card.account,
                    public: card.public,
                },
            ))
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
    let own = own
        .into_iter()
        .map(|held| {
            let plain = old.open_own(&held.sealed).ok()?;
            Some(OwnSealed {
                sealed: fresh.seal_own(&plain),
                ..held
            })
        })
        .collect::<Option<Vec<_>>>()?;
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
        own,
        changes,
        retirements,
    })
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
    be_block_protocol::write::<(), _>(
        connection,
        None,
        be_block_protocol::Op::CreateLink {
            link: link_id(&code),
            card: me.seal(&code, CardRole::Owner),
            code: identity.seal_own(&code.to_bytes()),
            kind: LinkKind::Contact(grant),
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

/// The `verified` of each contact's record the server holds, by contact. A record a link
/// makes follows it, so a contact verified again by a new link outranks every record of it any
/// device of this account pinned, whatever the devices' clocks say: each pinned one was served
/// or written back there (`contacts`).
async fn served_verified(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
) -> Result<HashMap<Uuid, u64>, ClientError> {
    Ok(served(connection)
        .await?
        .into_iter()
        .filter_map(|(account, sealed)| {
            let record = OwnContact::open(identity, &sealed, account)?;
            Some((account, record.verified))
        })
        .collect())
}

/// Each contact's record as the server holds it, sealed, by contact.
async fn served(connection: &Arc<Connection>) -> Result<HashMap<Uuid, Vec<u8>>, ClientError> {
    Ok(
        be_block_protocol::read::<Vec<be_block_protocol::ContactRow>, _>(
            connection,
            None,
            be_block_protocol::Query::Contacts,
        )
        .await??
        .into_iter()
        .map(|row| (row.account, row.sealed))
        .collect(),
    )
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
    let after = served_verified(connection, identity)
        .await?
        .get(&Uuid::from_bytes(card.account))
        .copied()
        .unwrap_or_default();
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
    be_block_protocol::write::<(), _>(
        connection,
        None,
        be_block_protocol::Op::AcceptLink {
            link: link.id,
            card: me.seal(code, CardRole::Accepter),
            contact: OwnContact::new(card.clone(), after).seal(identity),
            key: key,
        },
    )
    .await??;
    Ok((link, card))
}

pub(crate) async fn finish_links(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    organization: Uuid,
) -> Result<Vec<Finished>, ClientError> {
    let links = be_block_protocol::read::<Vec<be_block_protocol::OwnLink>, _>(
        connection,
        None,
        be_block_protocol::Query::Links,
    )
    .await??;
    // Each accepted link's card, from the mailbox: by link, the message and its sender.
    let mut cards: HashMap<Uuid, (u64, Uuid, Vec<u8>)> = HashMap::new();
    let mut stray = Vec::new();
    for message in mailbox(connection).await? {
        if let (Some(from), be_block_protocol::Letter::Accepted { link, card }) =
            (message.from, message.letter)
        {
            match links.iter().any(|own| own.id == link) {
                true => {
                    cards.insert(link, (message.id, from, card));
                }
                // Its link was finished, or ended, already.
                false => stray.push(be_block_protocol::Op::DeleteMessage(message.id)),
            }
        }
    }
    if !stray.is_empty() {
        be_block_protocol::write_all::<(), _>(connection, None, stray).await??;
    }
    let served = match links.iter().any(|link| link.accepter.is_some()) {
        true => served(connection).await?,
        false => HashMap::new(),
    };
    let mut finished = Vec::new();
    for link in links {
        let Some(accepter) = link.accepter else {
            continue;
        };
        // A card from anyone but the accepter the server names is no card for this link.
        let (letter, card) = match cards.remove(&link.id) {
            Some((letter, from, card)) if from == accepter => (Some(letter), card),
            Some((letter, ..)) => (Some(letter), Vec::new()),
            None => (None, Vec::new()),
        };
        let opened = identity
            .open_own(&link.code)
            .ok()
            .and_then(|code| InviteCode::from_bytes(&code).ok())
            .and_then(|code| ContactCard::open(&code, CardRole::Accepter, &card).ok())
            .filter(|card| card.account == accepter.into_bytes());
        let mut kept = opened.is_some();
        if let Some(card) = &opened {
            let put = crate::ask(be_block_protocol::write::<(), _>(
                connection,
                None,
                be_block_protocol::Op::PutContact {
                    contact: accepter,
                    sealed: OwnContact::new(
                        card.clone(),
                        served
                            .get(&accepter)
                            .and_then(|sealed| OwnContact::open(identity, sealed, accepter))
                            .map_or(0, |record| record.verified),
                    )
                    .seal(identity),
                    replaces: served.get(&accepter).cloned().unwrap_or_default(),
                },
            ))
            .await;
            match put {
                Ok(_) => {}
                Err(ClientError::Refused(ErrorCode::NotAContact, _)) => kept = false,
                Err(error) => return Err(error),
            }
        }
        let (grant, invited) = match link.kind {
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
        if kept && elsewhere {
            continue;
        }
        match (opened.filter(|_| kept), letter) {
            (Some(card), Some(letter)) => finished.push(Finished {
                card,
                grant,
                invited,
                link: link.id,
                letter,
            }),
            (_, letter) => {
                let mut ops = vec![be_block_protocol::Op::DeleteLink(link.id)];
                ops.extend(letter.map(be_block_protocol::Op::DeleteMessage));
                be_block_protocol::write_all::<(), _>(connection, None, ops).await??
            }
        }
    }
    Ok(finished)
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
    be_block_protocol::write::<(), _>(
        connection,
        Some(organization),
        be_block_protocol::Op::CreateLink {
            link: link_id(&code),
            card: me.seal(&code, CardRole::Owner),
            code: identity.seal_own(&code.to_bytes()),
            kind: LinkKind::Invitation(Invitation {
                organization,
                escrow,
            }),
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

/// The code of an invitation in the invite list, which only its inviter's identity opens.
pub fn invitation_link_code(identity: &RecoverySecret, sealed: &[u8]) -> Option<InviteCode> {
    identity
        .open_own(sealed)
        .ok()
        .and_then(|code| InviteCode::from_bytes(&code).ok())
}
