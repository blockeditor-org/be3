use be_graph::Access;
use be_keys::{CardRole, ContactCard, InviteCode, Party, RecoveryPublic, RecoverySecret};
use std::collections::HashMap;

use be_protocol::{
    ContactLink, ErrorCode, EscrowKey, KeyChange, OrganizationInvitation, OrganizationRole,
    OwnKind, OwnSealed, PendingGrant, SealedKey, client,
};
use uuid::Uuid;

use std::sync::Arc;

use crate::{ClientError, connection::Connection};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contact {
    pub account: Uuid,
    pub email: String,
    pub display_name: String,
    pub public: Option<RecoveryPublic>,
}

#[derive(Clone, Debug)]
pub struct GrantOffer {
    pub organization: Uuid,
    pub block: Uuid,
    pub access: Access,
    pub key: Uuid,
    pub secret: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct Finished {
    pub card: ContactCard,
    pub grant: Option<PendingGrant>,
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
        display_name: display_name.to_owned(),
        email: email.to_owned(),
    }
}

pub fn seal_pin(identity: &RecoverySecret, organization: Uuid, escrow: &EscrowKey) -> Vec<u8> {
    identity.seal_pin(organization.into_bytes(), &escrow.encode())
}

pub fn open_pin(identity: &RecoverySecret, organization: Uuid, sealed: &[u8]) -> Option<EscrowKey> {
    if sealed.is_empty() {
        return None;
    }
    EscrowKey::decode(&identity.open_pin(organization.into_bytes(), sealed).ok()?)
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

pub async fn contacts(
    connection: &Arc<Connection>,
    account: Uuid,
    identity: &RecoverySecret,
) -> Result<Vec<Contact>, ClientError> {
    let contacts = client::contacts::list_contacts(connection).await??;
    let mut held = None;
    let mut listed = Vec::with_capacity(contacts.len());
    for row in contacts {
        let mut card = ContactCard::open_own(identity, &row.sealed)
            .ok()
            .filter(|card| card.account == row.account.into_bytes());
        if let Some(verified) = &mut card
            && let Some(public) = changed_key(account, identity, verified, &row.changes)
        {
            if held.is_none() {
                held = Some(sealed_to_me(connection).await?);
            }
            reseal_from(
                connection,
                account,
                identity,
                verified,
                held.as_deref().unwrap_or_default(),
            )
            .await?;
            verified.public = public;
            client::contacts::put_contact(connection, row.account, verified.seal_own(identity))
                .await??;
        }
        listed.push(Contact {
            public: card.map(|card| card.public),
            account: row.account,
            email: row.email,
            display_name: row.display_name,
        });
    }
    Ok(listed)
}

fn changed_key(
    account: Uuid,
    identity: &RecoverySecret,
    card: &ContactCard,
    changes: &[Vec<u8>],
) -> Option<RecoveryPublic> {
    let mut current = card.public;
    for sealed in changes {
        let from = Party {
            account: card.account,
            public: current,
        };
        if let Ok(next) = identity.open_key_change(&from, account.into_bytes(), sealed) {
            current = next;
        }
    }
    (current != card.public).then_some(current)
}

async fn sealed_to_me(connection: &Arc<Connection>) -> Result<Vec<SealedKey>, ClientError> {
    Ok(client::keys::get_recovery_key(connection).await??.sealed)
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
        client::keys::reseal_keys(connection, keys).await??;
    }
    Ok(())
}

pub struct NewIdentity {
    pub sealed: Vec<SealedKey>,
    pub own: Vec<OwnSealed>,
    pub changes: Vec<KeyChange>,
}

pub fn new_identity(
    account: Uuid,
    old: &RecoverySecret,
    fresh: &RecoverySecret,
    held: Vec<SealedKey>,
    own: Vec<OwnSealed>,
) -> Option<NewIdentity> {
    let me = Party {
        account: account.into_bytes(),
        public: old.public(),
    };
    let contacts: HashMap<Uuid, Party> = own
        .iter()
        .filter(|held| held.kind == OwnKind::Contact)
        .filter_map(|held| {
            let card = ContactCard::open_own(old, &held.sealed).ok()?;
            (card.account == held.id.into_bytes()).then_some((
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
            sealed: old.seal_key_change(account.into_bytes(), party, fresh.public()),
        })
        .collect();
    Some(NewIdentity {
        sealed,
        own,
        changes,
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
    client::contacts::create_contact_link(
        connection,
        link_id(&code),
        me.seal(&code, CardRole::Owner),
        identity.seal_own(&code.to_bytes()),
        grant,
    )
    .await??;
    Ok(code)
}

pub async fn open_link(
    connection: &Arc<Connection>,
    code: &InviteCode,
) -> Result<(ContactLink, ContactCard), ClientError> {
    let link = client::contacts::get_contact_link(connection, link_id(code)).await??;
    let card = ContactCard::open(code, CardRole::Owner, &link.card).map_err(|_| tampered())?;
    if card.account != link.owner.into_bytes() {
        return Err(tampered());
    }
    Ok((link, card))
}

pub async fn accept_link(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    code: &InviteCode,
) -> Result<ContactCard, ClientError> {
    let (link, card) = open_link(connection, code).await?;
    let key = match &link.grant {
        Some(grant) => {
            let secret = code
                .open_grant(grant.key.into_bytes(), &grant.sealed)
                .map_err(|_| tampered())?;
            let mine = Party {
                account: me.account,
                public: identity.public(),
            };
            identity.seal_for(me.account, &mine, grant.key.as_bytes(), &secret)
        }
        None => Vec::new(),
    };
    client::contacts::accept_contact_link(
        connection,
        link.id,
        me.seal(code, CardRole::Accepter),
        card.seal_own(identity),
        key,
    )
    .await??;
    Ok(card)
}

pub async fn finish_links(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    organization: Uuid,
) -> Result<Vec<Finished>, ClientError> {
    let links = client::contacts::list_contact_links(connection).await??;
    let mut finished = Vec::new();
    for link in links {
        let Some(accepter) = link.accepter else {
            continue;
        };
        let opened = identity
            .open_own(&link.code)
            .ok()
            .and_then(|code| InviteCode::from_bytes(&code).ok())
            .and_then(|code| ContactCard::open(&code, CardRole::Accepter, &link.card).ok())
            .filter(|card| card.account == accepter.into_bytes());
        let mut kept = opened.is_some();
        if let Some(card) = &opened {
            let put = crate::ask(client::contacts::put_contact(
                connection,
                accepter,
                card.seal_own(identity),
            ))
            .await;
            match put {
                Ok(_) => {}
                Err(ClientError::Refused(ErrorCode::NotAContact, _)) => kept = false,
                Err(error) => return Err(error),
            }
        }
        let elsewhere = link
            .grant
            .as_ref()
            .is_some_and(|grant| grant.organization != organization);
        if kept && elsewhere {
            continue;
        }
        client::contacts::delete_contact_link(connection, link.id).await??;
        if let Some(card) = opened.filter(|_| kept) {
            finished.push(Finished {
                card,
                grant: link.grant,
            });
        }
    }
    Ok(finished)
}

pub async fn remove_contact(
    connection: &Arc<Connection>,
    account: Uuid,
) -> Result<(), ClientError> {
    client::contacts::remove_contact(connection, account).await??;
    Ok(())
}

pub async fn invite(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    organization: Uuid,
    email: String,
    role: OrganizationRole,
    escrow: Option<&EscrowKey>,
) -> Result<InviteCode, ClientError> {
    let code = InviteCode::generate();
    let escrow = escrow.map_or_else(Vec::new, |escrow| {
        code.seal_escrow(organization.into_bytes(), &escrow.encode())
    });
    client::organizations::invite(
        connection,
        link_id(&code),
        organization,
        email,
        role,
        me.seal(&code, CardRole::Owner),
        identity.seal_own(&code.to_bytes()),
        escrow,
    )
    .await??;
    Ok(code)
}

pub async fn invitation(
    connection: &Arc<Connection>,
    code: &InviteCode,
) -> Result<(OrganizationInvitation, ContactCard), ClientError> {
    let invitation = client::organizations::get_invitation(connection, link_id(code)).await??;
    let card =
        ContactCard::open(code, CardRole::Owner, &invitation.sealed).map_err(|_| tampered())?;
    if card.account != invitation.invited_by.into_bytes() {
        return Err(tampered());
    }
    Ok((invitation, card))
}

pub async fn accept_invitation(
    connection: &Arc<Connection>,
    identity: &RecoverySecret,
    me: &ContactCard,
    code: &InviteCode,
) -> Result<OrganizationInvitation, ClientError> {
    let (invitation, card) = invitation(connection, code).await?;
    let escrow = match invitation.escrow.is_empty() {
        true => Vec::new(),
        false => {
            let opened = code
                .open_escrow(invitation.organization.into_bytes(), &invitation.escrow)
                .ok()
                .and_then(|bytes| EscrowKey::decode(&bytes))
                .ok_or_else(tampered)?;
            seal_pin(identity, invitation.organization, &opened)
        }
    };
    client::organizations::accept_invitation(
        connection,
        invitation.id,
        me.seal(code, CardRole::Accepter),
        card.seal_own(identity),
        escrow,
    )
    .await??;
    Ok(invitation)
}
