//! Share links (guides/blocks/keys.md, "Share links"): "anyone with the link" on a block. The
//! server keeps each link's record of whom it admits and what it hands its opener, sealed with a
//! code it never sees, and checks every grant made through one in the block's log, where the
//! link's key, which only a holder of the code has, signed it.

use std::collections::BTreeSet;

use be_block::LinkSharing;
use be_block_graph::{Access, BlockGraph, SEARCH_INDEX_TYPE};
use be_block_keys::{Admits, Level, SigningPublic};
use be_block_protocol::{
    ErrorCode, KeyKind, LinkAdmits, OrganizationKind, OrganizationRole, ShareLink, ShareLinkRow,
    WrappedBy,
};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{
    ServerError,
    blocks::{revoke_or_grant, visibility},
    contacts::role_in,
    keys::{encode_wrapped_by, key_row},
    store::{Identity, ServerStore, decode_access, encode_access, encode_role, portal::now},
};

/// The most share links one block has open.
pub const MAX_SHARE_LINKS_PER_BLOCK: i64 = 16;

/// A share link as the server keeps it.
struct Stored {
    organization: Uuid,
    block: Uuid,
    maker: Uuid,
    access: Access,
    admits: Admits,
    key: SigningPublic,
    sealed: Vec<u8>,
}

/// What opening a link changed: who could know of its block before, for those who are told.
pub struct Opened {
    pub organization: Uuid,
    pub block: Uuid,
    pub before: BTreeSet<Uuid>,
}

/// What revoking a link changed: who could know of its block before, whose grants fell, and the
/// guests that left the organization with them.
pub struct Revoked {
    pub before: BTreeSet<Uuid>,
    pub fallen: Vec<Uuid>,
    pub left: Vec<Uuid>,
}

fn not_found() -> ServerError {
    ServerError::Refused(
        ErrorCode::LinkNotFound,
        "that link does not exist, or was revoked".into(),
    )
}

fn refused(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::PermissionDenied, message.into())
}

fn encode_admits(admits: Admits) -> &'static str {
    match admits {
        Admits::Members => "members",
        Admits::Anyone => "anyone",
    }
}

fn decode_admits(value: &str) -> Result<Admits, ServerError> {
    match value {
        "members" => Ok(Admits::Members),
        "anyone" => Ok(Admits::Anyone),
        _ => Err(ServerError::Corrupt),
    }
}

fn wire_admits(admits: Admits) -> LinkAdmits {
    match admits {
        Admits::Members => LinkAdmits::Members,
        Admits::Anyone => LinkAdmits::Anyone,
    }
}

fn access_of(level: Level) -> Access {
    match level {
        Level::None => Access::None,
        Level::KnowExists => Access::KnowExists,
        Level::Join => Access::Join,
        Level::View => Access::View,
        Level::Edit => Access::Edit,
        Level::Manage => Access::Manage,
    }
}

fn stored(connection: &Connection, link: Uuid) -> Result<Option<Stored>, ServerError> {
    let row = connection
        .prepare_cached(
            "SELECT organization_id, block_id, maker_id, access, admits, link_key, sealed
             FROM share_links WHERE id = ?1",
        )?
        .query_row([link.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, Vec<u8>>(6)?,
            ))
        })
        .optional()?;
    let Some((organization, block, maker, access, admits, key, sealed)) = row else {
        return Ok(None);
    };
    Ok(Some(Stored {
        organization: crate::store::parse_uuid(&organization)?,
        block: crate::store::parse_uuid(&block)?,
        maker: crate::store::parse_uuid(&maker)?,
        access: decode_access(&access)?,
        admits: decode_admits(&admits)?,
        key: SigningPublic(key.try_into().map_err(|_| ServerError::Corrupt)?),
        sealed,
    }))
}

impl ServerStore {
    /// Whether `organization`'s policy lets a link admit `admits`: in the personal organization,
    /// where every account is a member, only its members.
    fn link_policy_allows(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        organization: Uuid,
        admits: Admits,
    ) -> Result<bool, ServerError> {
        if crate::keys::organization_kind(connection, organization)? == OrganizationKind::Personal {
            return Ok(admits == Admits::Members);
        }
        Ok(
            match self
                .policies(graph, connection, organization)?
                .link_sharing()
            {
                LinkSharing::Off => false,
                LinkSharing::Members => admits == Admits::Members,
                LinkSharing::Anyone => true,
            },
        )
    }

    /// Makes share link `link` on `block`, by a manager of it who is a member, with the record
    /// opening it in the block's log, while the organization's policy allows whom it admits.
    pub async fn create_share_link(
        &self,
        identity: Identity,
        link: Uuid,
        block: Uuid,
        sealed: Vec<u8>,
        signed: Vec<u8>,
    ) -> Result<(), ServerError> {
        crate::limits::at_most(
            "what a share link hands its opener",
            sealed.len(),
            crate::limits::MAX_SEALED_BYTES,
        )?;
        if identity.role == OrganizationRole::Guest {
            return Err(refused("a guest makes no share links"));
        }
        self.with_graph(identity.organization, move |store, graph, database| {
            crate::blocks::require_manage(graph, &visibility(identity), block)?;
            if graph.owner_only_root(block).is_some()
                || graph
                    .get(block)
                    .is_some_and(|node| node.content_type == SEARCH_INDEX_TYPE)
            {
                return Err(refused(
                    "this block is shared with no one, so it has no share link",
                ));
            }
            let entry = be_block_keys::Entry::decode(&signed)
                .map_err(|_| refused("the share link record does not read"))?;
            let be_block_keys::Record::ShareLink(made) = &entry.record else {
                return Err(refused("a share link is made by a share link record"));
            };
            if !store.link_policy_allows(graph, database, identity.organization, made.admits)? {
                return Err(crate::organization::refused(
                    "this organization's policy does not let a share link admit them",
                ));
            }
            let open: i64 = database
                .prepare_cached(
                    "SELECT COUNT(*) FROM share_links WHERE organization_id = ?1 AND block_id = ?2",
                )?
                .query_row(
                    params![identity.organization.to_string(), block.to_string()],
                    |row| row.get(0),
                )?;
            if open >= MAX_SHARE_LINKS_PER_BLOCK {
                return Err(crate::limits::too_many(
                    "share links one block has open",
                    MAX_SHARE_LINKS_PER_BLOCK,
                ));
            }
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let made = crate::access::append_share_link(
                &transaction,
                identity.organization,
                block,
                identity.account,
                None,
                true,
                &signed,
            )?;
            let inserted = transaction.execute(
                "INSERT OR IGNORE INTO share_links
                    (id, organization_id, block_id, maker_id, access, admits, link_key, sealed,
                     created)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    link.to_string(),
                    identity.organization.to_string(),
                    block.to_string(),
                    identity.account.to_string(),
                    encode_access(access_of(made.level)),
                    encode_admits(made.admits),
                    made.key.0.to_vec(),
                    sealed,
                    now()
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

    /// Refuses `caller` a link it may not open: one it does not admit, or one the policy no
    /// longer allows. Answers the caller's role in the link's organization, if any.
    fn may_open(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        caller: Uuid,
        found: &Stored,
    ) -> Result<Option<OrganizationRole>, ServerError> {
        if !graph.contains(found.block) {
            return Err(not_found());
        }
        let role = role_in(connection, caller, found.organization)?;
        let member = role.is_some_and(|role| role != OrganizationRole::Guest);
        if found.admits == Admits::Members && !member {
            return Err(refused(
                "this link opens only for members of its organization",
            ));
        }
        if !self.link_policy_allows(graph, connection, found.organization, found.admits)? {
            return Err(crate::organization::refused(
                "this organization's policy no longer lets this link admit you",
            ));
        }
        Ok(role)
    }

    /// Share link `link`, as `caller` reads it to open it.
    pub async fn share_link(&self, caller: Uuid, link: Uuid) -> Result<ShareLink, ServerError> {
        let organization = self
            .with_database(move |_, database| {
                Ok(stored(database, link)?.map(|found| found.organization))
            })
            .await?
            .ok_or_else(not_found)?;
        self.with_graph(organization, move |store, graph, database| {
            let found = stored(database, link)?.ok_or_else(not_found)?;
            let role = store.may_open(graph, database, caller, &found)?;
            Ok(ShareLink {
                id: link,
                organization: found.organization,
                organization_name: crate::contacts::organization_name(database, organization)?
                    .unwrap_or_default(),
                block: found.block,
                access: found.access,
                admits: wire_admits(found.admits),
                sealed: found.sealed,
                head: crate::access::block_head(database, organization, found.block)?
                    .ok_or(ServerError::Corrupt)?,
                guest: role.is_none(),
            })
        })
        .await
    }

    /// Opens share link `link` for `caller`: appends the record granting it the link's level,
    /// signed with the link's key, applies the grant, and keeps the scope key `key` it sealed to
    /// itself. A caller from outside the organization joins it as a guest.
    pub async fn open_share_link(
        &self,
        caller: Uuid,
        link: Uuid,
        signed: Vec<u8>,
        key: Option<Uuid>,
        sealed: Vec<u8>,
    ) -> Result<Opened, ServerError> {
        crate::limits::at_most(
            "a sealed key",
            sealed.len(),
            crate::limits::MAX_SEALED_BYTES,
        )?;
        let organization = self
            .with_database(move |_, database| {
                Ok(stored(database, link)?.map(|found| found.organization))
            })
            .await?
            .ok_or_else(not_found)?;
        self.with_graph(organization, move |store, graph, database| {
            let found = stored(database, link)?.ok_or_else(not_found)?;
            let role = store.may_open(graph, database, caller, &found)?;
            // A Join link carries no key (plans/hosted_blocks.md): its opener reads nothing
            // stored. Any other carries the block's own scope key, which the opener seals to
            // itself.
            let carried = match (found.access, key) {
                (Access::Join, None) if sealed.is_empty() => None,
                (Access::Join, _) => {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "a Join link carries no key".into(),
                    ));
                }
                (_, Some(key))
                    if key_row(database, organization, key)?.is_some_and(|row| {
                        row.block == found.block && row.kind == KeyKind::Scope
                    }) =>
                {
                    Some(key)
                }
                _ => {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "a share link carries its block's own scope key".into(),
                    ));
                }
            };
            let before = graph.audience(found.block);
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            if role.is_none() {
                transaction.execute(
                    "INSERT INTO memberships (organization_id, account_id, role)
                     VALUES (?1, ?2, ?3)",
                    params![
                        organization.to_string(),
                        caller.to_string(),
                        encode_role(OrganizationRole::Guest)
                    ],
                )?;
            }
            crate::access::append_opened(
                &transaction,
                organization,
                found.block,
                caller,
                found.key,
                crate::access::level(found.access),
                &signed,
            )?;
            let held = graph
                .get(found.block)
                .and_then(|node| node.grants.get(&caller).copied())
                .unwrap_or(Access::None);
            if found.access > held {
                revoke_or_grant(store, 
                    graph,
                    &transaction,
                    organization,
                    found.block,
                    caller,
                    found.access,
                )?;
            }
            if let Some(key) = carried
                && !sealed.is_empty()
            {
                transaction.execute(
                    "INSERT OR REPLACE INTO wrapped_keys
                        (organization_id, key_id, wrapped_by, sender, sealed)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        organization.to_string(),
                        key.to_string(),
                        encode_wrapped_by(WrappedBy::Account(caller)),
                        caller.to_string(),
                        sealed
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(Opened {
                organization,
                block: found.block,
                before,
            })
        })
        .await
    }

    /// The share links open on `block`, newest first, for a manager of it.
    pub async fn share_links(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<Vec<ShareLinkRow>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            crate::blocks::require_manage(graph, &visibility(identity), block)?;
            let mut statement = database.prepare_cached(
                "SELECT id, maker_id, access, admits, link_key, created,
                        COALESCE((SELECT display_name FROM accounts WHERE id = maker_id), '')
                 FROM share_links WHERE organization_id = ?1 AND block_id = ?2
                 ORDER BY created DESC, id",
            )?;
            let rows = statement.query_map(
                params![identity.organization.to_string(), block.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )?;
            let mut links = Vec::new();
            for row in rows {
                let (id, maker, access, admits, key, created, maker_name) = row?;
                links.push(ShareLinkRow {
                    id: crate::store::parse_uuid(&id)?,
                    access: decode_access(&access)?,
                    admits: wire_admits(decode_admits(&admits)?),
                    maker: crate::store::parse_uuid(&maker)?,
                    maker_name,
                    created: created * 1000,
                    key: key.try_into().map_err(|_| ServerError::Corrupt)?,
                });
            }
            Ok(links)
        })
        .await
    }

    /// Revokes share link `link`, by a manager of its block, with the record closing it: every
    /// grant made through it falls, as the log says, and each guest left with no grant leaves.
    pub async fn revoke_share_link(
        &self,
        identity: Identity,
        link: Uuid,
        signed: Vec<u8>,
    ) -> Result<(Uuid, Revoked), ServerError> {
        self.with_graph(identity.organization, move |store, graph, database| {
            let found = stored(database, link)?
                .filter(|found| found.organization == identity.organization)
                .ok_or_else(not_found)?;
            crate::blocks::require_manage(graph, &visibility(identity), found.block)?;
            let _ = found.maker;
            let fallen = crate::access::linked_through(
                database,
                identity.organization,
                found.block,
                found.key,
            )?;
            let before = graph.audience(found.block);
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            crate::access::append_share_link(
                &transaction,
                identity.organization,
                found.block,
                identity.account,
                Some(found.key),
                false,
                &signed,
            )?;
            let guests: Vec<Uuid> = fallen
                .iter()
                .copied()
                .filter(|account| {
                    role_in(database, *account, identity.organization)
                        .is_ok_and(|role| role == Some(OrganizationRole::Guest))
                })
                .collect();
            for account in &fallen {
                revoke_or_grant(store, 
                    graph,
                    &transaction,
                    identity.organization,
                    found.block,
                    *account,
                    Access::None,
                )?;
            }
            // Guests with no grant left went with them (`drop_idle_guest`, from `write_grant`).
            let mut left = Vec::new();
            for account in guests {
                if role_in(&transaction, account, identity.organization)?.is_none() {
                    left.push(account);
                }
            }
            transaction.execute("DELETE FROM share_links WHERE id = ?1", [link.to_string()])?;
            transaction.commit()?;
            Ok((
                found.block,
                Revoked {
                    before,
                    fallen,
                    left,
                },
            ))
        })
        .await
    }
}

/// Removes `account` from `organization` when it is a guest that holds no grant there any more:
/// a guest is there only through what links granted it. Its shell's own blocks, which only it
/// reads (decisions/a_guest_makes_only_its_own_shell.md), go with it. Answers whether it went.
/// Every path that removes a grant calls it (`blocks::write_grant`): a guest's membership row
/// is deleted nowhere else, so its shell never outlives it.
pub(crate) fn drop_idle_guest(
    store: &crate::store::ServerStore,
    connection: &Connection,
    graph: &mut BlockGraph,
    organization: Uuid,
    account: Uuid,
) -> Result<bool, ServerError> {
    if role_in(connection, account, organization)? != Some(OrganizationRole::Guest) {
        return Ok(false);
    }
    // A grant on a block it made itself (its shell's own) keeps it no more than none.
    let held: i64 = connection
        .prepare_cached(
            "SELECT COUNT(*) FROM block_access AS access
             JOIN blocks ON blocks.organization_id = access.organization_id
                 AND blocks.id = access.block_id
             WHERE access.organization_id = ?1 AND access.account_id = ?2
                 AND blocks.author != ?2",
        )?
        .query_row(
            params![organization.to_string(), account.to_string()],
            |row| row.get(0),
        )?;
    if held > 0 {
        return Ok(false);
    }
    connection.execute(
        "DELETE FROM memberships WHERE organization_id = ?1 AND account_id = ?2",
        params![organization.to_string(), account.to_string()],
    )?;
    // Its shell: its own settings at the top, owner-only, with everything below them.
    let shell: Vec<Uuid> = graph
        .children(be_block_graph::BlockParent::Root)
        .into_iter()
        .filter(|block| {
            graph
                .get(*block)
                .is_some_and(|node| node.author == account && node.owner_only)
        })
        .flat_map(|root| graph.subtree(root))
        .collect();
    store.forget_blocks(connection, graph, organization, &shell)?;
    Ok(true)
}
