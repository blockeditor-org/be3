use be_graph::Access;
use be_keys::{CardRole, ContactCard, InviteCode, Party, RecoveryPublic, RecoverySecret};
use be_protocol::{
    ClientMessage, ContactLink, ErrorCode, EscrowKey, OrganizationInvitation, OrganizationRole,
    PendingGrant, ServerMessage,
};
use uuid::Uuid;

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

pub async fn verified_escrow(
    connection: &Connection,
    identity: &RecoverySecret,
    organization: Uuid,
) -> Result<Option<EscrowKey>, ClientError> {
    let ServerMessage::OrganizationOpened {
        escrows, pinned, ..
    } = connection
        .request(|request| ClientMessage::OpenOrganization {
            request,
            organization,
        })
        .await?
    else {
        return Err(ClientError::Unexpected);
    };
    let pinned = open_pin(identity, organization, &pinned);
    Ok(crate::keys::verify_escrow(
        organization,
        pinned.as_ref(),
        &escrows,
        |_| false,
    ))
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
    connection: &Connection,
    identity: &RecoverySecret,
) -> Result<Vec<Contact>, ClientError> {
    let ServerMessage::Contacts { contacts, .. } = connection
        .request(|request| ClientMessage::ListContacts { request })
        .await?
    else {
        return Err(ClientError::Unexpected);
    };
    Ok(contacts
        .into_iter()
        .map(|row| Contact {
            public: ContactCard::open_own(identity, &row.sealed)
                .ok()
                .filter(|card| card.account == row.account.into_bytes())
                .map(|card| card.public),
            account: row.account,
            email: row.email,
            display_name: row.display_name,
        })
        .collect())
}

pub async fn make_link(
    connection: &Connection,
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
    connection
        .request(|request| ClientMessage::CreateContactLink {
            request,
            link: link_id(&code),
            card: me.seal(&code, CardRole::Owner),
            code: identity.seal_own(&code.to_bytes()),
            grant,
        })
        .await?;
    Ok(code)
}

pub async fn open_link(
    connection: &Connection,
    code: &InviteCode,
) -> Result<(ContactLink, ContactCard), ClientError> {
    let ServerMessage::ContactLink { link, .. } = connection
        .request(|request| ClientMessage::GetContactLink {
            request,
            link: link_id(code),
        })
        .await?
    else {
        return Err(ClientError::Unexpected);
    };
    let card = ContactCard::open(code, CardRole::Owner, &link.card).map_err(|_| tampered())?;
    if card.account != link.owner.into_bytes() {
        return Err(tampered());
    }
    Ok((link, card))
}

pub async fn accept_link(
    connection: &Connection,
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
    connection
        .request(|request| ClientMessage::AcceptContactLink {
            request,
            link: link.id,
            card: me.seal(code, CardRole::Accepter),
            contact: card.seal_own(identity),
            key,
        })
        .await?;
    Ok(card)
}

pub async fn finish_links(
    connection: &Connection,
    identity: &RecoverySecret,
    organization: Uuid,
) -> Result<Vec<Finished>, ClientError> {
    let ServerMessage::ContactLinks { links, .. } = connection
        .request(|request| ClientMessage::ListContactLinks { request })
        .await?
    else {
        return Err(ClientError::Unexpected);
    };
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
            let put = connection
                .request(|request| ClientMessage::PutContact {
                    request,
                    contact: accepter,
                    sealed: card.seal_own(identity),
                })
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
        connection
            .request(|request| ClientMessage::DeleteContactLink {
                request,
                link: link.id,
            })
            .await?;
        if let Some(card) = opened.filter(|_| kept) {
            finished.push(Finished {
                card,
                grant: link.grant,
            });
        }
    }
    Ok(finished)
}

pub async fn remove_contact(connection: &Connection, account: Uuid) -> Result<(), ClientError> {
    connection
        .request(|request| ClientMessage::RemoveContact { request, account })
        .await?;
    Ok(())
}

pub async fn invite(
    connection: &Connection,
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
    connection
        .request(|request| ClientMessage::Invite {
            request,
            invitation: link_id(&code),
            organization,
            email,
            role,
            sealed: me.seal(&code, CardRole::Owner),
            code: identity.seal_own(&code.to_bytes()),
            escrow,
        })
        .await?;
    Ok(code)
}

pub async fn invitation(
    connection: &Connection,
    code: &InviteCode,
) -> Result<(OrganizationInvitation, ContactCard), ClientError> {
    let ServerMessage::Invitation { invitation, .. } = connection
        .request(|request| ClientMessage::GetInvitation {
            request,
            invitation: link_id(code),
        })
        .await?
    else {
        return Err(ClientError::Unexpected);
    };
    let card =
        ContactCard::open(code, CardRole::Owner, &invitation.sealed).map_err(|_| tampered())?;
    if card.account != invitation.invited_by.into_bytes() {
        return Err(tampered());
    }
    Ok((invitation, card))
}

pub async fn accept_invitation(
    connection: &Connection,
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
    connection
        .request(|request| ClientMessage::AcceptInvitation {
            request,
            invitation: invitation.id,
            card: me.seal(code, CardRole::Accepter),
            contact: card.seal_own(identity),
            escrow,
        })
        .await?;
    Ok(invitation)
}
