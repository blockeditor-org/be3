use std::collections::{BTreeMap, BTreeSet};

use be_graph::BlockGraph;
use be_protocol::{ErrorCode, KeyKind, KeyRow, KeySet, SealedKey, WrappedBy, WrappedKey};
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

fn key_row(
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

fn keys_of_block(
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

fn wraps_of(
    connection: &Connection,
    organization: Uuid,
    key: Uuid,
) -> Result<Vec<WrappedKey>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT wrapped_by, sealed FROM wrapped_keys WHERE organization_id = ?1 AND key_id = ?2",
    )?;
    let rows = statement.query_map(params![organization.to_string(), key.to_string()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    let mut wraps = Vec::new();
    for row in rows {
        let (wrapped_by, sealed) = row?;
        wraps.push(WrappedKey {
            key,
            wrapped_by: decode_wrapped_by(&wrapped_by)?,
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

pub(crate) fn store_keys(
    graph: &mut BlockGraph,
    connection: &Connection,
    organization: Uuid,
    set: &KeySet,
    allowed: impl Fn(&BlockGraph, Uuid) -> bool,
) -> Result<(), ServerError> {
    let mut fresh: BTreeMap<Uuid, KeyRow> = BTreeMap::new();
    for row in &set.keys {
        if !graph.contains(row.block) || !allowed(graph, row.block) {
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
        if !fresh.contains_key(&row.id) && !allowed(graph, row.block) {
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
            WrappedBy::Account(account) => {
                if !is_member_of(connection, account, organization)? {
                    return Err(refused(format!(
                        "account {account} is not a member of this organization"
                    )));
                }
            }
        }
    }
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
            "INSERT OR IGNORE INTO wrapped_keys (organization_id, key_id, wrapped_by, sealed)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                organization.to_string(),
                wrap.key.to_string(),
                encode_wrapped_by(wrap.wrapped_by),
                wrap.sealed
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
            let wanted: BTreeSet<Uuid> = match blocks {
                None if seen.administrator() => graph.ids().into_iter().collect(),
                None => graph.visible(identity.account).into_keys().collect(),
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
                &set,
                |graph, block| seen.can_edit(graph, block),
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    pub async fn recovery_key(
        &self,
        account: Uuid,
    ) -> Result<(Option<[u8; 32]>, Vec<SealedKey>), ServerError> {
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
                        "SELECT wrapped_keys.key_id, keys.block_id, wrapped_keys.sealed
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
                                row.get::<_, Vec<u8>>(2)?,
                            ))
                        },
                    )?;
                    let mut found = Vec::new();
                    for row in rows {
                        let (key, block, bytes) = row?;
                        if seen.can_view(graph, parse_uuid(&block)?) {
                            found.push(SealedKey {
                                organization: identity.organization,
                                key: parse_uuid(&key)?,
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

    pub async fn set_recovery_key(
        &self,
        account: Uuid,
        public: [u8; 32],
        sealed: Vec<SealedKey>,
    ) -> Result<(), ServerError> {
        let (_, held) = self.recovery_key(account).await?;
        let resealed: BTreeMap<(Uuid, Uuid), &SealedKey> = sealed
            .iter()
            .map(|key| ((key.organization, key.key), key))
            .collect();
        if let Some(missing) = held
            .iter()
            .find(|held| !resealed.contains_key(&(held.organization, held.key)))
        {
            return Err(refused(format!(
                "a new recovery key must seal every key the old one did, and {} is missing",
                missing.key
            )));
        }
        let by = encode_wrapped_by(WrappedBy::Account(account));
        let database = self.database().lock().await;
        let transaction = database.unchecked_transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO recovery_keys (account_id, public) VALUES (?1, ?2)",
            params![account.to_string(), public.to_vec()],
        )?;
        transaction.execute("DELETE FROM wrapped_keys WHERE wrapped_by = ?1", [&by])?;
        for held in &held {
            let key = resealed[&(held.organization, held.key)];
            transaction.execute(
                "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sealed)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    key.organization.to_string(),
                    key.key.to_string(),
                    by,
                    key.sealed
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}
