use std::collections::{BTreeMap, BTreeSet};

use be_graph::BlockGraph;
use be_protocol::{
    ErrorCode, EscrowKey, KeyChange, KeyKind, KeyRow, KeySet, KeyWork, OrganizationKind, OwnKind,
    OwnSealed, SealedKey, Unsealed, WrappedBy, WrappedKey,
};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{
    ServerError,
    blocks::{Visibility, visibility},
    store::{Identity, ServerStore, decode_role, is_member_of, parse_uuid, recovery_key_of},
};

pub fn encode_kind(kind: KeyKind) -> &'static str {
    match kind {
        KeyKind::Data => "data",
        KeyKind::Scope => "scope",
        KeyKind::Escrow => "escrow",
    }
}

pub fn decode_kind(value: &str) -> Result<KeyKind, ServerError> {
    match value {
        "data" => Ok(KeyKind::Data),
        "scope" => Ok(KeyKind::Scope),
        "escrow" => Ok(KeyKind::Escrow),
        _ => Err(ServerError::Corrupt),
    }
}

pub fn encode_wrapped_by(wrapped_by: WrappedBy) -> String {
    match wrapped_by {
        WrappedBy::Key(key) => key.to_string(),
        WrappedBy::Account(account) => format!("account:{account}"),
    }
}

pub fn decode_wrapped_by(value: &str) -> Result<WrappedBy, ServerError> {
    match value.strip_prefix("account:") {
        Some(account) => Ok(WrappedBy::Account(parse_uuid(account)?)),
        None => Ok(WrappedBy::Key(parse_uuid(value)?)),
    }
}

fn refused(message: impl Into<String>) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

fn now_milliseconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

pub(crate) fn key_row(
    connection: &Connection,
    organization: Uuid,
    key: Uuid,
) -> Result<Option<KeyRow>, ServerError> {
    let row = connection
        .query_row(
            "SELECT block_id, kind, epoch FROM keys WHERE organization_id = ?1 AND id = ?2",
            params![organization.to_string(), key.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    row.map(|(block, kind, epoch)| {
        Ok(KeyRow {
            id: key,
            block: parse_uuid(&block)?,
            kind: decode_kind(&kind)?,
            epoch: u64::try_from(epoch).map_err(|_| ServerError::Corrupt)?,
        })
    })
    .transpose()
}

pub(crate) fn keys_of_block(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Vec<KeyRow>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT id, kind, epoch FROM keys WHERE organization_id = ?1 AND block_id = ?2",
    )?;
    let rows = statement.query_map(
        params![organization.to_string(), block.to_string()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    let mut keys = Vec::new();
    for row in rows {
        let (id, kind, epoch) = row?;
        keys.push(KeyRow {
            id: parse_uuid(&id)?,
            block,
            kind: decode_kind(&kind)?,
            epoch: u64::try_from(epoch).map_err(|_| ServerError::Corrupt)?,
        });
    }
    Ok(keys)
}

pub(crate) fn wraps_of(
    connection: &Connection,
    organization: Uuid,
    key: Uuid,
) -> Result<Vec<WrappedKey>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT wrapped_by, sender, sealed FROM wrapped_keys
         WHERE organization_id = ?1 AND key_id = ?2",
    )?;
    let rows = statement.query_map(params![organization.to_string(), key.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    let mut wraps = Vec::new();
    for row in rows {
        let (wrapped_by, sender, sealed) = row?;
        wraps.push(WrappedKey {
            key,
            wrapped_by: decode_wrapped_by(&wrapped_by)?,
            sender: parse_uuid(&sender)?,
            sealed,
        });
    }
    Ok(wraps)
}

pub(crate) fn served_keys(
    graph: &BlockGraph,
    connection: &Connection,
    identity: Identity,
    seen: &Visibility,
    blocks: impl IntoIterator<Item = Uuid>,
) -> Result<KeySet, ServerError> {
    let mut keys = BTreeMap::new();
    for block in blocks {
        if !seen.can_view(graph, block) {
            continue;
        }
        for row in keys_of_block(connection, identity.organization, block)? {
            keys.insert(row.id, row);
        }
    }
    if seen.administrator() {
        for row in keys_of_block(connection, identity.organization, identity.organization)? {
            keys.insert(row.id, row);
        }
    }
    let mut wrapped = Vec::new();
    for key in keys.keys() {
        for wrap in wraps_of(connection, identity.organization, *key)? {
            let served = match wrap.wrapped_by {
                WrappedBy::Account(account) => account == identity.account,
                WrappedBy::Key(by) => keys.contains_key(&by),
            };
            if served {
                wrapped.push(wrap);
            }
        }
    }
    Ok(KeySet {
        keys: keys.into_values().collect(),
        wrapped,
    })
}

fn passes_on(graph: &BlockGraph, seen: &Visibility, kind: Uuid) -> bool {
    graph
        .get(kind)
        .is_some_and(be_graph::BlockNode::is_open_inside)
        && seen.can_view(graph, kind)
}

fn passed_on(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    caller: Uuid,
    row: &KeyRow,
    wrap: &WrappedKey,
) -> Result<bool, ServerError> {
    let WrappedBy::Account(account) = wrap.wrapped_by else {
        return Ok(false);
    };
    let open = graph
        .get(row.block)
        .is_some_and(be_graph::BlockNode::is_open_inside);
    Ok(row.kind == KeyKind::Scope
        && open
        && account != caller
        && is_viewer(connection, graph, organization, caller, row.block)?
        && sealed_to(connection, graph, organization, row.block)?.contains(&account))
}

pub(crate) fn store_keys(
    graph: &mut BlockGraph,
    connection: &Connection,
    organization: Uuid,
    caller: Uuid,
    set: &KeySet,
    allowed: impl Fn(&BlockGraph, Uuid) -> bool,
) -> Result<(), ServerError> {
    let administrator = crate::contacts::role_in(connection, caller, organization)?
        .is_some_and(|role| role == be_protocol::OrganizationRole::Administrator);
    let allowed = |graph: &BlockGraph, row: &KeyRow| match row.kind {
        KeyKind::Escrow => administrator && row.block == organization,
        KeyKind::Data | KeyKind::Scope => graph.contains(row.block) && allowed(graph, row.block),
    };
    let mut fresh: BTreeMap<Uuid, KeyRow> = BTreeMap::new();
    for row in &set.keys {
        if row.kind == KeyKind::Escrow {
            return Err(refused("an escrow key is made with CreateEscrow"));
        }
        if !allowed(graph, row) {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                format!(
                    "a key for block {} may not be stored by this account",
                    row.block
                ),
            ));
        }
        if fresh.insert(row.id, *row).is_some()
            || key_row(connection, organization, row.id)?.is_some()
        {
            return Err(refused(format!("key {} already exists", row.id)));
        }
    }
    let known = |key: Uuid| -> Result<Option<KeyRow>, ServerError> {
        match fresh.get(&key) {
            Some(row) => Ok(Some(*row)),
            None => key_row(connection, organization, key),
        }
    };
    for wrap in &set.wrapped {
        let row =
            known(wrap.key)?.ok_or_else(|| refused(format!("key {} does not exist", wrap.key)))?;
        if !fresh.contains_key(&row.id)
            && !allowed(graph, &row)
            && !passed_on(connection, graph, organization, caller, &row, wrap)?
        {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                format!("key {} may not be wrapped by this account", row.id),
            ));
        }
        match wrap.wrapped_by {
            WrappedBy::Key(by) => {
                if by == wrap.key || known(by)?.is_none() {
                    return Err(refused(format!("key {} cannot wrap key {}", by, wrap.key)));
                }
            }
            WrappedBy::Account(account)
                if account == crate::organization::server_account(connection)? =>
            {
                if row.kind == KeyKind::Escrow {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        "the escrow key is sealed only to administrators".into(),
                    ));
                }
            }
            WrappedBy::Account(account) => {
                if !is_member_of(connection, account, organization)? {
                    return Err(refused(format!(
                        "account {account} is not a member of this organization"
                    )));
                }
                if account != caller && !crate::contacts::is_contact(connection, caller, account)? {
                    return Err(ServerError::Refused(
                        ErrorCode::NotAContact,
                        format!("a key may be sealed only to a contact, and {account} is not one"),
                    ));
                }
                if row.kind == KeyKind::Escrow
                    && crate::contacts::role_in(connection, account, organization)?
                        != Some(be_protocol::OrganizationRole::Administrator)
                {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!(
                            "the escrow key is sealed only to administrators, and {account} is not one"
                        ),
                    ));
                }
            }
        }
    }
    let server = crate::organization::server_account(connection)?;
    let created = now_milliseconds();
    for row in fresh.values() {
        connection.execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                organization.to_string(),
                row.id.to_string(),
                row.block.to_string(),
                encode_kind(row.kind),
                i64::try_from(row.epoch).map_err(|_| refused("that epoch is too large"))?,
                created
            ],
        )?;
    }
    let mut current: BTreeMap<Uuid, KeyRow> = BTreeMap::new();
    for row in set.keys.iter().filter(|row| row.kind == KeyKind::Data) {
        let newer = current
            .get(&row.block)
            .is_none_or(|held| row.epoch >= held.epoch);
        if newer {
            current.insert(row.block, *row);
        }
    }
    for (block, row) in current {
        connection.execute(
            "UPDATE blocks SET key_id = ?3 WHERE organization_id = ?1 AND id = ?2",
            params![
                organization.to_string(),
                block.to_string(),
                row.id.to_string()
            ],
        )?;
        graph.set_key(block, row.id)?;
    }
    for wrap in &set.wrapped {
        connection.execute(
            "INSERT INTO wrapped_keys
                (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (organization_id, key_id, wrapped_by) DO UPDATE
                SET sender = excluded.sender, sealed = excluded.sealed
                WHERE wrapped_keys.sender = ?6",
            params![
                organization.to_string(),
                wrap.key.to_string(),
                encode_wrapped_by(wrap.wrapped_by),
                caller.to_string(),
                wrap.sealed,
                server.to_string()
            ],
        )?;
        let WrappedBy::Key(by) = wrap.wrapped_by else {
            continue;
        };
        if known(by)?.is_some_and(|row| row.kind == KeyKind::Escrow) {
            continue;
        }
        for other in wraps_of(connection, organization, wrap.key)? {
            let WrappedBy::Key(other_by) = other.wrapped_by else {
                continue;
            };
            if other_by == by || known(other_by)?.is_some_and(|row| row.kind == KeyKind::Escrow) {
                continue;
            }
            connection.execute(
                "DELETE FROM wrapped_keys
                 WHERE organization_id = ?1 AND key_id = ?2 AND wrapped_by = ?3",
                params![
                    organization.to_string(),
                    wrap.key.to_string(),
                    encode_wrapped_by(other.wrapped_by)
                ],
            )?;
        }
    }
    Ok(())
}

pub(crate) fn escrow_chain(
    connection: &Connection,
    organization: Uuid,
) -> Result<Vec<EscrowKey>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT id, epoch, public, signing, signature FROM keys
         WHERE organization_id = ?1 AND block_id = ?1 AND kind = 'escrow'
         ORDER BY epoch",
    )?;
    let rows = statement.query_map([organization.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Vec<u8>>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, Vec<u8>>(4)?,
        ))
    })?;
    let mut chain = Vec::new();
    for row in rows {
        let (id, epoch, public, signing, signature) = row?;
        chain.push(EscrowKey {
            id: parse_uuid(&id)?,
            epoch: u64::try_from(epoch).map_err(|_| ServerError::Corrupt)?,
            public: public.try_into().map_err(|_| ServerError::Corrupt)?,
            signing: signing.try_into().map_err(|_| ServerError::Corrupt)?,
            signature,
        });
    }
    Ok(chain)
}

pub(crate) fn current_escrow(
    connection: &Connection,
    organization: Uuid,
) -> Result<Option<EscrowKey>, ServerError> {
    Ok(escrow_chain(connection, organization)?.pop())
}

pub(crate) fn pinned_escrow(
    connection: &Connection,
    account: Uuid,
    organization: Uuid,
) -> Result<Vec<u8>, ServerError> {
    Ok(connection
        .query_row(
            "SELECT sealed FROM pinned_escrows WHERE account_id = ?1 AND organization_id = ?2",
            params![account.to_string(), organization.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or_default())
}

pub(crate) fn pin_escrow(
    connection: &Connection,
    account: Uuid,
    organization: Uuid,
    sealed: &[u8],
) -> Result<(), ServerError> {
    if sealed.is_empty() {
        return Ok(());
    }
    connection.execute(
        "INSERT OR REPLACE INTO pinned_escrows (account_id, organization_id, sealed)
         VALUES (?1, ?2, ?3)",
        params![account.to_string(), organization.to_string(), sealed],
    )?;
    Ok(())
}

pub(crate) fn organization_kind(
    connection: &Connection,
    organization: Uuid,
) -> Result<OrganizationKind, ServerError> {
    let kind: String = connection.query_row(
        "SELECT kind FROM organizations WHERE id = ?1",
        [organization.to_string()],
        |row| row.get(0),
    )?;
    crate::store::decode_kind(&kind)
}

fn latest_scope_key(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Option<Uuid>, ServerError> {
    Ok(keys_of_block(connection, organization, block)?
        .into_iter()
        .filter(|row| row.kind == KeyKind::Scope)
        .max_by_key(|row| row.epoch)
        .map(|row| row.id))
}

pub(crate) fn is_viewer(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    account: Uuid,
    block: Uuid,
) -> Result<bool, ServerError> {
    if account == crate::organization::server_account(connection)? {
        return Ok(graph.inherited(block, account).can_view()
            || crate::organization::in_settings(graph, organization, block));
    }
    Ok(
        match crate::contacts::role_in(connection, account, organization)? {
            None => false,
            Some(be_protocol::OrganizationRole::Administrator) => graph.contains(block),
            Some(be_protocol::OrganizationRole::Editor) => {
                graph.inherited(block, account).can_view()
                    || crate::organization::in_settings(graph, organization, block)
            }
        },
    )
}

pub(crate) fn sealed_to(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    block: Uuid,
) -> Result<BTreeSet<Uuid>, ServerError> {
    let Some(node) = graph.get(block) else {
        return Ok(BTreeSet::new());
    };
    let mut accounts: BTreeSet<Uuid> = node
        .grants
        .iter()
        .filter(|(_, level)| level.can_view())
        .map(|(account, _)| *account)
        .collect();
    if !matches!(node.parent, be_graph::BlockParent::Block(_)) {
        accounts.insert(node.author);
    }
    if node.is_open_inside()
        && let Some(owner) = graph.settings_owner(block)
    {
        accounts.extend(graph.inside_viewers(owner));
    }
    if crate::organization::in_settings(graph, organization, block) {
        accounts.insert(crate::organization::server_account(connection)?);
        accounts.extend(administrators(connection, organization)?);
    }
    let mut viewers = BTreeSet::new();
    for account in accounts {
        if is_viewer(connection, graph, organization, account, block)? {
            viewers.insert(account);
        }
    }
    Ok(viewers)
}

pub(crate) fn waiting_for_key(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    account: Uuid,
) -> Result<bool, ServerError> {
    let Some(key) = latest_scope_key(connection, organization, block)? else {
        return Ok(false);
    };
    Ok(!wraps_of(connection, organization, key)?
        .iter()
        .any(|wrap| wrap.wrapped_by == WrappedBy::Account(account)))
}

pub(crate) fn lost_view(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    account: Uuid,
) -> Result<(), ServerError> {
    connection.execute(
        "INSERT OR IGNORE INTO rotations (organization_id, block_id) VALUES (?1, ?2)",
        params![organization.to_string(), block.to_string()],
    )?;
    connection.execute(
        "DELETE FROM wrapped_keys
         WHERE organization_id = ?1 AND wrapped_by = ?3
            AND key_id IN (SELECT id FROM keys WHERE organization_id = ?1 AND block_id = ?2)",
        params![
            organization.to_string(),
            block.to_string(),
            encode_wrapped_by(WrappedBy::Account(account))
        ],
    )?;
    Ok(())
}

pub(crate) fn open_kinds_viewed(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    account: Uuid,
    block: Uuid,
) -> Result<BTreeSet<Uuid>, ServerError> {
    let mut viewed = BTreeSet::new();
    for owner in std::iter::once(block).chain(graph.ancestors(block)) {
        for kind in graph.kinds_of(owner) {
            if graph
                .get(kind)
                .is_some_and(be_graph::BlockNode::is_open_inside)
                && is_viewer(connection, graph, organization, account, kind)?
            {
                viewed.insert(kind);
            }
        }
    }
    Ok(viewed)
}

pub(crate) fn lost_open_kinds(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    account: Uuid,
    viewed: BTreeSet<Uuid>,
) -> Result<(), ServerError> {
    for kind in viewed {
        if !is_viewer(connection, graph, organization, account, kind)? {
            lost_view(connection, organization, kind, account)?;
        }
    }
    Ok(())
}

pub(crate) struct ViewingBelow {
    block: Uuid,
    accounts: Vec<(Uuid, bool, BTreeSet<Uuid>)>,
}

pub(crate) fn viewing_below(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    block: Uuid,
) -> Result<ViewingBelow, ServerError> {
    let mut candidates = graph.viewers(block);
    for inside in graph.subtree(block) {
        let Some(node) = graph.get(inside) else {
            continue;
        };
        candidates.insert(node.author);
        candidates.extend(
            node.grants
                .iter()
                .filter(|(_, level)| level.can_view())
                .map(|(account, _)| *account),
        );
    }
    candidates.remove(&be_graph::ANYONE_INSIDE);
    let mut accounts = Vec::new();
    for account in candidates {
        let viewed = is_viewer(connection, graph, organization, account, block)?;
        let kinds = open_kinds_viewed(connection, graph, organization, account, block)?;
        if viewed || !kinds.is_empty() {
            accounts.push((account, viewed, kinds));
        }
    }
    Ok(ViewingBelow { block, accounts })
}

pub(crate) fn lost_below(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    before: ViewingBelow,
) -> Result<BTreeSet<Uuid>, ServerError> {
    let mut lost = BTreeSet::new();
    for (account, viewed, kinds) in before.accounts {
        if viewed && !is_viewer(connection, graph, organization, account, before.block)? {
            lost_view(connection, organization, before.block, account)?;
            lost.insert(account);
        }
        for kind in kinds {
            if !is_viewer(connection, graph, organization, account, kind)? {
                lost_view(connection, organization, kind, account)?;
                lost.insert(account);
            }
        }
    }
    Ok(lost)
}

pub(crate) fn rotate_escrow_later(
    connection: &Connection,
    organization: Uuid,
    account: Uuid,
) -> Result<(), ServerError> {
    lost_view(connection, organization, organization, account)
}

fn administrators(connection: &Connection, organization: Uuid) -> Result<Vec<Uuid>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT account_id FROM memberships WHERE organization_id = ?1 AND role = 'administrator'",
    )?;
    let rows = statement.query_map([organization.to_string()], |row| row.get::<_, String>(0))?;
    let mut found = Vec::new();
    for row in rows {
        found.push(parse_uuid(&row?)?);
    }
    Ok(found)
}

fn pending_rotations(
    connection: &Connection,
    organization: Uuid,
) -> Result<Vec<Uuid>, ServerError> {
    let mut statement =
        connection.prepare_cached("SELECT block_id FROM rotations WHERE organization_id = ?1")?;
    let rows = statement.query_map([organization.to_string()], |row| row.get::<_, String>(0))?;
    let mut found = Vec::new();
    for row in rows {
        found.push(parse_uuid(&row?)?);
    }
    Ok(found)
}

fn rotated(connection: &Connection, organization: Uuid, block: Uuid) -> Result<(), ServerError> {
    connection.execute(
        "DELETE FROM rotations WHERE organization_id = ?1 AND block_id = ?2",
        params![organization.to_string(), block.to_string()],
    )?;
    Ok(())
}

pub(crate) fn forget_block_keys(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<(), ServerError> {
    for row in keys_of_block(connection, organization, block)? {
        connection.execute(
            "DELETE FROM wrapped_keys
             WHERE organization_id = ?1 AND (key_id = ?2 OR wrapped_by = ?2)",
            params![organization.to_string(), row.id.to_string()],
        )?;
    }
    connection.execute(
        "DELETE FROM keys WHERE organization_id = ?1 AND block_id = ?2",
        params![organization.to_string(), block.to_string()],
    )?;
    Ok(())
}

impl ServerStore {
    pub async fn get_keys(
        &self,
        identity: Identity,
        blocks: Option<Vec<Uuid>>,
    ) -> Result<KeySet, ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let seen = visibility(identity);
            self.server_key().seal_settings_to(
                database,
                graph,
                identity.organization,
                identity.account,
            )?;
            let settings = crate::organization::settings_of(identity.organization);
            let wanted: BTreeSet<Uuid> = match blocks {
                None if seen.administrator() => graph.ids().into_iter().collect(),
                None => graph
                    .visible(identity.account)
                    .into_keys()
                    .chain(match graph.contains(settings) {
                        true => graph.subtree(settings),
                        false => Vec::new(),
                    })
                    .collect(),
                Some(blocks) => blocks
                    .into_iter()
                    .filter(|block| graph.contains(*block))
                    .flat_map(|block| {
                        std::iter::once(block)
                            .chain(graph.ancestors(block))
                            .collect::<Vec<_>>()
                    })
                    .collect(),
            };
            served_keys(graph, database, identity, &seen, wanted)
        })
        .await
    }

    pub async fn put_keys(&self, identity: Identity, set: KeySet) -> Result<(), ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let seen = visibility(identity);
            let transaction = database.unchecked_transaction()?;
            store_keys(
                graph,
                &transaction,
                identity.organization,
                identity.account,
                &set,
                |graph, block| seen.can_edit(graph, block),
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    pub async fn key_work(&self, identity: Identity) -> Result<KeyWork, ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let organization = identity.organization;
            let seen = visibility(identity);
            let escrows = escrow_chain(database, organization)?;
            let escrow = escrows.last().map(|escrow| escrow.id);
            let mut work = KeyWork {
                escrows,
                ..KeyWork::default()
            };
            let pending = pending_rotations(database, organization)?;
            for block in &pending {
                if *block == organization {
                    work.escrow_rotation = seen.administrator();
                } else if !graph.contains(*block) {
                    rotated(database, organization, *block)?;
                } else if seen.can_edit(graph, *block) {
                    work.rotations.push(*block);
                }
            }
            let rotating: BTreeSet<Uuid> = work.rotations.iter().copied().collect();
            work.rotations.retain(|block| {
                !graph
                    .ancestors(*block)
                    .any(|above| rotating.contains(&above))
            });
            let server = crate::organization::server_account(database)?;
            let candidates: Vec<Uuid> = match seen.administrator() {
                true => graph.ids(),
                false => graph.visible(identity.account).into_keys().collect(),
            };
            let contacts = crate::contacts::contacts_of(database, identity.account)?;
            for block in candidates {
                if !graph.is_scope_root(block) {
                    continue;
                }
                let editor = seen.can_edit(graph, block);
                if !editor && !passes_on(graph, &seen, block) {
                    continue;
                }
                let Some(key) = latest_scope_key(database, organization, block)? else {
                    continue;
                };
                let wraps = wraps_of(database, organization, key)?;
                for account in sealed_to(database, graph, organization, block)? {
                    if !editor && !contacts.contains(&account) {
                        continue;
                    }
                    if !wraps.iter().any(|wrap| {
                        wrap.wrapped_by == WrappedBy::Account(account) && wrap.sender != server
                    }) {
                        work.unsealed.push(Unsealed { key, account });
                    }
                }
                if editor
                    && let Some(escrow) = escrow
                    && !wraps
                        .iter()
                        .any(|wrap| wrap.wrapped_by == WrappedBy::Key(escrow))
                {
                    work.unescrowed.push(key);
                }
            }
            if seen.administrator()
                && let Some(escrow) = escrow
            {
                let wraps = wraps_of(database, organization, escrow)?;
                for account in administrators(database, organization)? {
                    if !wraps
                        .iter()
                        .any(|wrap| wrap.wrapped_by == WrappedBy::Account(account))
                    {
                        work.unsealed.push(Unsealed {
                            key: escrow,
                            account,
                        });
                    }
                }
            }
            Ok(work)
        })
        .await
    }

    pub async fn rotate(
        &self,
        identity: Identity,
        roots: Vec<Uuid>,
        set: KeySet,
    ) -> Result<(), ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let organization = identity.organization;
            let seen = visibility(identity);
            let data: BTreeSet<Uuid> = set
                .keys
                .iter()
                .filter(|row| row.kind == KeyKind::Data)
                .map(|row| row.block)
                .collect();
            let scopes: BTreeSet<Uuid> = set
                .keys
                .iter()
                .filter(|row| row.kind == KeyKind::Scope)
                .map(|row| row.block)
                .collect();
            let mut covered = BTreeSet::new();
            for root in &roots {
                if !seen.can_edit(graph, *root) {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("block {root} is not editable by this account"),
                    ));
                }
                for block in graph.subtree(*root) {
                    if !data.contains(&block) {
                        return Err(refused(format!(
                            "a rotation gives every block below it a new data key, and {block} has none"
                        )));
                    }
                    if graph.is_scope_root(block) && !scopes.contains(&block) {
                        return Err(refused(format!(
                            "a rotation gives every scope below it a new key, and {block} has none"
                        )));
                    }
                    covered.insert(block);
                }
            }
            let transaction = database.unchecked_transaction()?;
            store_keys(
                graph,
                &transaction,
                organization,
                identity.account,
                &set,
                |graph, block| seen.can_edit(graph, block),
            )?;
            for block in pending_rotations(&transaction, organization)? {
                if covered.contains(&block) {
                    rotated(&transaction, organization, block)?;
                }
            }
            for block in &covered {
                if !graph.is_scope_root(*block) {
                    continue;
                }
                for row in keys_of_block(&transaction, organization, *block)? {
                    if row.kind != KeyKind::Scope {
                        continue;
                    }
                    for wrap in wraps_of(&transaction, organization, row.id)? {
                        let WrappedBy::Account(account) = wrap.wrapped_by else {
                            continue;
                        };
                        if !is_viewer(&transaction, graph, organization, account, *block)? {
                            transaction.execute(
                                "DELETE FROM wrapped_keys
                                 WHERE organization_id = ?1 AND key_id = ?2 AND wrapped_by = ?3",
                                params![
                                    organization.to_string(),
                                    row.id.to_string(),
                                    encode_wrapped_by(wrap.wrapped_by)
                                ],
                            )?;
                        }
                    }
                }
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    pub async fn create_escrow(
        &self,
        identity: Identity,
        escrow: EscrowKey,
        set: KeySet,
    ) -> Result<(), ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let organization = identity.organization;
            let seen = visibility(identity);
            if !seen.administrator()
                || organization_kind(database, organization)? != OrganizationKind::Company
            {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "only an administrator of a company organization makes its escrow key".into(),
                ));
            }
            let next = current_escrow(database, organization)?.map_or(1, |previous| previous.epoch + 1);
            if escrow.epoch != next {
                return Err(refused(format!(
                    "the next escrow key is epoch {next}, not {}",
                    escrow.epoch
                )));
            }
            let transaction = database.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO keys
                    (organization_id, id, block_id, kind, epoch, created, public, signing, signature)
                 VALUES (?1, ?2, ?1, 'escrow', ?3, ?4, ?5, ?6, ?7)",
                params![
                    organization.to_string(),
                    escrow.id.to_string(),
                    i64::try_from(escrow.epoch).map_err(|_| refused("that epoch is too large"))?,
                    now_milliseconds(),
                    escrow.public.to_vec(),
                    escrow.signing.to_vec(),
                    escrow.signature
                ],
            )?;
            store_keys(
                graph,
                &transaction,
                organization,
                identity.account,
                &set,
                |graph, block| seen.can_edit(graph, block),
            )?;
            let older: Vec<Uuid> = keys_of_block(&transaction, organization, organization)?
                .into_iter()
                .filter(|row| row.id != escrow.id)
                .map(|row| row.id)
                .collect();
            for wrap in &set.wrapped {
                if wrap.wrapped_by != WrappedBy::Key(escrow.id) {
                    continue;
                }
                for old in &older {
                    transaction.execute(
                        "DELETE FROM wrapped_keys
                         WHERE organization_id = ?1 AND key_id = ?2 AND wrapped_by = ?3",
                        params![
                            organization.to_string(),
                            wrap.key.to_string(),
                            old.to_string()
                        ],
                    )?;
                }
            }
            rotated(&transaction, organization, organization)?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    pub async fn pin_escrow(&self, identity: Identity, sealed: Vec<u8>) -> Result<(), ServerError> {
        let database = self.database().lock().await;
        pin_escrow(&database, identity.account, identity.organization, &sealed)
    }

    pub async fn recovery_key(
        &self,
        account: Uuid,
    ) -> Result<(Option<[u8; 32]>, Vec<SealedKey>, Vec<OwnSealed>), ServerError> {
        let (public, memberships) = {
            let database = self.database().lock().await;
            let public = recovery_key_of(&database, account)?;
            let mut statement = database
                .prepare("SELECT organization_id, role FROM memberships WHERE account_id = ?1")?;
            let rows = statement.query_map([account.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut memberships = Vec::new();
            for row in rows {
                let (organization, role) = row?;
                memberships.push(Identity {
                    account,
                    organization: parse_uuid(&organization)?,
                    role: decode_role(&role)?,
                });
            }
            (public, memberships)
        };
        let mut sealed = Vec::new();
        for identity in memberships {
            let found = self
                .with_graph(identity.organization, move |graph, database| {
                    let seen = visibility(identity);
                    let mut statement = database.prepare(
                        "SELECT wrapped_keys.key_id, keys.block_id, wrapped_keys.sender,
                                wrapped_keys.sealed
                         FROM wrapped_keys JOIN keys
                            ON keys.organization_id = wrapped_keys.organization_id
                            AND keys.id = wrapped_keys.key_id
                         WHERE wrapped_keys.wrapped_by = ?1 AND wrapped_keys.organization_id = ?2",
                    )?;
                    let rows = statement.query_map(
                        params![
                            encode_wrapped_by(WrappedBy::Account(account)),
                            identity.organization.to_string()
                        ],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, Vec<u8>>(3)?,
                            ))
                        },
                    )?;
                    let mut found = Vec::new();
                    for row in rows {
                        let (key, block, sender, bytes) = row?;
                        let block = parse_uuid(&block)?;
                        if seen.can_view(graph, block)
                            || (block == identity.organization && seen.administrator())
                        {
                            found.push(SealedKey {
                                organization: identity.organization,
                                key: parse_uuid(&key)?,
                                sender: parse_uuid(&sender)?,
                                sealed: bytes,
                            });
                        }
                    }
                    Ok(found)
                })
                .await?;
            sealed.extend(found);
        }
        let own = crate::contacts::own_sealed(&*self.database().lock().await, account)?;
        Ok((public, sealed, own))
    }

    pub async fn set_recovery_key(
        &self,
        account: Uuid,
        public: [u8; 32],
        sealed: Vec<SealedKey>,
        own: Vec<OwnSealed>,
        changes: Vec<KeyChange>,
    ) -> Result<(), ServerError> {
        let (previous, held, mut held_own) = self.recovery_key(account).await?;
        if previous.is_none() {
            held_own.clear();
        }
        let resealed: BTreeMap<(Uuid, Uuid), &SealedKey> = sealed
            .iter()
            .map(|key| ((key.organization, key.key), key))
            .collect();
        let by = encode_wrapped_by(WrappedBy::Account(account));
        let database = self.database().lock().await;
        let mut needed = Vec::new();
        for held in &held {
            if held.sender == account
                || crate::contacts::is_verified_contact(&database, account, held.sender)?
            {
                needed.push(held);
            }
        }
        if let Some(missing) = needed
            .iter()
            .find(|held| !resealed.contains_key(&(held.organization, held.key)))
        {
            return Err(refused(format!(
                "a new recovery key must seal every key the old one did, and {} is missing",
                missing.key
            )));
        }
        let resealed_own: BTreeMap<(OwnKind, Uuid), &OwnSealed> = own
            .iter()
            .map(|sealed| ((sealed.kind, sealed.id), sealed))
            .collect();
        if let Some(missing) = held_own
            .iter()
            .find(|held| !resealed_own.contains_key(&(held.kind, held.id)))
        {
            return Err(refused(format!(
                "a new recovery key must reseal every contact and link the old one did, and {} is missing",
                missing.id
            )));
        }
        for change in &changes {
            if !crate::contacts::is_contact(&database, account, change.contact)? {
                return Err(ServerError::Refused(
                    ErrorCode::NotAContact,
                    format!("{} is not one of your contacts", change.contact),
                ));
            }
        }
        let transaction = database.unchecked_transaction()?;
        for change in &changes {
            transaction.execute(
                "INSERT INTO key_changes (account_id, contact_id, position, sealed)
                 VALUES (?1, ?2,
                    (SELECT COALESCE(MAX(position), -1) + 1 FROM key_changes
                     WHERE account_id = ?1 AND contact_id = ?2),
                    ?3)",
                params![
                    account.to_string(),
                    change.contact.to_string(),
                    change.sealed
                ],
            )?;
        }
        for held in &held_own {
            crate::contacts::reseal_own(
                &transaction,
                account,
                resealed_own[&(held.kind, held.id)],
            )?;
        }
        transaction.execute(
            "INSERT OR REPLACE INTO recovery_keys (account_id, public) VALUES (?1, ?2)",
            params![account.to_string(), public.to_vec()],
        )?;
        transaction.execute("DELETE FROM wrapped_keys WHERE wrapped_by = ?1", [&by])?;
        for held in &held {
            let Some(key) = resealed.get(&(held.organization, held.key)) else {
                continue;
            };
            transaction.execute(
                "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    key.organization.to_string(),
                    key.key.to_string(),
                    by,
                    account.to_string(),
                    key.sealed
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub async fn reseal_keys(
        &self,
        account: Uuid,
        keys: Vec<SealedKey>,
    ) -> Result<(), ServerError> {
        let by = encode_wrapped_by(WrappedBy::Account(account));
        let database = self.database().lock().await;
        let transaction = database.unchecked_transaction()?;
        for key in &keys {
            let updated = transaction.execute(
                "UPDATE wrapped_keys SET sealed = ?4, sender = ?5
                 WHERE organization_id = ?1 AND key_id = ?2 AND wrapped_by = ?3",
                params![
                    key.organization.to_string(),
                    key.key.to_string(),
                    by,
                    key.sealed,
                    account.to_string()
                ],
            )?;
            if updated == 0 {
                return Err(refused(format!(
                    "{} is not sealed to you, so you cannot seal it again",
                    key.key
                )));
            }
        }
        transaction.commit()?;
        Ok(())
    }
}
