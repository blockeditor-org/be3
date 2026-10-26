use std::collections::BTreeSet;

use be_graph::Access;
use be_protocol::{
    ContactLink, ContactRow, ErrorCode, KeyKind, OwnContactLink, OwnKind, OwnSealed, PendingGrant,
    WrappedBy,
};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{
    ServerError,
    blocks::{visibility, write_grant},
    keys::{encode_wrapped_by, key_row},
    store::{
        Identity, ServerStore, decode_access, decode_role, encode_access, is_member_of, parse_uuid,
    },
};

pub struct Accepted {
    pub owner: Uuid,
    pub granted: Option<Granted>,
}

pub struct Granted {
    pub organization: Uuid,
    pub block: Uuid,
    pub before: BTreeSet<Uuid>,
}

fn link_not_found() -> ServerError {
    ServerError::Refused(
        ErrorCode::LinkNotFound,
        "that contact link does not exist, or was already used".into(),
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

pub(crate) fn is_verified_contact(
    connection: &Connection,
    account: Uuid,
    contact: Uuid,
) -> Result<bool, ServerError> {
    Ok(connection
        .prepare_cached(
            "SELECT 1 FROM contacts
             WHERE account_id = ?1 AND contact_id = ?2 AND length(sealed) > 0",
        )?
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

pub(crate) fn befriend(
    connection: &Connection,
    owner: Uuid,
    owner_sealed: &[u8],
    other: Uuid,
    other_sealed: &[u8],
) -> Result<(), ServerError> {
    for (account, contact, sealed) in [(owner, other, owner_sealed), (other, owner, other_sealed)] {
        connection.execute(
            "INSERT INTO contacts (account_id, contact_id, sealed) VALUES (?1, ?2, ?3)
             ON CONFLICT (account_id, contact_id)
             DO UPDATE SET sealed = CASE WHEN length(excluded.sealed) > 0
                 THEN excluded.sealed ELSE contacts.sealed END",
            params![account.to_string(), contact.to_string(), sealed],
        )?;
    }
    Ok(())
}

pub(crate) fn own_sealed(
    connection: &Connection,
    account: Uuid,
) -> Result<Vec<OwnSealed>, ServerError> {
    let mut own = Vec::new();
    for (kind, query) in [
        (
            OwnKind::Contact,
            "SELECT contact_id, sealed FROM contacts WHERE account_id = ?1 AND length(sealed) > 0",
        ),
        (
            OwnKind::Link,
            "SELECT id, code FROM contact_links WHERE owner_id = ?1 AND length(code) > 0",
        ),
        (
            OwnKind::Link,
            "SELECT id, code FROM invitations WHERE invited_by = ?1 AND length(code) > 0",
        ),
        (
            OwnKind::Escrow,
            "SELECT organization_id, sealed FROM pinned_escrows WHERE account_id = ?1",
        ),
    ] {
        let mut statement = connection.prepare(query)?;
        let rows = statement.query_map([account.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        for row in rows {
            let (id, sealed) = row?;
            own.push(OwnSealed {
                kind,
                id: parse_uuid(&id)?,
                sealed,
            });
        }
    }
    Ok(own)
}

pub(crate) fn reseal_own(
    connection: &Connection,
    account: Uuid,
    resealed: &OwnSealed,
) -> Result<(), ServerError> {
    let (account, id) = (account.to_string(), resealed.id.to_string());
    match resealed.kind {
        OwnKind::Contact => {
            connection.execute(
                "UPDATE contacts SET sealed = ?3 WHERE account_id = ?1 AND contact_id = ?2",
                params![account, id, resealed.sealed],
            )?;
        }
        OwnKind::Link => {
            connection.execute(
                "UPDATE contact_links SET code = ?3 WHERE owner_id = ?1 AND id = ?2",
                params![account, id, resealed.sealed],
            )?;
            connection.execute(
                "UPDATE invitations SET code = ?3 WHERE invited_by = ?1 AND id = ?2",
                params![account, id, resealed.sealed],
            )?;
        }
        OwnKind::Escrow => {
            connection.execute(
                "UPDATE pinned_escrows SET sealed = ?3 WHERE account_id = ?1 AND organization_id = ?2",
                params![account, id, resealed.sealed],
            )?;
        }
    }
    Ok(())
}

fn key_changes(
    connection: &Connection,
    account: &str,
    caller: Uuid,
) -> Result<Vec<Vec<u8>>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT sealed FROM key_changes WHERE account_id = ?1 AND contact_id = ?2
         ORDER BY position",
    )?;
    let rows = statement.query_map(params![account, caller.to_string()], |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub(crate) fn role_in(
    connection: &Connection,
    account: Uuid,
    organization: Uuid,
) -> Result<Option<be_protocol::OrganizationRole>, ServerError> {
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
    code: Vec<u8>,
    grant: Option<PendingGrant>,
    accepter: Option<Uuid>,
    accepter_card: Vec<u8>,
}

fn stored_link(connection: &Connection, link: Uuid) -> Result<Option<StoredLink>, ServerError> {
    let row = connection
        .query_row(
            "SELECT owner_id, card, code, grant_organization, grant_block, grant_access,
                    grant_key, grant_sealed, accepter_id, accepter_card
             FROM contact_links WHERE id = ?1",
            [link.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Vec<u8>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Vec<u8>>(9)?,
                ))
            },
        )
        .optional()?;
    let Some((
        owner,
        card,
        code,
        organization,
        block,
        access,
        key,
        sealed,
        accepter,
        accepter_card,
    )) = row
    else {
        return Ok(None);
    };
    let grant = match organization.is_empty() {
        true => None,
        false => Some(PendingGrant {
            organization: parse_uuid(&organization)?,
            block: parse_uuid(&block)?,
            access: decode_access(&access)?,
            key: parse_uuid(&key)?,
            sealed,
        }),
    };
    Ok(Some(StoredLink {
        owner: parse_uuid(&owner)?,
        card,
        code,
        grant,
        accepter: match accepter.is_empty() {
            true => None,
            false => Some(parse_uuid(&accepter)?),
        },
        accepter_card,
    }))
}

fn mark_accepted(
    connection: &Connection,
    link: Uuid,
    accepter: Uuid,
    card: &[u8],
) -> Result<(), ServerError> {
    let marked = connection.execute(
        "UPDATE contact_links SET accepter_id = ?2, accepter_card = ?3
         WHERE id = ?1 AND accepter_id = ''",
        params![link.to_string(), accepter.to_string(), card],
    )?;
    match marked {
        1 => Ok(()),
        _ => Err(link_not_found()),
    }
}

impl ServerStore {
    pub async fn create_contact_link(
        &self,
        owner: Uuid,
        link: Uuid,
        card: Vec<u8>,
        code: Vec<u8>,
        grant: Option<PendingGrant>,
    ) -> Result<(), ServerError> {
        if let Some(grant) = &grant {
            let role = self.membership(owner, grant.organization).await?;
            let identity = Identity {
                account: owner,
                organization: grant.organization,
                role,
            };
            let grant = grant.clone();
            self.with_graph(grant.organization, move |graph, database| {
                if !visibility(identity).can_manage(graph, grant.block) {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("block {} is not managed by this account", grant.block),
                    ));
                }
                if !self
                    .policies(graph, database, grant.organization)?
                    .shares_outside()
                {
                    return Err(crate::organization::refused(
                        "this organization shares nothing outside it, and a link reaches whoever holds it",
                    ));
                }
                let key = key_row(database, grant.organization, grant.key)?;
                if !key.is_some_and(|key| key.block == grant.block && key.kind == KeyKind::Scope)
                    || grant.access == Access::None
                {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "a pending grant needs an access level and the block's own scope key"
                            .into(),
                    ));
                }
                Ok(())
            })
            .await?;
        }
        let (organization, block, access, key, sealed) = match &grant {
            Some(grant) => (
                grant.organization.to_string(),
                grant.block.to_string(),
                encode_access(grant.access).unwrap_or_default(),
                grant.key.to_string(),
                grant.sealed.clone(),
            ),
            None => Default::default(),
        };
        let database = self.database().lock().await;
        let inserted = database.execute(
            "INSERT OR IGNORE INTO contact_links
                (id, owner_id, card, code, grant_organization, grant_block, grant_access,
                 grant_key, grant_sealed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                link.to_string(),
                owner.to_string(),
                card,
                code,
                organization,
                block,
                access,
                key,
                sealed
            ],
        )?;
        if inserted == 0 {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "a contact link with that id already exists".into(),
            ));
        }
        Ok(())
    }

    pub async fn contact_link(&self, caller: Uuid, link: Uuid) -> Result<ContactLink, ServerError> {
        let database = self.database().lock().await;
        let stored = stored_link(&database, link)?
            .filter(|stored| stored.accepter.is_none())
            .ok_or_else(link_not_found)?;
        if stored.owner == caller {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "this is your own contact link: send it to the person you want to connect with"
                    .into(),
            ));
        }
        Ok(ContactLink {
            id: link,
            owner: stored.owner,
            card: stored.card,
            grant: stored.grant,
        })
    }

    pub async fn accept_contact_link(
        &self,
        caller: Uuid,
        link: Uuid,
        card: Vec<u8>,
        contact: Vec<u8>,
        key: Vec<u8>,
    ) -> Result<Accepted, ServerError> {
        let found = self.contact_link(caller, link).await?;
        let owner = found.owner;
        let Some(grant) = found.grant else {
            let database = self.database().lock().await;
            let transaction = database.unchecked_transaction()?;
            mark_accepted(&transaction, link, caller, &card)?;
            befriend(&transaction, owner, &[], caller, &contact)?;
            transaction.commit()?;
            return Ok(Accepted {
                owner,
                granted: None,
            });
        };
        let granted = self
            .with_graph(grant.organization, move |graph, database| {
                let transaction = database.unchecked_transaction()?;
                mark_accepted(&transaction, link, caller, &card)?;
                befriend(&transaction, owner, &[], caller, &contact)?;
                let owner_role = role_in(&transaction, owner, grant.organization)?;
                let may_grant = is_member_of(&transaction, caller, grant.organization)?
                    && graph.contains(grant.block)
                    && owner_role.is_some_and(|role| {
                        visibility(Identity {
                            account: owner,
                            organization: grant.organization,
                            role,
                        })
                        .can_manage(graph, grant.block)
                    })
                    && self
                        .policies(graph, &transaction, grant.organization)
                        .is_ok_and(|policies| policies.shares_outside());
                let mut granted = None;
                if may_grant {
                    let before = graph.audience(grant.block);
                    write_grant(
                        graph,
                        &transaction,
                        grant.organization,
                        grant.block,
                        caller,
                        grant.access,
                    )?;
                    if !key.is_empty() {
                        transaction.execute(
                            "INSERT OR REPLACE INTO wrapped_keys
                                (organization_id, key_id, wrapped_by, sender, sealed)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![
                                grant.organization.to_string(),
                                grant.key.to_string(),
                                encode_wrapped_by(WrappedBy::Account(caller)),
                                caller.to_string(),
                                key
                            ],
                        )?;
                    }
                    granted = Some(Granted {
                        organization: grant.organization,
                        block: grant.block,
                        before,
                    });
                }
                transaction.commit()?;
                Ok(granted)
            })
            .await?;
        Ok(Accepted { owner, granted })
    }

    pub async fn contact_links(&self, caller: Uuid) -> Result<Vec<OwnContactLink>, ServerError> {
        let database = self.database().lock().await;
        let mut ids = Vec::new();
        {
            let mut statement =
                database.prepare("SELECT id FROM contact_links WHERE owner_id = ?1")?;
            let rows = statement.query_map([caller.to_string()], |row| row.get::<_, String>(0))?;
            for row in rows {
                ids.push(parse_uuid(&row?)?);
            }
        }
        let mut links = Vec::new();
        for id in ids {
            let Some(stored) = stored_link(&database, id)? else {
                continue;
            };
            links.push(OwnContactLink {
                id,
                code: stored.code,
                accepter: stored.accepter,
                card: stored.accepter_card,
                grant: stored.grant,
            });
        }
        Ok(links)
    }

    pub async fn delete_contact_link(&self, caller: Uuid, link: Uuid) -> Result<(), ServerError> {
        let database = self.database().lock().await;
        database.execute(
            "DELETE FROM contact_links WHERE id = ?1 AND owner_id = ?2",
            params![link.to_string(), caller.to_string()],
        )?;
        Ok(())
    }

    pub async fn put_contact(
        &self,
        caller: Uuid,
        contact: Uuid,
        sealed: Vec<u8>,
    ) -> Result<(), ServerError> {
        let database = self.database().lock().await;
        let updated = database.execute(
            "UPDATE contacts SET sealed = ?3 WHERE account_id = ?1 AND contact_id = ?2",
            params![caller.to_string(), contact.to_string(), sealed],
        )?;
        if updated == 0 {
            return Err(ServerError::Refused(
                ErrorCode::NotAContact,
                "that account is not one of your contacts".into(),
            ));
        }
        Ok(())
    }

    pub async fn list_contacts(&self, caller: Uuid) -> Result<Vec<ContactRow>, ServerError> {
        let database = self.database().lock().await;
        let mut statement = database.prepare(
            "SELECT accounts.id, CASE WHEN accounts.local_profile THEN '' ELSE accounts.email END, accounts.display_name, contacts.sealed
             FROM contacts JOIN accounts ON accounts.id = contacts.contact_id
             WHERE contacts.account_id = ?1
             ORDER BY accounts.display_name, accounts.email",
        )?;
        let rows = statement.query_map([caller.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })?;
        let mut contacts = Vec::new();
        for row in rows {
            let (account, email, display_name, sealed) = row?;
            let changes = key_changes(&database, &account, caller)?;
            contacts.push(ContactRow {
                account: parse_uuid(&account)?,
                email,
                display_name,
                sealed,
                changes,
            });
        }
        Ok(contacts)
    }

    pub async fn remove_contact(
        &self,
        caller: Uuid,
        other: Uuid,
    ) -> Result<Vec<Granted>, ServerError> {
        let shared = {
            let database = self.database().lock().await;
            let transaction = database.unchecked_transaction()?;
            transaction.execute(
                "DELETE FROM contacts WHERE (account_id = ?1 AND contact_id = ?2)
                    OR (account_id = ?2 AND contact_id = ?1)",
                params![caller.to_string(), other.to_string()],
            )?;
            transaction.execute(
                "DELETE FROM key_changes WHERE (account_id = ?1 AND contact_id = ?2)
                    OR (account_id = ?2 AND contact_id = ?1)",
                params![caller.to_string(), other.to_string()],
            )?;
            transaction.execute(
                "DELETE FROM contact_links WHERE (owner_id = ?1 AND accepter_id = ?2)
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
                let rows =
                    statement.query_map(params![caller.to_string(), other.to_string()], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?;
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
            shared
        };
        let mut revoked = Vec::new();
        for (organization, mine, theirs) in shared {
            let found = self
                .with_graph(organization, move |graph, database| {
                    let me = visibility(Identity {
                        account: caller,
                        organization,
                        role: mine,
                    });
                    let them = visibility(Identity {
                        account: other,
                        organization,
                        role: theirs,
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
                    let transaction = database.unchecked_transaction()?;
                    let mut revoked = Vec::new();
                    for (block, grantee) in grants {
                        let before = graph.audience(block);
                        crate::blocks::revoke_or_grant(
                            graph,
                            &transaction,
                            organization,
                            block,
                            grantee,
                            Access::None,
                        )?;
                        revoked.push(Granted {
                            organization,
                            block,
                            before,
                        });
                    }
                    transaction.commit()?;
                    Ok(revoked)
                })
                .await?;
            revoked.extend(found);
        }
        Ok(revoked)
    }
}
