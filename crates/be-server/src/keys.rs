use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use be_block_graph::BlockGraph;
use be_block_protocol::{
    ErrorCode, EscrowKey, KeyChange, KeyKind, KeyPage, KeyRow, KeySet, KeyWork, OrganizationKind,
    SealedKey, SigningKey, Unsealed, WrappedBy, WrappedKey,
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

/// The key rows `query.keys` serves of `blocks`, by id: those of the blocks the caller may view,
/// and to an administrator the organization's escrow keys.
fn served_rows(
    graph: &BlockGraph,
    connection: &Connection,
    identity: Identity,
    seen: &Visibility,
    blocks: impl IntoIterator<Item = Uuid>,
) -> Result<BTreeMap<Uuid, KeyRow>, ServerError> {
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
    Ok(keys)
}

/// The rows of `served` after `after`, in the order of their ids, each with its wraps by the
/// caller's account or by a served key, while they fit in `room` bytes and at least one. Only
/// the page's own wraps are read.
fn page_of(
    connection: &Connection,
    identity: Identity,
    served: &BTreeMap<Uuid, KeyRow>,
    after: Option<Uuid>,
    mut room: usize,
) -> Result<KeyPage, ServerError> {
    let rows = match after {
        Some(after) => served.range((Bound::Excluded(after), Bound::Unbounded)),
        None => served.range(..),
    };
    let mut page = KeySet::default();
    for (id, row) in rows {
        let wrapped: Vec<WrappedKey> = wraps_of(connection, identity.organization, *id)?
            .into_iter()
            .filter(|wrap| match wrap.wrapped_by {
                WrappedBy::Account(account) => account == identity.account,
                WrappedBy::Key(by) => served.contains_key(&by),
            })
            .collect();
        let size = be_block_protocol::encoded_len(row) + be_block_protocol::encoded_len(&wrapped);
        if size > room && !page.keys.is_empty() {
            let next = page.keys.last().map(|row| row.id);
            return Ok(KeyPage { keys: page, next });
        }
        room = room.saturating_sub(size);
        page.keys.push(row.clone());
        page.wrapped.extend(wrapped);
    }
    Ok(KeyPage {
        keys: page,
        next: None,
    })
}

/// Whether `row` is the scope key of an owner-only block, which is sealed to the block's author
/// alone: never wrapped by another key, sealed to the escrow key or to anyone else.
pub(crate) fn owner_only_scope(graph: &BlockGraph, row: &KeyRow) -> bool {
    row.kind == KeyKind::Scope && graph.get(row.block).is_some_and(|node| node.owner_only)
}

fn passes_on(graph: &BlockGraph, seen: &Visibility, kind: Uuid) -> bool {
    graph
        .get(kind)
        .is_some_and(be_block_graph::BlockNode::is_open_inside)
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
        .is_some_and(be_block_graph::BlockNode::is_open_inside);
    Ok(row.kind == KeyKind::Scope
        && open
        && account != caller
        && is_viewer(connection, graph, organization, caller, row.block)?
        && sealed_to(connection, graph, organization, row.block)?.contains(&account))
}

/// What `store_keys` stored, so whoever it may open something for can be told.
#[derive(Default)]
pub(crate) struct Stored {
    /// Whether it inserted or changed any row.
    pub any: bool,
    /// Blocks with a new key, or a key newly wrapped under one that is not the escrow key:
    /// what their viewers may now open.
    pub opened: BTreeSet<Uuid>,
    /// Keys newly sealed to an account: the block each is of, and the account.
    pub sealed: BTreeSet<(Uuid, Uuid)>,
}

pub(crate) fn store_keys(
    graph: &mut BlockGraph,
    connection: &Connection,
    organization: Uuid,
    caller: Uuid,
    set: &KeySet,
    allowed: impl Fn(&BlockGraph, Uuid) -> bool,
) -> Result<Stored, ServerError> {
    let administrator = crate::contacts::role_in(connection, caller, organization)?
        .is_some_and(|role| role == be_block_protocol::OrganizationRole::Administrator);
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
        if be_block_protocol::owner_only_key(row.id) != owner_only_scope(graph, row) {
            return Err(refused(format!(
                "key {} has an owner-only id exactly when it is the scope key of an owner-only block",
                row.id
            )));
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
        if owner_only_scope(graph, &row) {
            let author = graph.get(row.block).map(|node| node.author);
            if wrap.wrapped_by != WrappedBy::Account(caller) || author != Some(caller) {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    format!(
                        "key {} belongs to a block readable by its author alone, so it is sealed only to its author",
                        row.id
                    ),
                ));
            }
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
                if !crate::store::holds_membership(connection, account, organization)? {
                    return Err(refused(format!(
                        "account {account} is not a member of this organization"
                    )));
                }
                // To a contact, or to a grantee the block's log names, with
                // the keys its grant names, which a device checks as the server does.
                let granted = row.kind == KeyKind::Scope
                    && crate::access::granted_level(connection, organization, row.block, account)?
                        .is_some_and(|level| level >= be_block_keys::Level::View);
                if account != caller
                    && !granted
                    && !crate::contacts::is_contact(connection, caller, account)?
                {
                    return Err(ServerError::Refused(
                        ErrorCode::NotAContact,
                        format!(
                            "a key is sealed only to a contact or a grantee its block's log names, and {account} is neither"
                        ),
                    ));
                }
                if row.kind == KeyKind::Escrow
                    && crate::contacts::role_in(connection, account, organization)?
                        != Some(be_block_protocol::OrganizationRole::Administrator)
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
    let mut stored = Stored {
        any: !fresh.is_empty(),
        opened: fresh.values().map(|row| row.block).collect(),
        sealed: BTreeSet::new(),
    };
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
    let rekeyed: Vec<Uuid> = current.keys().copied().collect();
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
    // A summary names its block's key, so a block with a new one changed.
    if !rekeyed.is_empty() {
        crate::changes::touch(connection, graph, organization, &rekeyed)?;
    }
    for wrap in &set.wrapped {
        let changed = connection.execute(
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
        let block = known(wrap.key)?.map(|row| row.block);
        if changed > 0 {
            stored.any = true;
        }
        let WrappedBy::Key(by) = wrap.wrapped_by else {
            if let (true, Some(block), WrappedBy::Account(account)) =
                (changed > 0, block, wrap.wrapped_by)
            {
                stored.sealed.insert((block, account));
            }
            continue;
        };
        if known(by)?.is_some_and(|row| row.kind == KeyKind::Escrow) {
            continue;
        }
        if changed > 0
            && let Some(block) = block
        {
            stored.opened.insert(block);
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
    Ok(stored)
}

pub(crate) fn escrow_chain(
    connection: &Connection,
    organization: Uuid,
) -> Result<Vec<EscrowKey>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT id, epoch, public, maker, signature, head FROM keys
         WHERE organization_id = ?1 AND block_id = ?1 AND kind = 'escrow'
         ORDER BY epoch",
    )?;
    let rows = statement.query_map([organization.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Vec<u8>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Vec<u8>>(4)?,
            row.get::<_, Vec<u8>>(5)?,
        ))
    })?;
    let mut chain = Vec::new();
    for row in rows {
        let (id, epoch, public, maker, signature, head) = row?;
        chain.push(EscrowKey {
            id: parse_uuid(&id)?,
            epoch: u64::try_from(epoch).map_err(|_| ServerError::Corrupt)?,
            public: public.try_into().map_err(|_| ServerError::Corrupt)?,
            head: head.try_into().map_err(|_| ServerError::Corrupt)?,
            maker: parse_uuid(&maker)?,
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

/// Leaves what `account` pins for `organization`, sealed by it for itself, in its mailbox from
/// `from` (`letter.pinned`), for a device to keep in the account's block.
pub(crate) fn pin_escrow(
    connection: &Connection,
    account: Uuid,
    from: Uuid,
    organization: Uuid,
    sealed: &[u8],
) -> Result<(), ServerError> {
    if sealed.is_empty() {
        return Ok(());
    }
    crate::limits::at_most(
        "a pinned escrow",
        sealed.len(),
        crate::limits::MAX_SEALED_BYTES,
    )?;
    crate::store::mailbox::leave(
        connection,
        account,
        Some(from),
        &be_block_protocol::Letter::Pinned {
            organization,
            sealed: sealed.to_vec(),
        },
    )
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

/// For an administrator's device: the newest data key of each block inside the organization's
/// settings (the policies blocks), for each administrator it is not sealed to. A device that
/// trusts the key seals it to them as itself, so another administrator writes those blocks under
/// a key it trusts: wrapped under the settings key, which the server holds, it vouches for
/// nothing (guides/blocks/keys.md, Trust). Owner-only blocks are never listed.
fn settings_unsealed(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    server: Uuid,
    work: &mut KeyWork,
) -> Result<(), ServerError> {
    let settings = be_block::organization_settings(organization);
    let administrators = administrators(connection, organization)?;
    for block in graph.ids() {
        let inside = block == settings || graph.ancestors(block).any(|above| above == settings);
        let owner_only = std::iter::once(block)
            .chain(graph.ancestors(block))
            .any(|above| graph.get(above).is_some_and(|node| node.owner_only));
        if !inside || owner_only {
            continue;
        }
        let Some(key) = latest_data_key(connection, organization, block)? else {
            continue;
        };
        let wraps = wraps_of(connection, organization, key)?;
        for account in &administrators {
            if !wraps.iter().any(|wrap| {
                wrap.wrapped_by == WrappedBy::Account(*account) && wrap.sender != server
            }) {
                work.unsealed.push(Unsealed {
                    key,
                    account: *account,
                });
            }
        }
    }
    Ok(())
}

fn latest_data_key(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Option<Uuid>, ServerError> {
    Ok(keys_of_block(connection, organization, block)?
        .into_iter()
        .filter(|row| row.kind == KeyKind::Data)
        .max_by_key(|row| row.epoch)
        .map(|row| row.id))
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

/// What a write must leave whole, checked once at its end (`ServerStore::check_whole`).
pub(crate) enum Whole {
    /// `block` was made naming `key` its data key.
    Made { block: Uuid, key: Uuid },
    /// `block` was moved from below the scope `scope_before` (none from the top or the trash).
    Moved {
        block: Uuid,
        scope_before: Option<Uuid>,
    },
}

fn made_whole(
    connection: &Connection,
    identity: Identity,
    block: Uuid,
    key: Uuid,
) -> Result<(), ServerError> {
    match key_row(connection, identity.organization, key)? {
        Some(row) if row.block == block && row.kind == KeyKind::Data => Ok(()),
        _ => Err(refused(format!(
            "block {block} is made with its data key {key}, stored in the same write"
        ))),
    }
}

/// Whether the move of `block` left every key it took into a new scope opening from that
/// scope's latest key: the block's own scope key if it is a scope whose key does, else its data
/// key and, below it, each block's data key, or a nested scope's key, of what the mover may view
/// (`Peer::rescope` sends these). A move within one scope, to the top or the trash, of a block
/// readable by its author alone, or into a scope with no key, takes no key anywhere new. What
/// lies in a block readable by its author alone is skipped, as `rescope` skips an owner-only
/// block and what is below it; a nested scope moved out of one is not, and `rescope` wraps its
/// key under the new scope too. Only the scope's latest key counts: a key wrapped under an older
/// one (a block trashed while its scope rotated) is wrapped again. A few queries a block moved,
/// inside the write's transaction, as `rescope` lists and wraps each one.
fn moved_whole(
    connection: &Connection,
    graph: &BlockGraph,
    seen: &Visibility,
    identity: Identity,
    block: Uuid,
    scope_before: Option<Uuid>,
) -> Result<(), ServerError> {
    let organization = identity.organization;
    let Some(be_block_graph::BlockParent::Block(parent)) = graph.get(block).map(|node| node.parent)
    else {
        return Ok(());
    };
    let scope = graph.scope(parent);
    if scope_before == Some(scope) || graph.owner_only_root(block).is_some() {
        return Ok(());
    }
    let Some(under) = latest_scope_key(connection, organization, scope)? else {
        return Ok(());
    };
    let opens = |key: Uuid| opens_from(connection, organization, key, under);
    let unwrapped = |key: Uuid| {
        ServerError::Refused(
            ErrorCode::KeyMissing,
            format!("a move into scope {scope} wraps key {key} under that scope's key {under}"),
        )
    };
    if graph.is_scope_root(block)
        && let Some(own) = latest_scope_key(connection, organization, block)?
        && opens(own)?
    {
        return Ok(());
    }
    let mut level = vec![block];
    while !level.is_empty() {
        let mut below = Vec::new();
        for node in level {
            if node != block {
                if !seen.can_view(graph, node) || graph.owner_only_root(node).is_some() {
                    continue;
                }
                if graph.is_scope_root(node) {
                    if let Some(nested) = latest_scope_key(connection, organization, node)?
                        && !opens(nested)?
                    {
                        return Err(unwrapped(nested));
                    }
                    continue;
                }
            }
            if let Some(data) = latest_data_key(connection, organization, node)?
                && !opens(data)?
            {
                return Err(unwrapped(data));
            }
            below.extend(graph.children(be_block_graph::BlockParent::Block(node)));
        }
        level = below;
    }
    Ok(())
}

/// Whether `key` opens from `under`: wrapped under it, or under a key that does.
fn opens_from(
    connection: &Connection,
    organization: Uuid,
    key: Uuid,
    under: Uuid,
) -> Result<bool, ServerError> {
    let mut seen = BTreeSet::from([key]);
    let mut next = vec![key];
    while let Some(key) = next.pop() {
        for wrap in wraps_of(connection, organization, key)? {
            let WrappedBy::Key(by) = wrap.wrapped_by else {
                continue;
            };
            if by == under {
                return Ok(true);
            }
            if seen.insert(by) {
                next.push(by);
            }
        }
    }
    Ok(false)
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
            Some(be_block_protocol::OrganizationRole::Administrator) => {
                graph.contains(block) && !graph.closed_to(block, account)
            }
            Some(
                be_block_protocol::OrganizationRole::Editor
                | be_block_protocol::OrganizationRole::Guest,
            ) => {
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
    if !matches!(node.parent, be_block_graph::BlockParent::Block(_)) || node.owner_only {
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
                .is_some_and(be_block_graph::BlockNode::is_open_inside)
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
    candidates.remove(&be_block_graph::ANYONE_INSIDE);
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
        "SELECT account_id FROM memberships WHERE organization_id = ?1 AND role = 'administrator'
            AND account_id NOT IN (SELECT account_id FROM agents WHERE bound_to IS NOT NULL)",
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

impl ServerStore {
    /// A page of the keys `query.keys` serves: those after `after` in the order of their ids, each
    /// with its wraps, while they fit in `ServerConfig::key_page_bytes` (at least one). A key made with a
    /// smaller id while a client pages is not skipped for good: `topic.keys` tells it to
    /// read again.
    pub async fn get_keys(
        &self,
        identity: Identity,
        blocks: Option<Vec<Uuid>>,
        after: Option<Uuid>,
    ) -> Result<KeyPage, ServerError> {
        self.with_graph(identity.organization, move |store, graph, database| {
            let seen = visibility(identity);
            if identity.bound.is_none() {
                store.server_key().seal_settings_to(
                    database,
                    graph,
                    identity.organization,
                    identity.account,
                )?;
            }
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
            let served = served_rows(graph, database, identity, &seen, wanted)?;
            page_of(database, identity, &served, after, store.key_page_bytes())
        })
        .await
    }

    /// Checks what a write must leave whole, once it wrote everything (`Whole`): a block it
    /// made has the data key it names, and a move into another scope left every key it took
    /// there opening from the new scope's key. A refusal keeps the cached graph, which the
    /// write's rollback then reads again.
    pub(crate) async fn check_whole(
        &self,
        identity: Identity,
        check: Whole,
    ) -> Result<(), ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            Ok(match check {
                Whole::Made { block, key } => made_whole(database, identity, block, key),
                Whole::Moved {
                    block,
                    scope_before,
                } => moved_whole(
                    database,
                    graph,
                    &visibility(identity),
                    identity,
                    block,
                    scope_before,
                ),
            })
        })
        .await?
    }

    /// Stores `set` and answers whom to tell (`topic.keys`), none when it stored nothing:
    /// the viewers of each block it gave a key or wrapped one of under another, and each account
    /// it sealed a key to that views the key's block. Administrators are told anyway, which is
    /// all an escrow wrap tells.
    pub async fn put_keys(
        &self,
        identity: Identity,
        set: KeySet,
    ) -> Result<Option<BTreeSet<Uuid>>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            let seen = visibility(identity);
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let stored = store_keys(
                graph,
                &transaction,
                identity.organization,
                identity.account,
                &set,
                |graph, block| seen.can_edit(graph, block),
            )?;
            transaction.commit()?;
            if !stored.any {
                return Ok(None);
            }
            let mut told = BTreeSet::new();
            for block in stored.opened {
                told.extend(graph.viewers(block));
            }
            for (block, account) in stored.sealed {
                if graph.viewers(block).contains(&account) {
                    told.insert(account);
                }
            }
            Ok(Some(told))
        })
        .await
    }

    pub async fn key_work(&self, identity: Identity) -> Result<KeyWork, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            let organization = identity.organization;
            let seen = visibility(identity);
            let escrows = escrow_chain(database, organization)?;
            let escrow = escrows.last().map(|escrow| escrow.id);
            let mut work = KeyWork {
                escrows,
                unsigned_members: unsigned_members(database, organization, identity)?,
                unsigned_revocations: unsigned_revocations(database, graph, organization, &seen)?,
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
                    && !graph.get(block).is_some_and(|node| node.owner_only)
                    && let Some(escrow) = escrow
                    && !wraps
                        .iter()
                        .any(|wrap| wrap.wrapped_by == WrappedBy::Key(escrow))
                {
                    work.unescrowed.push(key);
                }
            }
            if seen.administrator() {
                settings_unsealed(database, graph, organization, server, &mut work)?;
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
        self.with_graph(identity.organization, move |_, graph, database| {
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
                    if graph.closed_to(block, identity.account) {
                        continue;
                    }
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
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
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
        self.with_graph(identity.organization, move |_, graph, database| {
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
            crate::limits::at_most(
                "an escrow signature",
                escrow.signature.len(),
                crate::limits::MAX_SEALED_BYTES,
            )?;
            let made: i64 = database
                .prepare_cached(
                    "SELECT COUNT(*) FROM keys WHERE organization_id = ?1 AND block_id = ?1 AND kind = 'escrow'",
                )?
                .query_row([organization.to_string()], |row| row.get(0))?;
            if made >= crate::limits::MAX_ESCROWS_PER_ORGANIZATION {
                return Err(crate::limits::too_many(
                    "escrow keys one organization makes",
                    crate::limits::MAX_ESCROWS_PER_ORGANIZATION,
                ));
            }
            let previous = current_escrow(database, organization)?;
            let next = previous.as_ref().map_or(1, |previous| previous.epoch + 1);
            if escrow.epoch != next {
                return Err(refused(format!(
                    "the next escrow key is epoch {next}, not {}",
                    escrow.epoch
                )));
            }
            // Checked as devices check it (`verify_escrow`): made and signed by an administrator
            // the organization's log names, chained to the epoch before, under the log's head
            // now, so a demotion is behind every new epoch (guides/blocks/keys.md, "Epochs name
            // their heads").
            let head = crate::access::checked_organization_log(database, organization)?
                .map(|checked| checked.head);
            if head != Some(escrow.head) {
                return Err(refused(
                    "an escrow key names the head of the organization's log it is made under",
                ));
            }
            let maker = crate::access::administrator_signing(database, organization, identity.account)?;
            let chained = escrow.chained(
                organization,
                previous.map_or(Uuid::nil(), |previous| previous.id),
            );
            if escrow.maker != identity.account
                || !maker.is_some_and(|maker| maker.verifies_escrow(&chained, &escrow.signature))
            {
                return Err(refused(
                    "an escrow key is signed by the administrator making it, as the organization's log names them",
                ));
            }
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            transaction.execute(
                "INSERT INTO keys
                    (organization_id, id, block_id, kind, epoch, created, public, maker, signature, head)
                 VALUES (?1, ?2, ?1, 'escrow', ?3, ?4, ?5, ?7, ?6, ?8)",
                params![
                    organization.to_string(),
                    escrow.id.to_string(),
                    i64::try_from(escrow.epoch).map_err(|_| refused("that epoch is too large"))?,
                    now_milliseconds(),
                    escrow.public.to_vec(),
                    escrow.signature,
                    escrow.maker.to_string(),
                    escrow.head.to_vec()
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

    /// The signing keys of those of `accounts` the caller may know: itself, its contacts and
    /// the members of the organization open. A key the server holds is only what the account
    /// sent; a device trusts it only on first use (plans/deferred/authorship.md).
    pub async fn signing_keys(
        &self,
        identity: Identity,
        accounts: Vec<Uuid>,
    ) -> Result<Vec<SigningKey>, ServerError> {
        if accounts.len() > crate::limits::MAX_SIGNING_KEYS_ASKED {
            return Err(crate::limits::too_many(
                "accounts asked for at once",
                crate::limits::MAX_SIGNING_KEYS_ASKED,
            ));
        }
        self.with_database(move |_, database| {
            let mut found = Vec::new();
            for account in accounts.into_iter().collect::<BTreeSet<_>>() {
                let known = account == identity.account
                    || crate::contacts::is_contact(&database, identity.account, account)?
                    || is_member_of(&database, account, identity.organization)?;
                if !known {
                    continue;
                }
                let signing: Option<Vec<u8>> = database
                    .prepare_cached("SELECT signing FROM recovery_keys WHERE account_id = ?1")?
                    .query_row([account.to_string()], |row| row.get(0))
                    .optional()?;
                if let Some(key) = signing.and_then(|key| <[u8; 32]>::try_from(key).ok()) {
                    found.push(SigningKey { account, key });
                }
            }
            Ok(found)
        })
        .await
    }

    pub async fn recovery_key(
        &self,
        account: Uuid,
    ) -> Result<(Option<[u8; 32]>, Vec<SealedKey>), ServerError> {
        let (public, memberships) = {
            self.with_database(move |_, database| {
                let public = recovery_key_of(&database, account)?;
                let mut statement = database.prepare(
                    "SELECT memberships.organization_id, memberships.role, agents.bound_to
                     FROM memberships
                     LEFT JOIN agents ON agents.account_id = memberships.account_id
                        AND agents.organization_id = memberships.organization_id
                     WHERE memberships.account_id = ?1",
                )?;
                let rows = statement.query_map([account.to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })?;
                let mut memberships = Vec::new();
                for row in rows {
                    let (organization, role, bound) = row?;
                    memberships.push(Identity {
                        account,
                        organization: parse_uuid(&organization)?,
                        role: decode_role(&role)?,
                        bound: bound.as_deref().map(parse_uuid).transpose()?,
                    });
                }
                Ok((public, memberships))
            })
            .await?
        };
        let mut sealed = Vec::new();
        for identity in memberships {
            let found = self
                .with_graph(identity.organization, move |_, graph, database| {
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
        Ok((public, sealed))
    }

    /// Every access log `account` signed a record in, at its head (`recovery-key`'s `logs`).
    pub async fn signed_logs(
        &self,
        account: Uuid,
    ) -> Result<Vec<be_block_protocol::SignedLog>, ServerError> {
        self.with_database(move |_, database| crate::access::signed_logs(&database, account))
            .await
    }

    pub async fn set_recovery_key(
        &self,
        account: Uuid,
        public: [u8; 32],
        signing: [u8; 32],
        sealed: Vec<SealedKey>,
        changes: Vec<KeyChange>,
        retirements: Vec<be_block_protocol::Retirement>,
    ) -> Result<(), ServerError> {
        let (_, held) = self.recovery_key(account).await?;
        let by = encode_wrapped_by(WrappedBy::Account(account));
        self.with_database(move |store, database| {
            let resealed: BTreeMap<(Uuid, Uuid), &SealedKey> = sealed
                .iter()
                .map(|key| ((key.organization, key.key), key))
                .collect();
            // What the account sealed for itself. What its contacts sealed to it the device
            // reseals as far as its records of them in the account's block reach, which the
            // server cannot read.
            let needed: Vec<&SealedKey> =
                held.iter().filter(|held| held.sender == account).collect();
            if let Some(missing) = needed
                .iter()
                .find(|held| !resealed.contains_key(&(held.organization, held.key)))
            {
                return Err(refused(format!(
                    "a new recovery key must seal every key the old one did, and {} is missing",
                    missing.key
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
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            for change in changes {
                crate::store::mailbox::leave(
                    &transaction,
                    change.contact,
                    Some(account),
                    &be_block_protocol::Letter::KeyChange(change.sealed),
                )?;
            }
            crate::access::append_retirements(
                &transaction,
                account,
                &be_block_keys::PublicIdentity {
                    public: be_block_keys::RecoveryPublic(public),
                    signing: be_block_keys::SigningPublic(signing),
                },
                &retirements,
            )?;
            if let Some(old) = recovery_key_of(&transaction, account)? {
                store.server_key().reseal_from(&transaction, account, old)?;
            }
            transaction.execute(
                "INSERT OR REPLACE INTO recovery_keys (account_id, public, signing)
                 VALUES (?1, ?2, ?3)",
                params![account.to_string(), public.to_vec(), signing.to_vec()],
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
        })
        .await
    }

    pub async fn reseal_keys(
        &self,
        account: Uuid,
        keys: Vec<SealedKey>,
    ) -> Result<(), ServerError> {
        let by = encode_wrapped_by(WrappedBy::Account(account));
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
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
        })
        .await
    }
}

/// The organization's members its log does not name, whose escrow
/// wraps administrators do not open until one is signed in: every such member, to an
/// administrator; the caller alone, to a member, so its device shows that it waits. None in an
/// organization with no log.
fn unsigned_members(
    connection: &Connection,
    organization: Uuid,
    identity: Identity,
) -> Result<Vec<Uuid>, ServerError> {
    let Some(checked) = crate::access::checked_organization_log(connection, organization)? else {
        return Ok(Vec::new());
    };
    let mut statement = connection.prepare_cached(
        "SELECT account_id FROM memberships WHERE organization_id = ?1 ORDER BY account_id",
    )?;
    let members: Vec<String> = statement
        .query_map([organization.to_string()], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let server = crate::organization::server_account(connection)?;
    let mut unsigned = Vec::new();
    for member in members {
        let member = parse_uuid(&member)?;
        let shown = member != server && (identity.is_administrator() || member == identity.account);
        if shown && checked.member(member.into_bytes()).is_none() {
            unsigned.push(member);
        }
    }
    Ok(unsigned)
}

/// Revocations the server made on its own of grants blocks' logs name, in blocks `seen` may
/// manage, for a device to sign (`blocks::write_grant`).
fn unsigned_revocations(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    seen: &crate::blocks::Visibility,
) -> Result<Vec<be_block_protocol::Revocation>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT block_id, account_id FROM unsigned_revocations WHERE organization_id = ?1
         ORDER BY block_id, account_id",
    )?;
    let rows: Vec<(String, String)> = statement
        .query_map([organization.to_string()], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut found = Vec::new();
    for (block, account) in rows {
        let block = parse_uuid(&block)?;
        if graph.contains(block) && seen.can_manage(graph, block) {
            found.push(be_block_protocol::Revocation {
                block,
                account: parse_uuid(&account)?,
            });
        }
    }
    Ok(found)
}
