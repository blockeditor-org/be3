//! Storage accounting (decisions/storage_is_charged_to_the_author.md): each object's size, each
//! block's own and subtree size, and each account's usage against its quota. Every counter is
//! kept incrementally where objects are held, released, uploaded and collected, so nothing here
//! walks an organization; the one walk is the backfill the first time a server opens its data.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use be_block_graph::{BlockGraph, ObjectRefs};
use be_block_protocol::{AccountStorage, ErrorCode};
use be_block_store::{Hash, ObjectStore};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{Identity, MAX_OBJECT_BYTES, ServerStore, parse_uuid};
use crate::ServerError;

/// The bytes an account may keep when the server is not told otherwise
/// (`ServerConfig::account_quota`, `--account-quota`).
pub const DEFAULT_ACCOUNT_QUOTA: u64 = 10 * 1024 * 1024 * 1024;
/// How long an uploaded object no block holds is kept, and charged to its uploader, before it is
/// collected (`ServerConfig::upload_grace`). A client uploads before it publishes, so this is
/// longest a publish may lag its upload.
pub const DEFAULT_UPLOAD_GRACE: Duration = Duration::from_secs(24 * 60 * 60);
/// How often a running server collects uploads past their grace.
pub(crate) const UPLOAD_SWEEP_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// Uploads collected in one transaction, so a large sweep never holds the database for long.
const UPLOADS_SWEPT_AT_ONCE: usize = 1024;
const SIZES_BACKFILL: &str = "sizes";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageLimits {
    /// None for no limit, as on a device's own embedded server.
    pub account_quota: Option<u64>,
    pub upload_grace: Duration,
}

impl StorageLimits {
    pub const DEFAULT: Self = Self {
        account_quota: Some(DEFAULT_ACCOUNT_QUOTA),
        upload_grace: DEFAULT_UPLOAD_GRACE,
    };
}

impl Default for StorageLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// What a block held of some objects before it took them: taken by `before_holding` before the
/// rows are written, and charged by `charge_holding` after.
pub(crate) struct Holding {
    organization: Uuid,
    payer: Uuid,
    new_to_block: Vec<Hash>,
    new_to_organization: Vec<Hash>,
    unreferenced: Vec<Hash>,
}

impl ServerStore {
    pub(crate) fn before_holding(
        &self,
        connection: &Connection,
        graph: &BlockGraph,
        refs: &ObjectRefs,
        organization: Uuid,
        block: Uuid,
        hashes: &[Hash],
    ) -> Result<Holding, ServerError> {
        let author = graph.get(block).ok_or(ServerError::Corrupt)?.author;
        let mut holding = Holding {
            organization,
            payer: block_payer(connection, organization, block, author)?,
            new_to_block: Vec::new(),
            new_to_organization: Vec::new(),
            unreferenced: Vec::new(),
        };
        for hash in distinct(hashes) {
            if !held_by_block(connection, organization, block, hash)? {
                holding.new_to_block.push(hash);
            }
            if !held_in_organization(connection, organization, hash)? {
                holding.new_to_organization.push(hash);
            }
            if refs.count(hash) == 0 {
                holding.unreferenced.push(hash);
            }
        }
        Ok(holding)
    }

    /// Charges what `holding` added, once its rows are written and before the transaction
    /// commits, and refuses when that takes the payer past its quota or the organization past
    /// its policy's. A refusal leaves the caller to drop the transaction. Answers the bytes the
    /// block grew by, for `BlockGraph::add_size`.
    pub(crate) fn charge_holding(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        holding: &Holding,
    ) -> Result<u64, ServerError> {
        let before = usage_of(connection, holding.payer)?;
        let organization_before = organization_bytes(connection, holding.organization)?;
        let mut grew = 0;
        for hash in &holding.new_to_block {
            let bytes = self.object_bytes(connection, *hash)?;
            grew += bytes;
            if count_account_object(connection, holding.payer, *hash, true)? {
                shift_usage(connection, holding.payer, Column::Held, bytes, true)?;
                self.usage_changed(holding.payer);
            }
        }
        for hash in &holding.new_to_organization {
            let bytes = self.object_bytes(connection, *hash)?;
            shift_organization(connection, holding.organization, bytes, true)?;
        }
        for hash in &holding.unreferenced {
            for payer in stop_pending(connection, *hash, self.object_bytes(connection, *hash)?)? {
                self.usage_changed(payer);
            }
        }
        let after = usage_of(connection, holding.payer)?;
        if let Some(quota) = self.limits.account_quota
            && after.total() > before.total()
            && after.total() > quota
        {
            return Err(over_account_quota(quota, after.total()));
        }
        if let Some(quota) = self
            .policies(graph, connection, holding.organization)?
            .quota()
        {
            let organization_after = organization_bytes(connection, holding.organization)?;
            if organization_after > organization_before && organization_after > quota {
                return Err(over_organization_quota(quota, organization_after));
            }
        }
        Ok(grew)
    }

    /// Uncharges what a block stopped holding: `hashes` are the rows just deleted for it. Call it
    /// before `forget_objects`, which drops the sizes it reads. Answers the bytes the block
    /// shrank by.
    pub(crate) fn release_charged(
        &self,
        connection: &Connection,
        graph: &BlockGraph,
        organization: Uuid,
        block: Uuid,
        hashes: &[Hash],
    ) -> Result<u64, ServerError> {
        let author = graph.get(block).ok_or(ServerError::Corrupt)?.author;
        let payer = block_payer(connection, organization, block, author)?;
        let mut shrank = 0;
        for hash in distinct(hashes) {
            if !held_by_block(connection, organization, block, hash)? {
                let bytes = self.object_bytes(connection, hash)?;
                shrank += bytes;
                if count_account_object(connection, payer, hash, false)? {
                    shift_usage(connection, payer, Column::Held, bytes, false)?;
                    self.usage_changed(payer);
                }
            }
            if !held_in_organization(connection, organization, hash)? {
                let bytes = self.object_bytes(connection, hash)?;
                shift_organization(connection, organization, bytes, false)?;
            }
        }
        Ok(shrank)
    }

    /// The size of an object the server holds. Every object stored since sizes were kept has
    /// one; the file's length stands in for any other.
    fn object_bytes(&self, connection: &Connection, hash: Hash) -> Result<u64, ServerError> {
        let stored: Option<i64> = connection
            .prepare_cached("SELECT bytes FROM object_sizes WHERE hash = ?1")?
            .query_row([hash.to_hex()], |row| row.get(0))
            .optional()?;
        if let Some(bytes) = stored {
            return Ok(u64::try_from(bytes).unwrap_or(0));
        }
        let bytes = self.objects().len(hash)?.unwrap_or(0) as u64;
        connection.execute(
            "INSERT OR IGNORE INTO object_sizes (hash, bytes) VALUES (?1, ?2)",
            params![hash.to_hex(), to_sql(bytes)],
        )?;
        Ok(bytes)
    }

    /// Stores uploaded objects, charging the ones no block holds to the uploader until a block
    /// does. Refuses before writing anything when that takes the uploader past its quota or the
    /// organization past its policy's.
    pub async fn put_objects(
        &self,
        identity: Identity,
        objects: &[impl AsRef<[u8]>],
    ) -> Result<Vec<Hash>, ServerError> {
        if objects
            .iter()
            .any(|bytes| bytes.as_ref().len() > MAX_OBJECT_BYTES)
        {
            return Err(ServerError::Refused(
                ErrorCode::ObjectTooLarge,
                format!("an object may not exceed {MAX_OBJECT_BYTES} bytes"),
            ));
        }
        let sized: Vec<(Hash, u64)> = objects
            .iter()
            .map(|bytes| (Hash::of(bytes.as_ref()), bytes.as_ref().len() as u64))
            .collect();
        if !sized.is_empty() {
            let sized = sized.clone();
            self.with_graph(identity.organization, move |graph, database| {
                self.reserve_uploads(graph, database, identity, &sized)
            })
            .await?;
        }
        let hashes = objects
            .iter()
            .map(|bytes| self.objects().put(bytes.as_ref()))
            .collect::<Result<Vec<Hash>, _>>()?;
        self.objects().sync()?;
        Ok(hashes)
    }

    fn reserve_uploads(
        &self,
        graph: &BlockGraph,
        database: &Connection,
        identity: Identity,
        sized: &[(Hash, u64)],
    ) -> Result<(), ServerError> {
        let transaction = database.unchecked_transaction()?;
        let payer = payer_of(&transaction, identity.account)?;
        let account = identity.account.to_string();
        let now = be_block_commit::now_milliseconds();
        let refs: BTreeMap<Hash, u32> = {
            let refs = self.object_refs();
            sized
                .iter()
                .map(|(hash, _)| (*hash, refs.count(*hash)))
                .collect()
        };
        let mut pending = 0;
        let mut new_to_organization = 0;
        let mut seen = BTreeSet::new();
        for (hash, bytes) in sized {
            if !seen.insert(*hash) {
                continue;
            }
            transaction.execute(
                "INSERT OR IGNORE INTO object_sizes (hash, bytes) VALUES (?1, ?2)",
                params![hash.to_hex(), to_sql(*bytes)],
            )?;
            let unreferenced = refs.get(hash).copied().unwrap_or(0) == 0;
            let already: bool = transaction
                .prepare_cached(
                    "SELECT EXISTS(SELECT 1 FROM object_uploads INDEXED BY object_uploads_object
                                   WHERE hash = ?1 AND account_id = ?2 AND pending = 1)",
                )?
                .query_row(params![hash.to_hex(), account], |row| row.get(0))?;
            if unreferenced && !already {
                pending += bytes;
            }
            if !held_in_organization(&transaction, identity.organization, *hash)? {
                new_to_organization += bytes;
            }
            transaction
                .prepare_cached(
                    "INSERT INTO object_uploads
                        (organization_id, account_id, hash, size, uploaded, payer, pending)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT (organization_id, account_id, hash) DO UPDATE SET
                        uploaded = CASE WHEN excluded.pending = 1 THEN excluded.uploaded
                                        ELSE uploaded END,
                        payer = excluded.payer,
                        pending = excluded.pending",
                )?
                .execute(params![
                    identity.organization.to_string(),
                    account,
                    hash.to_hex(),
                    to_sql(*bytes),
                    now,
                    payer.to_string(),
                    unreferenced
                ])?;
        }
        if pending > 0
            && let Some(quota) = self.limits.account_quota
        {
            let total = usage_of(&transaction, payer)?.total() + pending;
            if total > quota {
                return Err(over_account_quota(quota, total));
            }
        }
        if new_to_organization > 0
            && let Some(quota) = self
                .policies(graph, &transaction, identity.organization)?
                .quota()
        {
            let total =
                organization_bytes(&transaction, identity.organization)? + new_to_organization;
            if total > quota {
                return Err(over_organization_quota(quota, total));
            }
        }
        shift_usage(&transaction, payer, Column::Pending, pending, true)?;
        transaction.commit()?;
        if pending > 0 {
            self.usage_changed(payer);
        }
        Ok(())
    }

    /// Deletes uploads no block took within `StorageLimits::upload_grace`, uncharging their
    /// uploaders. A running server calls it every `UPLOAD_SWEEP_INTERVAL` and when it starts.
    /// Answers how many objects it deleted.
    pub async fn collect_uploads(&self) -> Result<usize, ServerError> {
        let grace = i64::try_from(self.limits.upload_grace.as_millis()).unwrap_or(i64::MAX);
        let cutoff = be_block_commit::now_milliseconds().saturating_sub(grace);
        let mut deleted = 0;
        loop {
            let database = self.database().lock().await;
            let transaction = database.unchecked_transaction()?;
            let expired: Vec<(String, String, String, String)> = {
                let mut statement = transaction.prepare_cached(
                    "SELECT organization_id, hash, account_id, payer FROM object_uploads
                     WHERE pending = 1 AND uploaded <= ?1 LIMIT ?2",
                )?;
                let rows = statement.query_map(
                    params![cutoff, to_sql(UPLOADS_SWEPT_AT_ONCE as u64)],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;
                rows.collect::<Result<_, _>>()?
            };
            let mut freed = Vec::new();
            for (organization, hex, account, payer) in &expired {
                let hash = Hash::from_hex(hex).ok_or(ServerError::Corrupt)?;
                let bytes = self.object_bytes(&transaction, hash)?;
                transaction.execute(
                    "DELETE FROM object_uploads
                     WHERE organization_id = ?1 AND hash = ?2 AND account_id = ?3",
                    params![organization, hex, account],
                )?;
                let charged_elsewhere: bool = transaction
                    .prepare_cached(
                        "SELECT EXISTS(SELECT 1 FROM object_uploads INDEXED BY object_uploads_object
                                       WHERE hash = ?1 AND account_id = ?2 AND pending = 1)",
                    )?
                    .query_row(params![hex, account], |row| row.get(0))?;
                if !charged_elsewhere && let Ok(payer) = Uuid::parse_str(payer) {
                    shift_usage(&transaction, payer, Column::Pending, bytes, false)?;
                    self.usage_changed(payer);
                }
                let still_pending: bool = transaction
                    .prepare_cached(
                        "SELECT EXISTS(SELECT 1 FROM object_uploads INDEXED BY object_uploads_object WHERE hash = ?1 AND pending = 1)",
                    )?
                    .query_row([hex], |row| row.get(0))?;
                if !still_pending && self.object_refs().count(hash) == 0 {
                    freed.push(hash);
                }
            }
            forget_objects(&transaction, &freed)?;
            transaction.commit()?;
            for hash in &freed {
                self.objects().remove(*hash)?;
            }
            deleted += freed.len();
            if expired.len() < UPLOADS_SWEPT_AT_ONCE {
                return Ok(deleted);
            }
        }
    }

    /// Tells the connections following `payer`'s storage that it changed. The change may not be
    /// committed yet; they read it under the database lock this caller holds, so after it.
    fn usage_changed(&self, payer: Uuid) {
        let _ = self.usage_changes.send(payer);
    }

    /// The account whose quota `account`'s uploads count against: its own, or an agent's
    /// creator's.
    pub async fn payer(&self, account: Uuid) -> Result<Uuid, ServerError> {
        payer_of(&*self.database().lock().await, account)
    }

    pub async fn storage_usage(&self, identity: Identity) -> Result<AccountStorage, ServerError> {
        let database = self.database().lock().await;
        let usage = usage_of(&database, payer_of(&database, identity.account)?)?;
        Ok(AccountStorage {
            held: usage.held,
            pending: usage.pending,
            quota: self.limits.account_quota,
        })
    }
}

/// Persists the sizes `graph` holds for `blocks`, and forgets those it no longer has.
pub(crate) fn persist_sizes(
    connection: &Connection,
    graph: &BlockGraph,
    organization: Uuid,
    blocks: impl IntoIterator<Item = Uuid>,
) -> Result<(), ServerError> {
    let organization_id = organization.to_string();
    for block in distinct_ids(blocks) {
        match graph.get(block) {
            Some(node) => {
                let payer = payer_of(connection, node.author)?;
                connection
                    .prepare_cached(
                        "INSERT INTO block_sizes (organization_id, block_id, payer, size, tree_size)
                         VALUES (?1, ?2, ?3, ?4, ?5)
                         ON CONFLICT (organization_id, block_id)
                         DO UPDATE SET size = excluded.size, tree_size = excluded.tree_size",
                    )?
                    .execute(params![
                        organization_id,
                        block.to_string(),
                        payer.to_string(),
                        to_sql(node.size),
                        to_sql(node.tree_size)
                    ])?;
            }
            None => {
                connection.execute(
                    "DELETE FROM block_sizes WHERE organization_id = ?1 AND block_id = ?2",
                    params![organization_id, block.to_string()],
                )?;
            }
        }
    }
    Ok(())
}

/// Reads the sizes stored for an organization's blocks into its freshly loaded graph.
pub(crate) fn load_sizes(
    connection: &Connection,
    graph: &mut BlockGraph,
    organization: Uuid,
) -> Result<(), ServerError> {
    let mut statement = connection
        .prepare("SELECT block_id, size, tree_size FROM block_sizes WHERE organization_id = ?1")?;
    let rows = statement.query_map([organization.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (block, size, tree_size) = row?;
        let block = parse_uuid(&block)?;
        if graph.contains(block) {
            graph.set_sizes(
                block,
                u64::try_from(size).unwrap_or(0),
                u64::try_from(tree_size).unwrap_or(0),
            )?;
        }
    }
    Ok(())
}

/// Drops what the server kept about objects it deleted.
pub(crate) fn forget_objects(connection: &Connection, freed: &[Hash]) -> Result<(), ServerError> {
    for hash in freed {
        connection.execute("DELETE FROM object_sizes WHERE hash = ?1", [hash.to_hex()])?;
        connection.execute(
            "DELETE FROM object_uploads WHERE hash = ?1",
            [hash.to_hex()],
        )?;
    }
    Ok(())
}

/// The account that pays for what `account` stores: an agent's creator, or the account itself.
pub(crate) fn payer_of(connection: &Connection, account: Uuid) -> Result<Uuid, ServerError> {
    let creator: Option<String> = connection
        .prepare_cached("SELECT created_by FROM agents WHERE account_id = ?1")?
        .query_row([account.to_string()], |row| row.get(0))
        .optional()?;
    match creator {
        Some(creator) => parse_uuid(&creator),
        None => Ok(account),
    }
}

/// The account a block's objects are charged to, fixed when the block first held anything, so
/// a later change to who pays for its author never strands a charge.
fn block_payer(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    author: Uuid,
) -> Result<Uuid, ServerError> {
    let stored: Option<String> = connection
        .prepare_cached(
            "SELECT payer FROM block_sizes WHERE organization_id = ?1 AND block_id = ?2",
        )?
        .query_row(
            params![organization.to_string(), block.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    match stored {
        Some(payer) => parse_uuid(&payer),
        None => payer_of(connection, author),
    }
}

fn held_by_block(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
    hash: Hash,
) -> Result<bool, ServerError> {
    Ok(connection
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM commit_objects
                           WHERE organization_id = ?1 AND hash = ?3 AND block_id = ?2)
                 OR EXISTS(SELECT 1 FROM block_objects
                           WHERE organization_id = ?1 AND block_id = ?2 AND hash = ?3)",
        )?
        .query_row(
            params![organization.to_string(), block.to_string(), hash.to_hex()],
            |row| row.get(0),
        )?)
}

fn held_in_organization(
    connection: &Connection,
    organization: Uuid,
    hash: Hash,
) -> Result<bool, ServerError> {
    Ok(connection
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM commit_objects WHERE organization_id = ?1 AND hash = ?2)
                 OR EXISTS(SELECT 1 FROM block_objects WHERE organization_id = ?1 AND hash = ?2)",
        )?
        .query_row(params![organization.to_string(), hash.to_hex()], |row| {
            row.get(0)
        })?)
}

/// Counts one more (or one fewer) of the payer's blocks holding `hash`. Answers whether that
/// made it the first (or ended the last), which is when the payer's usage changes.
fn count_account_object(
    connection: &Connection,
    payer: Uuid,
    hash: Hash,
    adding: bool,
) -> Result<bool, ServerError> {
    let (payer, hash) = (payer.to_string(), hash.to_hex());
    let count: Option<i64> = connection
        .prepare_cached("SELECT count FROM account_objects WHERE account_id = ?1 AND hash = ?2")?
        .query_row(params![payer, hash], |row| row.get(0))
        .optional()?;
    match (adding, count) {
        (true, None) => {
            connection.execute(
                "INSERT INTO account_objects (account_id, hash, count) VALUES (?1, ?2, 1)",
                params![payer, hash],
            )?;
            Ok(true)
        }
        (false, Some(1)) => {
            connection.execute(
                "DELETE FROM account_objects WHERE account_id = ?1 AND hash = ?2",
                params![payer, hash],
            )?;
            Ok(true)
        }
        (_, Some(count)) => {
            connection.execute(
                "UPDATE account_objects SET count = ?3 WHERE account_id = ?1 AND hash = ?2",
                params![payer, hash, if adding { count + 1 } else { count - 1 }],
            )?;
            Ok(false)
        }
        (false, None) => Ok(false),
    }
}

/// A block took an object nothing held: it stops being charged to whoever uploaded it.
/// Answers those it was charged to.
fn stop_pending(connection: &Connection, hash: Hash, bytes: u64) -> Result<Vec<Uuid>, ServerError> {
    let payers: Vec<String> = {
        let mut statement = connection.prepare_cached(
            "SELECT MIN(payer) FROM object_uploads INDEXED BY object_uploads_object WHERE hash = ?1 AND pending = 1
                 GROUP BY account_id",
        )?;
        let rows = statement.query_map([hash.to_hex()], |row| row.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut uncharged = Vec::new();
    for payer in payers {
        if let Ok(payer) = Uuid::parse_str(&payer) {
            shift_usage(connection, payer, Column::Pending, bytes, false)?;
            uncharged.push(payer);
        }
    }
    connection.execute(
        "UPDATE object_uploads SET pending = 0 WHERE hash = ?1",
        [hash.to_hex()],
    )?;
    Ok(uncharged)
}

#[derive(Clone, Copy, Debug, Default)]
struct Usage {
    held: u64,
    pending: u64,
}

impl Usage {
    fn total(self) -> u64 {
        self.held.saturating_add(self.pending)
    }
}

fn usage_of(connection: &Connection, account: Uuid) -> Result<Usage, ServerError> {
    let usage: Option<(i64, i64)> = connection
        .prepare_cached("SELECT held, pending FROM account_usage WHERE account_id = ?1")?
        .query_row([account.to_string()], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    Ok(usage
        .map(|(held, pending)| Usage {
            held: u64::try_from(held).unwrap_or(0),
            pending: u64::try_from(pending).unwrap_or(0),
        })
        .unwrap_or_default())
}

#[derive(Clone, Copy)]
enum Column {
    Held,
    Pending,
}

fn shift_usage(
    connection: &Connection,
    account: Uuid,
    column: Column,
    bytes: u64,
    adding: bool,
) -> Result<(), ServerError> {
    if bytes == 0 {
        return Ok(());
    }
    let delta = signed(bytes, adding);
    let sql = match column {
        Column::Held => {
            "INSERT INTO account_usage (account_id, held, pending) VALUES (?1, MAX(?2, 0), 0)
             ON CONFLICT (account_id) DO UPDATE SET held = MAX(held + ?2, 0)"
        }
        Column::Pending => {
            "INSERT INTO account_usage (account_id, held, pending) VALUES (?1, 0, MAX(?2, 0))
             ON CONFLICT (account_id) DO UPDATE SET pending = MAX(pending + ?2, 0)"
        }
    };
    connection
        .prepare_cached(sql)?
        .execute(params![account.to_string(), delta])?;
    Ok(())
}

fn organization_bytes(connection: &Connection, organization: Uuid) -> Result<u64, ServerError> {
    let bytes: Option<i64> = connection
        .prepare_cached("SELECT bytes FROM organization_usage WHERE organization_id = ?1")?
        .query_row([organization.to_string()], |row| row.get(0))
        .optional()?;
    Ok(bytes.map_or(0, |bytes| u64::try_from(bytes).unwrap_or(0)))
}

fn shift_organization(
    connection: &Connection,
    organization: Uuid,
    bytes: u64,
    adding: bool,
) -> Result<(), ServerError> {
    connection
        .prepare_cached(
            "INSERT INTO organization_usage (organization_id, bytes) VALUES (?1, MAX(?2, 0))
             ON CONFLICT (organization_id) DO UPDATE SET bytes = MAX(bytes + ?2, 0)",
        )?
        .execute(params![organization.to_string(), signed(bytes, adding)])?;
    Ok(())
}

fn over_account_quota(quota: u64, total: u64) -> ServerError {
    ServerError::Refused(
        ErrorCode::OverQuota,
        format!("this account may store {quota} bytes, and this would make {total}"),
    )
}

fn over_organization_quota(quota: u64, total: u64) -> ServerError {
    ServerError::Refused(
        ErrorCode::OverQuota,
        format!("this organization stores at most {quota} bytes, and this would make {total}"),
    )
}

fn signed(bytes: u64, adding: bool) -> i64 {
    let bytes = to_sql(bytes);
    match adding {
        true => bytes,
        false => -bytes,
    }
}

fn to_sql(bytes: u64) -> i64 {
    i64::try_from(bytes).unwrap_or(i64::MAX)
}

fn distinct(hashes: &[Hash]) -> Vec<Hash> {
    let mut seen = BTreeSet::new();
    hashes
        .iter()
        .copied()
        .filter(|hash| seen.insert(*hash))
        .collect()
}

fn distinct_ids(ids: impl IntoIterator<Item = Uuid>) -> Vec<Uuid> {
    let mut seen = BTreeSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

/// Fills the counters from what the data already holds, the first time a server that keeps them
/// opens it. Objects on disk that nothing holds count as uploads made now, by nobody, so they are
/// collected once their grace has passed.
pub(crate) fn backfill(
    connection: &Connection,
    objects: &be_block_store::FileStore,
) -> Result<(), ServerError> {
    let done: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM server_backfills WHERE name = ?1)",
        [SIZES_BACKFILL],
        |row| row.get(0),
    )?;
    if done {
        return Ok(());
    }
    let transaction = connection.unchecked_transaction()?;
    let referenced: BTreeSet<Hash> = {
        let mut statement = transaction.prepare("SELECT hash FROM object_refs WHERE count > 0")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut referenced = BTreeSet::new();
        for row in rows {
            referenced.insert(Hash::from_hex(&row?).ok_or(ServerError::Corrupt)?);
        }
        referenced
    };
    let mut sizes = BTreeMap::new();
    for hash in &referenced {
        let bytes = objects.len(*hash)?.unwrap_or(0) as u64;
        sizes.insert(*hash, bytes);
    }
    let now = be_block_commit::now_milliseconds();
    let nobody = Uuid::nil().to_string();
    for hash in objects.hashes()? {
        if referenced.contains(&hash) {
            continue;
        }
        let bytes = objects.len(hash)?.unwrap_or(0) as u64;
        sizes.insert(hash, bytes);
        transaction.execute(
            "INSERT OR IGNORE INTO object_uploads
                (organization_id, account_id, hash, size, uploaded, payer, pending)
             VALUES (?2, ?2, ?1, ?4, ?3, ?2, 1)",
            params![hash.to_hex(), nobody, now, to_sql(bytes)],
        )?;
    }
    for (hash, bytes) in &sizes {
        transaction.execute(
            "INSERT OR IGNORE INTO object_sizes (hash, bytes) VALUES (?1, ?2)",
            params![hash.to_hex(), to_sql(*bytes)],
        )?;
    }
    let organizations: Vec<String> = {
        let mut statement = transaction.prepare("SELECT id FROM organizations")?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut account_objects: BTreeMap<(Uuid, Hash), i64> = BTreeMap::new();
    for organization in organizations {
        let organization = parse_uuid(&organization)?;
        let mut graph = super::load_graph(&transaction, organization)?;
        let held: Vec<(Uuid, Hash)> = {
            let mut statement = transaction.prepare(
                "SELECT block_id, hash FROM commit_objects WHERE organization_id = ?1
                 UNION SELECT block_id, hash FROM block_objects WHERE organization_id = ?1",
            )?;
            let rows = statement.query_map([organization.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut held = Vec::new();
            for row in rows {
                let (block, hash) = row?;
                held.push((
                    parse_uuid(&block)?,
                    Hash::from_hex(&hash).ok_or(ServerError::Corrupt)?,
                ));
            }
            held
        };
        let mut payers = BTreeMap::new();
        let mut in_organization = BTreeSet::new();
        for (block, hash) in held {
            let bytes = sizes.get(&hash).copied().unwrap_or(0);
            if graph.contains(block) {
                graph.add_size(block, bytes, true)?;
            }
            let author = graph.get(block).map_or(Uuid::nil(), |node| node.author);
            let payer = match payers.get(&author) {
                Some(payer) => *payer,
                None => {
                    let payer = payer_of(&transaction, author)?;
                    payers.insert(author, payer);
                    payer
                }
            };
            *account_objects.entry((payer, hash)).or_default() += 1;
            in_organization.insert(hash);
        }
        let organization_total: u64 = in_organization
            .iter()
            .map(|hash| sizes.get(hash).copied().unwrap_or(0))
            .sum();
        shift_organization(&transaction, organization, organization_total, true)?;
        persist_sizes(&transaction, &graph, organization, graph.ids())?;
    }
    let mut held: BTreeMap<Uuid, u64> = BTreeMap::new();
    for ((payer, hash), count) in &account_objects {
        transaction.execute(
            "INSERT OR REPLACE INTO account_objects (account_id, hash, count) VALUES (?1, ?2, ?3)",
            params![payer.to_string(), hash.to_hex(), count],
        )?;
        *held.entry(*payer).or_default() += sizes.get(hash).copied().unwrap_or(0);
    }
    for (payer, bytes) in held {
        shift_usage(&transaction, payer, Column::Held, bytes, true)?;
    }
    transaction.execute(
        "INSERT INTO server_backfills (name) VALUES (?1)",
        [SIZES_BACKFILL],
    )?;
    transaction.commit()?;
    Ok(())
}
