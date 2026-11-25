use std::collections::BTreeSet;

use be_block_graph::Access;
use be_block_protocol::{
    ErrorCode, Invitation, InvitationPreview, InvitationRow, InvitationState, Letter, Link,
    LinkKind, OrganizationRole,
};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{
    ServerError,
    blocks::visibility,
    store::{
        Identity, ServerStore, decode_role, encode_role, is_member_of, parse_uuid, portal::now,
    },
};

pub struct Accepted {
    pub owner: Uuid,
}

pub struct Granted {
    pub organization: Uuid,
    pub block: Uuid,
    pub before: BTreeSet<Uuid>,
}

fn link_not_found() -> ServerError {
    ServerError::Refused(
        ErrorCode::LinkNotFound,
        "that link does not exist, or was already used".into(),
    )
}

pub(crate) fn is_contact(
    connection: &Connection,
    account: Uuid,
    contact: Uuid,
) -> Result<bool, ServerError> {
    Ok(connection
        .prepare_cached("SELECT 1 FROM contacts WHERE account_id = ?1 AND contact_id = ?2")?
        .query_row(
            params![account.to_string(), contact.to_string()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub(crate) fn contacts_of(
    connection: &Connection,
    account: Uuid,
) -> Result<BTreeSet<Uuid>, ServerError> {
    let mut statement =
        connection.prepare_cached("SELECT contact_id FROM contacts WHERE account_id = ?1")?;
    let rows = statement.query_map([account.to_string()], |row| row.get::<_, String>(0))?;
    let mut contacts = BTreeSet::new();
    for row in rows {
        contacts.insert(parse_uuid(&row?)?);
    }
    Ok(contacts)
}

/// Makes `owner` and `other` contacts, both ways, leaving `other`'s record of `owner`, sealed by
/// `other` for itself, in `other`'s mailbox (`letter.contact`) and, given one, `owner`'s record
/// of `other` in `owner`'s: the server keeps only that the two are contacts, and each device
/// keeps its records in the account's block.
pub(crate) fn befriend(
    connection: &Connection,
    owner: Uuid,
    owner_record: &[u8],
    other: Uuid,
    other_record: &[u8],
) -> Result<(), ServerError> {
    for (account, contact, record) in [(owner, other, owner_record), (other, owner, other_record)] {
        crate::limits::at_most(
            "a contact record",
            record.len(),
            crate::limits::MAX_SEALED_BYTES,
        )?;
        let (known, count): (bool, i64) = connection
            .prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM contacts WHERE account_id = ?1 AND contact_id = ?2),
                        (SELECT COUNT(*) FROM contacts WHERE account_id = ?1)",
            )?
            .query_row(params![account.to_string(), contact.to_string()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
        if !known && count >= crate::limits::MAX_CONTACTS_PER_ACCOUNT {
            return Err(crate::limits::too_many(
                "contacts one account has",
                crate::limits::MAX_CONTACTS_PER_ACCOUNT,
            ));
        }
        connection.execute(
            "INSERT OR IGNORE INTO contacts (account_id, contact_id) VALUES (?1, ?2)",
            params![account.to_string(), contact.to_string()],
        )?;
        if !record.is_empty() {
            crate::store::mailbox::leave(
                connection,
                account,
                Some(contact),
                &Letter::Contact(record.to_vec()),
            )?;
        }
    }
    Ok(())
}

pub(crate) fn role_in(
    connection: &Connection,
    account: Uuid,
    organization: Uuid,
) -> Result<Option<be_block_protocol::OrganizationRole>, ServerError> {
    connection
        .query_row(
            "SELECT role FROM memberships WHERE organization_id = ?1 AND account_id = ?2",
            params![organization.to_string(), account.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|role| decode_role(&role))
        .transpose()
}

struct StoredLink {
    owner: Uuid,
    card: Vec<u8>,
    kind: LinkKind,
    accepter: Option<Uuid>,
    /// For an invitation, where it stands.
    invited: Option<Invited>,
    /// For an invitation, the key its record in the organization's log names.
    invitation_key: Vec<u8>,
}

/// An invitation's times and marks, in seconds since the epoch (0 for not yet).
#[derive(Clone, Copy)]
struct Invited {
    created: i64,
    expires: i64,
    opened: i64,
    ended: i64,
    revoked: bool,
}

impl Invited {
    fn state(self, accepted: bool) -> InvitationState {
        if accepted {
            InvitationState::Accepted
        } else if self.revoked {
            InvitationState::Revoked
        } else if self.expires <= now() {
            InvitationState::Expired
        } else if self.opened > 0 {
            InvitationState::Opened
        } else {
            InvitationState::Waiting
        }
    }
}

/// Why an invitation no longer open can't be read or accepted.
fn invitation_ended(state: InvitationState) -> ServerError {
    ServerError::Refused(
        ErrorCode::LinkNotFound,
        match state {
            InvitationState::Expired => {
                "This invitation expired. Ask whoever invited you for a new link."
            }
            InvitationState::Revoked => {
                "This invitation was revoked. Ask whoever invited you for a new link."
            }
            _ => "This invitation was already used. Ask whoever invited you for a new link.",
        }
        .into(),
    )
}

const LINK_COLUMNS: &str = "owner_id, card, invite_organization, invite_escrow, accepter_id,
    invite_created, invite_expires, invite_opened, invite_ended, invite_revoked, invite_key";

type LinkRow = (
    (String, Vec<u8>),
    (String, Vec<u8>, String),
    Invited,
    Vec<u8>,
);

fn link_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LinkRow> {
    Ok((
        (row.get(0)?, row.get(1)?),
        (row.get(2)?, row.get(3)?, row.get(4)?),
        Invited {
            created: row.get(5)?,
            expires: row.get(6)?,
            opened: row.get(7)?,
            ended: row.get(8)?,
            revoked: row.get(9)?,
        },
        row.get(10)?,
    ))
}

fn parse_link(row: LinkRow) -> Result<StoredLink, ServerError> {
    let ((owner, card), (invited_to, escrow, accepter), invited, invitation_key) = row;
    let (kind, invited) = match invited_to.is_empty() {
        false => (
            LinkKind::Invitation(Invitation {
                organization: parse_uuid(&invited_to)?,
                escrow,
            }),
            Some(invited),
        ),
        true => (LinkKind::Contact, None),
    };
    Ok(StoredLink {
        owner: parse_uuid(&owner)?,
        card,
        kind,
        accepter: match accepter.is_empty() {
            true => None,
            false => Some(parse_uuid(&accepter)?),
        },
        invited,
        invitation_key,
    })
}

fn stored_link(connection: &Connection, link: Uuid) -> Result<Option<StoredLink>, ServerError> {
    connection
        .query_row(
            &format!("SELECT {LINK_COLUMNS} FROM links WHERE id = ?1"),
            [link.to_string()],
            link_row,
        )
        .optional()?
        .map(parse_link)
        .transpose()
}

/// Ends the outstanding invitations `owner` made into `organization`, as revoked: an invitation
/// lasts only while its inviter administers the organization. Called in the transaction that
/// demotes or removes them.
pub(crate) fn revoke_invitations_of(
    connection: &Connection,
    owner: Uuid,
    organization: Uuid,
) -> Result<(), ServerError> {
    connection.execute(
        "UPDATE links SET invite_revoked = 1, invite_ended = ?3
         WHERE owner_id = ?1 AND invite_organization = ?2 AND accepter_id = ''
            AND invite_revoked = 0",
        params![owner.to_string(), organization.to_string(), now()],
    )?;
    Ok(())
}

/// Where an invitation stands, counting one whose inviter no longer administers its
/// organization as revoked, whatever its row says.
fn invitation_state(
    connection: &Connection,
    stored: &StoredLink,
    invited: Invited,
    organization: Uuid,
) -> Result<InvitationState, ServerError> {
    let state = invited.state(stored.accepter.is_some());
    let open = matches!(state, InvitationState::Waiting | InvitationState::Opened);
    if open
        && role_in(connection, stored.owner, organization)? != Some(OrganizationRole::Administrator)
    {
        return Ok(InvitationState::Revoked);
    }
    Ok(state)
}

/// Marks an invitation opened, the first time its link is read while it waits.
fn mark_opened(connection: &Connection, link: Uuid) -> Result<(), ServerError> {
    connection.execute(
        "UPDATE links SET invite_opened = ?2
         WHERE id = ?1 AND invite_opened = 0 AND accepter_id = '' AND invite_revoked = 0",
        params![link.to_string(), now()],
    )?;
    Ok(())
}

/// The organization's name, if it still exists.
pub(crate) fn organization_name(
    connection: &Connection,
    organization: Uuid,
) -> Result<Option<String>, ServerError> {
    Ok(connection
        .query_row(
            "SELECT name FROM organizations WHERE id = ?1",
            [organization.to_string()],
            |row| row.get(0),
        )
        .optional()?)
}

/// Marks `link` accepted by `accepter` and leaves their card, sealed with its code, in its
/// owner's mailbox (`letter.accepted`).
fn mark_accepted(
    connection: &Connection,
    link: Uuid,
    owner: Uuid,
    accepter: Uuid,
    card: Vec<u8>,
) -> Result<(), ServerError> {
    // An invitation is taken once, only while it is neither revoked nor expired.
    let marked = connection.execute(
        "UPDATE links SET accepter_id = ?2,
             invite_ended = CASE WHEN invite_organization = '' THEN 0 ELSE ?3 END
         WHERE id = ?1 AND accepter_id = ''
            AND (invite_organization = '' OR (invite_revoked = 0 AND invite_expires > ?3))",
        params![link.to_string(), accepter.to_string(), now()],
    )?;
    if marked != 1 {
        return Err(link_not_found());
    }
    crate::store::mailbox::leave(
        connection,
        owner,
        Some(accepter),
        &Letter::Accepted { link, card },
    )
}

/// What a link's kind stores in its row's invitation columns.
fn invitation_columns(kind: &LinkKind) -> (String, Vec<u8>) {
    match kind {
        LinkKind::Contact => Default::default(),
        LinkKind::Invitation(invitation) => (
            invitation.organization.to_string(),
            invitation.escrow.clone(),
        ),
    }
}

impl ServerStore {
    /// Makes a link. The dispatcher checked that an invitation's caller administers its
    /// organization (`client::rule`).
    pub async fn create_link(
        &self,
        owner: Uuid,
        link: Uuid,
        card: Vec<u8>,
        kind: LinkKind,
        signed: Option<Vec<u8>>,
    ) -> Result<(), ServerError> {
        crate::limits::at_most(
            "a contact card",
            card.len(),
            crate::limits::MAX_SEALED_BYTES,
        )?;
        match &kind {
            LinkKind::Contact => {}
            LinkKind::Invitation(_) if signed.is_none() => {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "an invitation carries the record opening it in the organization's log".into(),
                ));
            }
            LinkKind::Invitation(invitation) => {
                crate::limits::at_most(
                    "an invitation's escrow",
                    invitation.escrow.len(),
                    crate::limits::MAX_SEALED_BYTES,
                )?;
                if self.membership(owner, invitation.organization).await?
                    != OrganizationRole::Administrator
                {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        "only an organization administrator can invite people".into(),
                    ));
                }
            }
        }
        if matches!(kind, LinkKind::Contact) && signed.is_some() {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "a contact link carries no record".into(),
            ));
        }
        let (invited_to, escrow) = invitation_columns(&kind);
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            if let LinkKind::Contact = &kind {
                let open: i64 = transaction
                    .prepare_cached(
                        "SELECT COUNT(*) FROM links WHERE owner_id = ?1 AND invite_organization = ''",
                    )?
                    .query_row([owner.to_string()], |row| row.get(0))?;
                if open >= crate::limits::MAX_CONTACT_LINKS_PER_ACCOUNT {
                    return Err(crate::limits::too_many(
                        "contact links one account has open",
                        crate::limits::MAX_CONTACT_LINKS_PER_ACCOUNT,
                    ));
                }
            }
            if let LinkKind::Invitation(invitation) = &kind {
                let organization = invitation.organization.to_string();
                let now = now();
                // Ended ones past their retention go.
                transaction.execute(
                    "DELETE FROM links WHERE invite_organization = ?1
                        AND (accepter_id = '' OR invite_finished = 1)
                        AND ((invite_ended > 0 AND invite_ended <= ?2)
                             OR (invite_ended = 0 AND invite_expires <= ?2))",
                    params![organization, now - crate::limits::INVITATION_RETENTION],
                )?;
                let outstanding: i64 = transaction
                    .prepare_cached(
                        "SELECT COUNT(*) FROM links
                         WHERE invite_organization = ?1 AND accepter_id = ''
                            AND invite_revoked = 0 AND invite_expires > ?2",
                    )?
                    .query_row(params![organization, now], |row| row.get(0))?;
                if outstanding >= crate::limits::MAX_INVITATIONS_PER_ORGANIZATION {
                    return Err(crate::limits::too_many(
                        "invitations an organization has outstanding",
                        crate::limits::MAX_INVITATIONS_PER_ORGANIZATION,
                    ));
                }
                // Past the bound on what the list keeps, the oldest ended go.
                transaction.execute(
                    "DELETE FROM links WHERE id IN (
                        SELECT id FROM links
                        WHERE invite_organization = ?1
                            AND ((accepter_id != '' AND invite_finished = 1)
                                 OR (accepter_id = ''
                                     AND (invite_revoked = 1 OR invite_expires <= ?2)))
                        ORDER BY invite_created
                        LIMIT max(0, (SELECT COUNT(*) FROM links WHERE invite_organization = ?1) - ?3 + 1))",
                    params![
                        organization,
                        now,
                        crate::limits::MAX_INVITATIONS_KEPT_PER_ORGANIZATION
                    ],
                )?;
            }
            let (created, expires) = match &kind {
                LinkKind::Invitation(_) => (now(), now() + crate::limits::INVITATION_LIFETIME),
                LinkKind::Contact => (0, 0),
            };
            // An invitation opens in the organization's log, by its key, as it is made.
            let invitation_key = match (&kind, &signed) {
                (LinkKind::Invitation(invitation), Some(signed)) => {
                    crate::access::append_invitation(
                        &transaction,
                        invitation.organization,
                        owner,
                        None,
                        true,
                        signed,
                    )?
                    .0
                    .to_vec()
                }
                _ => Vec::new(),
            };
            let inserted = transaction.execute(
                "INSERT OR IGNORE INTO links
                    (id, owner_id, card, invite_organization, invite_escrow, invite_created,
                     invite_expires, invite_key)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    link.to_string(),
                    owner.to_string(),
                    card,
                    invited_to,
                    escrow,
                    created,
                    expires,
                    invitation_key
                ],
            )?;
            if inserted == 0 {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "a link with that id already exists".into(),
                ));
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    /// A link not yet accepted, as `caller` reads it: not its own, and an invitation only while
    /// it is open and `caller` is not yet a member. Reading an invitation marks it opened.
    pub async fn link(&self, caller: Uuid, link: Uuid) -> Result<Link, ServerError> {
        self.with_database(move |_, database| {
            let stored = stored_link(database, link)?.ok_or_else(link_not_found)?;
            if let (LinkKind::Invitation(invitation), Some(invited)) =
                (&stored.kind, stored.invited)
            {
                let state = invitation_state(database, &stored, invited, invitation.organization)?;
                if !matches!(state, InvitationState::Waiting | InvitationState::Opened) {
                    return Err(invitation_ended(state));
                }
            }
            if stored.accepter.is_some() {
                return Err(link_not_found());
            }
            if stored.owner == caller {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "this is your own link: send it to the person you want to connect with".into(),
                ));
            }
            let organization_name = match &stored.kind {
                LinkKind::Contact => String::new(),
                LinkKind::Invitation(invitation) => {
                    // An invitation into an organization gone since is no link at all.
                    let name = organization_name(database, invitation.organization)?
                        .ok_or_else(link_not_found)?;
                    mark_opened(database, link)?;
                    if is_member_of(database, caller, invitation.organization)? {
                        return Err(ServerError::Refused(
                            ErrorCode::InvalidRequest,
                            "you are already a member of that organization".into(),
                        ));
                    }
                    name
                }
            };
            let head = match &stored.kind {
                LinkKind::Contact => None,
                LinkKind::Invitation(invitation) => {
                    crate::access::organization_head(database, invitation.organization)?
                }
            };
            Ok(Link {
                id: link,
                owner: stored.owner,
                card: stored.card,
                kind: stored.kind,
                organization_name,
                head,
            })
        })
        .await
    }

    /// An invitation as anyone holding its link reads it, before signing in: its organization's
    /// name, its inviter's sealed card and where it stands. Marks it opened while it waits. Any
    /// other link, or an invitation into an organization gone since, reads as no link.
    pub async fn read_invitation(&self, link: Uuid) -> Result<InvitationPreview, ServerError> {
        self.with_database(move |_, database| {
            let stored = stored_link(database, link)?.ok_or_else(link_not_found)?;
            let (LinkKind::Invitation(invitation), Some(invited)) = (&stored.kind, stored.invited)
            else {
                return Err(link_not_found());
            };
            let organization_name = organization_name(database, invitation.organization)?
                .ok_or_else(link_not_found)?;
            let mut state = invitation_state(database, &stored, invited, invitation.organization)?;
            if state == InvitationState::Waiting {
                mark_opened(database, link)?;
                state = InvitationState::Opened;
            }
            // An ended invitation says only that it ended.
            let open = state == InvitationState::Opened;
            Ok(InvitationPreview {
                organization_name: if open {
                    organization_name
                } else {
                    String::new()
                },
                card: if open { stored.card } else { Vec::new() },
                state,
            })
        })
        .await
    }

    /// The organization's invitations, newest first, for one of its administrators; ended ones
    /// past their retention go first. Its inviter keeps each one's code in its block.
    pub async fn list_invitations(
        &self,
        organization: Uuid,
    ) -> Result<Vec<InvitationRow>, ServerError> {
        self.with_database(move |_, database| {
            database.execute(
                "DELETE FROM links WHERE invite_organization = ?1
                    AND (accepter_id = '' OR invite_finished = 1)
                    AND ((invite_ended > 0 AND invite_ended <= ?2)
                         OR (invite_ended = 0 AND invite_expires <= ?2))",
                params![
                    organization.to_string(),
                    now() - crate::limits::INVITATION_RETENTION
                ],
            )?;
            let mut statement = database.prepare(&format!(
                "SELECT {LINK_COLUMNS}, links.id,
                        COALESCE((SELECT display_name FROM accounts WHERE id = owner_id), ''),
                        COALESCE((SELECT display_name FROM accounts WHERE id = accepter_id), '')
                 FROM links WHERE invite_organization = ?1
                 ORDER BY invite_created DESC, links.id"
            ))?;
            let rows = statement.query_map([organization.to_string()], |row| {
                Ok((
                    link_row(row)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                ))
            })?;
            let mut invitations = Vec::new();
            for row in rows {
                let (row, id, inviter_name, accepter_name) = row?;
                let stored = parse_link(row)?;
                let Some(invited) = stored.invited else {
                    continue;
                };
                let state = invitation_state(database, &stored, invited, organization)?;
                let seconds = |at: i64| at * 1000;
                invitations.push(InvitationRow {
                    id: parse_uuid(&id)?,
                    inviter: stored.owner,
                    inviter_name,
                    state,
                    accepter: stored.accepter,
                    accepter_name,
                    created: seconds(invited.created),
                    expires: seconds(invited.expires),
                    opened: seconds(invited.opened),
                    ended: seconds(invited.ended),
                    key: stored
                        .invitation_key
                        .as_slice()
                        .try_into()
                        .unwrap_or_default(),
                });
            }
            Ok(invitations)
        })
        .await
    }

    pub async fn accept_link(
        &self,
        caller: Uuid,
        link: Uuid,
        card: Vec<u8>,
        contact: Vec<u8>,
        key: Vec<u8>,
        member: Option<Vec<u8>>,
    ) -> Result<Accepted, ServerError> {
        for (what, bytes) in [
            ("a contact card", &card),
            ("a contact record", &contact),
            ("a sealed key", &key),
        ] {
            crate::limits::at_most(what, bytes.len(), crate::limits::MAX_SEALED_BYTES)?;
        }
        let found = self.link(caller, link).await?;
        let owner = found.owner;
        if matches!(found.kind, LinkKind::Contact) && member.is_some() {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "accepting a contact link signs no one in".into(),
            ));
        }
        if matches!(found.kind, LinkKind::Contact) && !key.is_empty() {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "a contact link carries no key".into(),
            ));
        }
        match found.kind {
            LinkKind::Contact => {
                self.with_database(move |_, database| {
                    let transaction = be_block_sqlite_schema::write_transaction(database)?;
                    mark_accepted(&transaction, link, owner, caller, card)?;
                    befriend(&transaction, owner, &[], caller, &contact)?;
                    transaction.commit()?;
                    Ok(Accepted { owner })
                })
                .await
            }
            LinkKind::Invitation(invitation) => {
                let member = member.ok_or_else(|| {
                    ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "accepting an invitation carries the member record signing the accepter in"
                            .into(),
                    )
                })?;
                self.with_database(move |_, database| {
                    let transaction = be_block_sqlite_schema::write_transaction(database)?;
                    // Checked again where it is taken: an inviter no longer administering
                    // the organization ended it, and an account that joined meanwhile
                    // through another invitation spends none.
                    if role_in(&transaction, owner, invitation.organization)?
                        != Some(OrganizationRole::Administrator)
                    {
                        return Err(invitation_ended(InvitationState::Revoked));
                    }
                    if is_member_of(&transaction, caller, invitation.organization)? {
                        return Err(ServerError::Refused(
                            ErrorCode::InvalidRequest,
                            "you are already a member of that organization".into(),
                        ));
                    }
                    let stored = stored_link(&transaction, link)?.ok_or_else(link_not_found)?;
                    mark_accepted(&transaction, link, owner, caller, card)?;
                    // Signed in by the invitation's key, which only the code's holder has:
                    // the log, not the server, says who joined.
                    crate::access::append_accepted(
                        &transaction,
                        invitation.organization,
                        caller,
                        &stored.invitation_key,
                        &member,
                    )?;
                    befriend(&transaction, owner, &[], caller, &contact)?;
                    crate::keys::pin_escrow(
                        &transaction,
                        caller,
                        owner,
                        invitation.organization,
                        &key,
                    )?;
                    // An invitation joins as an editor, never as an administrator; a
                    // guest becomes a member.
                    transaction.execute(
                        "INSERT OR REPLACE INTO memberships (organization_id, account_id, role)
                             VALUES (?1, ?2, ?3)",
                        params![
                            invitation.organization.to_string(),
                            caller.to_string(),
                            encode_role(OrganizationRole::Editor)
                        ],
                    )?;
                    transaction.commit()?;
                    Ok(Accepted { owner })
                })
                .await
            }
        }
    }

    /// Ends a link: a contact link of the caller's goes; an invitation, the caller's own or one
    /// into an organization it administers, is revoked while outstanding, or finished by its
    /// inviter once accepted, and its row stays for the organization's list. Revoked by an
    /// administrator, it is ended in the organization's log by `signed`; an inviter who no
    /// longer administers needs none, as their invitations ended there with their role.
    pub async fn delete_link(
        &self,
        caller: Uuid,
        link: Uuid,
        signed: Option<Vec<u8>>,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let Some(stored) = stored_link(&transaction, link)? else {
                return Ok(());
            };
            match (&stored.kind, stored.invited) {
                (LinkKind::Invitation(invitation), Some(invited)) => {
                    let inviter = stored.owner == caller;
                    let administrator = role_in(&transaction, caller, invitation.organization)?
                        == Some(OrganizationRole::Administrator);
                    if !inviter && !administrator {
                        return Ok(());
                    }
                    match invited.state(stored.accepter.is_some()) {
                        InvitationState::Waiting | InvitationState::Opened => {
                            match (administrator, &signed) {
                                (true, Some(signed)) => {
                                    let key = <[u8; 32]>::try_from(stored.invitation_key.as_slice())
                                        .map_err(|_| ServerError::Corrupt)?;
                                    crate::access::append_invitation(
                                        &transaction,
                                        invitation.organization,
                                        caller,
                                        Some(be_block_keys::SigningPublic(key)),
                                        false,
                                        signed,
                                    )?;
                                }
                                (true, None) => {
                                    return Err(ServerError::Refused(
                                        ErrorCode::InvalidRequest,
                                        "an administrator revokes an invitation with the record ending it"
                                            .into(),
                                    ));
                                }
                                (false, _) => {}
                            }
                            transaction.execute(
                                "UPDATE links SET invite_revoked = 1, invite_ended = ?2
                                 WHERE id = ?1 AND accepter_id = ''",
                                params![link.to_string(), now()],
                            )?;
                        }
                        InvitationState::Accepted if inviter => {
                            transaction.execute(
                                "UPDATE links SET invite_finished = 1 WHERE id = ?1",
                                [link.to_string()],
                            )?;
                        }
                        _ => {}
                    }
                }
                _ if signed.is_some() => {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "only revoking an invitation carries a record".into(),
                    ));
                }
                _ => {
                    transaction.execute(
                        "DELETE FROM links WHERE id = ?1 AND owner_id = ?2",
                        params![link.to_string(), caller.to_string()],
                    )?;
                }
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    /// The grants in `organization` that removing `other` as `caller`'s contact revokes, named
    /// in their blocks' logs (`query.revocations`).
    pub async fn contact_revocations(
        &self,
        caller: Uuid,
        other: Uuid,
        organization: Uuid,
    ) -> Result<Vec<(Uuid, Uuid)>, ServerError> {
        self.with_graph(organization, move |_, graph, database| {
            let (Some(mine), Some(theirs)) = (
                role_in(database, caller, organization)?,
                role_in(database, other, organization)?,
            ) else {
                return Ok(Vec::new());
            };
            let pairs = contact_grants(graph, organization, caller, mine, other, theirs);
            crate::access::named_pairs(database, organization, &pairs)
        })
        .await
    }

    /// Removes `other` as `caller`'s contact, revoking each one's grants on blocks the other
    /// manages, in every organization they share. In `current`, the organization the caller has
    /// open, `revocations` are the signed revocations of those its logs name, each needed and
    /// checked before anything changes; in any other the server's revocations wait for a device
    /// to sign them (`KeyWork::unsigned_revocations`).
    pub async fn remove_contact(
        &self,
        caller: Uuid,
        other: Uuid,
        current: Option<Uuid>,
        revocations: Vec<Vec<u8>>,
    ) -> Result<Vec<Granted>, ServerError> {
        let signed = match current {
            Some(organization) => {
                let named = self
                    .contact_revocations(caller, other, organization)
                    .await?;
                crate::access::revocations_for(&named, &revocations)?
            }
            None => crate::access::revocations_for(&[], &revocations)?,
        };
        let shared = {
            self.with_database(move |_, database| {
                let transaction = be_block_sqlite_schema::write_transaction(database)?;
                transaction.execute(
                    "DELETE FROM contacts WHERE (account_id = ?1 AND contact_id = ?2)
                        OR (account_id = ?2 AND contact_id = ?1)",
                    params![caller.to_string(), other.to_string()],
                )?;
                crate::store::mailbox::forget_between(&transaction, caller, other)?;
                // Each one's devices drop their record of the other from the account's block.
                for (to, from) in [(caller, other), (other, caller)] {
                    crate::store::mailbox::leave(&transaction, to, Some(from), &Letter::Removed)?;
                }
                transaction.execute(
                    "DELETE FROM links WHERE (owner_id = ?1 AND accepter_id = ?2)
                        OR (owner_id = ?2 AND accepter_id = ?1)",
                    params![caller.to_string(), other.to_string()],
                )?;
                let mut shared = Vec::new();
                {
                    let mut statement = transaction.prepare(
                        "SELECT mine.organization_id, mine.role, theirs.role
                         FROM memberships mine JOIN memberships theirs
                            ON theirs.organization_id = mine.organization_id
                         WHERE mine.account_id = ?1 AND theirs.account_id = ?2",
                    )?;
                    let rows = statement.query_map(
                        params![caller.to_string(), other.to_string()],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                            ))
                        },
                    )?;
                    for row in rows {
                        let (organization, mine, theirs) = row?;
                        shared.push((
                            parse_uuid(&organization)?,
                            decode_role(&mine)?,
                            decode_role(&theirs)?,
                        ));
                    }
                }
                transaction.commit()?;
                Ok(shared)
            })
            .await?
        };
        let mut revoked = Vec::new();
        for (organization, mine, theirs) in shared {
            let signed = match current == Some(organization) {
                true => signed.clone(),
                false => std::collections::HashMap::new(),
            };
            let found = self
                .with_graph(organization, move |store, graph, database| {
                    let grants = contact_grants(graph, organization, caller, mine, other, theirs);
                    let transaction = be_block_sqlite_schema::write_transaction(database)?;
                    let mut revoked = Vec::new();
                    for (block, grantee) in grants {
                        let before = graph.audience(block);
                        crate::blocks::revoke_or_grant(
                            store,
                            graph,
                            &transaction,
                            organization,
                            block,
                            grantee,
                            Access::None,
                        )?;
                        if let Some(record) = signed.get(&(block, grantee)) {
                            crate::access::append_grant(
                                &transaction,
                                organization,
                                block,
                                caller,
                                grantee,
                                Access::None,
                                record,
                            )?;
                        }
                        revoked.push(Granted {
                            organization,
                            block,
                            before,
                        });
                    }
                    let forgotten = crate::store::forget_orphaned_block_agents(
                        store,
                        graph,
                        &transaction,
                        organization,
                    )?;
                    transaction.commit()?;
                    if !forgotten.is_empty() {
                        store.sessions_changed();
                    }
                    Ok(revoked)
                })
                .await?;
            revoked.extend(found);
        }
        Ok(revoked)
    }
}

/// Each one's grants on blocks the other manages, in `organization`: what removing a contact
/// revokes there.
fn contact_grants(
    graph: &be_block_graph::BlockGraph,
    organization: Uuid,
    caller: Uuid,
    mine: be_block_protocol::OrganizationRole,
    other: Uuid,
    theirs: be_block_protocol::OrganizationRole,
) -> Vec<(Uuid, Uuid)> {
    let me = visibility(Identity {
        account: caller,
        organization,
        role: mine,
        bound: None,
    });
    let them = visibility(Identity {
        account: other,
        organization,
        role: theirs,
        bound: None,
    });
    let mut grants = Vec::new();
    for (grantee, granter) in [(other, &me), (caller, &them)] {
        for block in graph.entry_points(grantee) {
            let held = graph
                .get(block)
                .is_some_and(|node| node.grants.contains_key(&grantee));
            if held && granter.can_manage(graph, block) {
                grants.push((block, grantee));
            }
        }
    }
    grants
}
