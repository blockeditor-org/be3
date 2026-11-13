//! Storage accounting (decisions/storage_is_charged_to_the_trees_owner.md): each object's size, each
//! block's own and subtree size, and each account's usage against its quota. Every counter is
//! kept incrementally where objects are held, released, uploaded and collected, so nothing here
//! walks an organization.

use std::{collections::BTreeSet, time::Duration};

use be_block_commit::CommitId;
use be_block_graph::{BlockGraph, BlockParent};
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
/// The longest a presigned URL for an object's bytes stays valid (`transfer_lifetime`).
const TRANSFER_LIFETIME: Duration = Duration::from_secs(15 * 60);
/// Uploads collected in one transaction, so a large sweep never holds the database for long.
const UPLOADS_SWEPT_AT_ONCE: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageLimits {
    /// None for no limit, as on a device's own embedded server.
    pub account_quota: Option<u64>,
    pub upload_grace: Duration,
    /// The bytes one commit of a search index may hold (`ServerConfig::max_index_bytes`).
    pub max_index_bytes: u64,
}

impl StorageLimits {
    pub const DEFAULT: Self = Self {
        account_quota: Some(DEFAULT_ACCOUNT_QUOTA),
        upload_grace: DEFAULT_UPLOAD_GRACE,
        max_index_bytes: crate::limits::MAX_INDEX_BYTES,
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
    identity: Identity,
    payer: Uuid,
    hashes: Vec<Hash>,
    new_to_block: Vec<Hash>,
    new_to_organization: Vec<Hash>,
}

impl ServerStore {
    pub(crate) fn before_holding(
        &self,
        connection: &Connection,
        graph: &BlockGraph,
        identity: Identity,
        block: Uuid,
        hashes: &[Hash],
    ) -> Result<Holding, ServerError> {
        let organization = identity.organization;
        if !graph.contains(block) {
            return Err(ServerError::Corrupt);
        }
        let mut holding = Holding {
            identity,
            payer: block_payer(connection, organization, block)?,
            hashes: distinct(hashes),
            new_to_block: Vec::new(),
            new_to_organization: Vec::new(),
        };
        for hash in &holding.hashes {
            if !held_by_block(connection, organization, block, *hash)? {
                holding.new_to_block.push(*hash);
            }
            if !held_in_organization(connection, organization, *hash)? {
                holding.new_to_organization.push(*hash);
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
        let organization = holding.identity.organization;
        let organization_before = organization_bytes(connection, organization)?;
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
            shift_organization(connection, organization, bytes, true)?;
        }
        self.take_uploads(connection, holding.identity, &holding.hashes)?;
        let after = usage_of(connection, holding.payer)?;
        if let Some(quota) = self.limits.account_quota
            && after.total() > before.total()
            && after.total() > quota
        {
            return Err(over_account_quota(quota, after.total()));
        }
        if let Some(quota) = self.policies(graph, connection, organization)?.quota() {
            let organization_after = organization_bytes(connection, organization)?;
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
        if !graph.contains(block) {
            return Err(ServerError::Corrupt);
        }
        let payer = block_payer(connection, organization, block)?;
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

    /// The bytes of the stored objects among `hashes`, each counted once.
    pub(crate) fn stored_bytes(
        &self,
        connection: &Connection,
        hashes: &[Hash],
    ) -> Result<u64, ServerError> {
        let mut total = 0u64;
        for hash in distinct(hashes) {
            let stored: Option<i64> = connection
                .prepare_cached("SELECT bytes FROM object_sizes WHERE hash = ?1")?
                .query_row([hash.to_hex()], |row| row.get(0))
                .optional()?;
            total = total.saturating_add(
                stored
                    .and_then(|bytes| u64::try_from(bytes).ok())
                    .unwrap_or(0),
            );
        }
        Ok(total)
    }

    /// The bytes of the stored objects `commit` of `block` holds, each counted once, in one
    /// query.
    pub(crate) fn commit_bytes(
        &self,
        connection: &Connection,
        organization: Uuid,
        block: Uuid,
        commit: CommitId,
    ) -> Result<u64, ServerError> {
        let bytes: i64 = connection
            .prepare_cached(
                "SELECT COALESCE(SUM(sizes.bytes), 0)
                 FROM (SELECT DISTINCT hash FROM commit_objects
                       WHERE organization_id = ?1 AND block_id = ?2 AND commit_id = ?3) held
                 JOIN object_sizes sizes ON sizes.hash = held.hash",
            )?
            .query_row(
                params![
                    organization.to_string(),
                    block.to_string(),
                    commit.hash().to_hex()
                ],
                |row| row.get(0),
            )?;
        Ok(u64::try_from(bytes).unwrap_or(0))
    }

    /// The size of an object the server holds, from `object_sizes`, which has a row for every
    /// object from its upload until it is forgotten (`forget_objects`), so nothing asks the
    /// object store, which may be a bucket a round trip away.
    fn object_bytes(&self, connection: &Connection, hash: Hash) -> Result<u64, ServerError> {
        let stored: Option<i64> = connection
            .prepare_cached("SELECT bytes FROM object_sizes WHERE hash = ?1")?
            .query_row([hash.to_hex()], |row| row.get(0))
            .optional()?;
        stored
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(ServerError::Corrupt)
    }

    /// Reserves room for objects a client is about to upload (`upload-urls`), charging the ones
    /// no block holds to the uploader until a block does. Refuses before reserving anything when
    /// an object is too large, its length is not the one the server already has for it, or the
    /// upload takes the uploader past its quota or the organization past its policy's. A
    /// reserved object counts as stored only once `confirm_uploads` saw its bytes, and one the
    /// server already had is the uploader's to hold only if it could read it when it reserved
    /// it: sending bytes the store holds proves nothing the server can see.
    pub async fn reserve_uploads(
        &self,
        identity: Identity,
        objects: Vec<(Hash, u64)>,
    ) -> Result<(), ServerError> {
        self.reserve_carried(identity, objects, false).await
    }

    async fn reserve_carried(
        &self,
        identity: Identity,
        objects: Vec<(Hash, u64)>,
        carried: bool,
    ) -> Result<(), ServerError> {
        if objects
            .iter()
            .any(|(_, bytes)| *bytes > MAX_OBJECT_BYTES as u64)
        {
            return Err(ServerError::Refused(
                ErrorCode::ObjectTooLarge,
                format!("an object may not exceed {MAX_OBJECT_BYTES} bytes"),
            ));
        }
        if objects.is_empty() {
            return Ok(());
        }
        if !carried {
            // Whether an object is stored decides whether uploading it proves anything
            // (`reserve`), so someone else's upload that arrived but is not used yet counts.
            let hashes = objects.iter().map(|(hash, _)| *hash).collect();
            self.confirm_arrived(None, hashes).await?;
        }
        self.with_graph(identity.organization, move |store, graph, database| {
            store.reserve(graph, database, identity, &objects, carried)
        })
        .await
    }

    /// Stores objects a call carries itself (`publish`'s), as `reserve_uploads` and an upload
    /// would, and confirms them at once since the server wrote them.
    pub async fn put_objects(
        &self,
        identity: Identity,
        objects: Vec<Vec<u8>>,
    ) -> Result<Vec<Hash>, ServerError> {
        let sized: Vec<(Hash, u64)> = objects
            .iter()
            .map(|bytes| (Hash::of(bytes), bytes.len() as u64))
            .collect();
        self.reserve_carried(identity, sized.clone(), true).await?;
        // Writing and syncing 8 MiB objects would stall the runtime's threads, so the files
        // are written on the blocking pool, after the quota is reserved and outside the lock.
        // Should the write fail, the reservations stay unconfirmed until they are collected.
        let store = self.objects().clone();
        let hashes = crate::store::blocking(move || {
            let hashes = store.put_many(&objects)?;
            store.sync()?;
            Ok(hashes)
        })
        .await?;
        self.with_database(move |_, database| confirm(database, &sized))
            .await?;
        Ok(hashes)
    }

    /// Confirms the objects among `hashes` that the caller reserved and the server has not yet
    /// seen: asks the object store for each one's length (a HEAD on a bucket, a file's length
    /// on disk) and records those whose length is the reserved one as stored, so later uses ask
    /// nothing. Only the caller's own reservations are looked at, so a hash it knows tells it
    /// nothing and costs no request to the store.
    pub(crate) async fn confirm_uploads(
        &self,
        identity: Identity,
        hashes: Vec<Hash>,
    ) -> Result<(), ServerError> {
        self.confirm_arrived(Some(identity), hashes).await
    }

    /// Of `hashes`, confirms those reserved and not yet confirmed (by `uploader` only, when
    /// given) whose bytes the object store holds with the reserved length.
    async fn confirm_arrived(
        &self,
        uploader: Option<Identity>,
        hashes: Vec<Hash>,
    ) -> Result<(), ServerError> {
        let unconfirmed = self
            .with_reader(move |_, reader| {
                let mut unconfirmed = Vec::new();
                let mut statement = reader.prepare_cached(
                    "SELECT bytes FROM object_sizes WHERE hash = ?1 AND NOT confirmed
                     AND (?2 IS NULL OR EXISTS(SELECT 1 FROM object_uploads
                         WHERE organization_id = ?2 AND account_id = ?3 AND hash = ?1))",
                )?;
                for hash in distinct(&hashes) {
                    let bytes: Option<i64> = statement
                        .query_row(
                            params![
                                hash.to_hex(),
                                uploader.map(|uploader| uploader.organization.to_string()),
                                uploader.map(|uploader| uploader.account.to_string())
                            ],
                            |row| row.get(0),
                        )
                        .optional()?;
                    if let Some(bytes) = bytes.and_then(|bytes| u64::try_from(bytes).ok()) {
                        unconfirmed.push((hash, bytes));
                    }
                }
                Ok(unconfirmed)
            })
            .await?;
        if unconfirmed.is_empty() {
            return Ok(());
        }
        let store = self.objects().clone();
        let arrived = crate::store::blocking(move || {
            let hashes: Vec<Hash> = unconfirmed.iter().map(|(hash, _)| *hash).collect();
            let mut arrived = Vec::new();
            for ((hash, bytes), length) in unconfirmed.iter().zip(store.len_many(&hashes)) {
                if length?.is_some_and(|length| length as u64 == *bytes) {
                    arrived.push((*hash, *bytes));
                }
            }
            Ok(arrived)
        })
        .await?;
        if arrived.is_empty() {
            return Ok(());
        }
        self.with_database(move |_, database| confirm(database, &arrived))
            .await
    }

    /// How long an upload no block took, or an `until-committed` hold, lasts.
    pub(crate) fn max_index_bytes(&self) -> u64 {
        self.limits.max_index_bytes
    }

    pub(crate) fn upload_grace(&self) -> Duration {
        self.limits.upload_grace
    }

    /// How long a presigned URL for an object's bytes stays valid: well inside
    /// `StorageLimits::upload_grace`, so an upload can never land after the reservation it was
    /// signed for was collected and its object deleted.
    pub fn transfer_lifetime(&self) -> Duration {
        TRANSFER_LIFETIME.min(self.limits.upload_grace / 2)
    }

    fn reserve(
        &self,
        graph: &BlockGraph,
        database: &Connection,
        identity: Identity,
        sized: &[(Hash, u64)],
        carried: bool,
    ) -> Result<(), ServerError> {
        let transaction = crate::store::write_transaction(database)?;
        let payer = payer_of(&transaction, identity.account)?;
        let account = identity.account.to_string();
        let now = be_block_commit::now_milliseconds();
        let mut pending = 0;
        let mut new_to_organization = 0;
        let mut seen = BTreeSet::new();
        for (hash, bytes) in sized {
            if !seen.insert(*hash) {
                continue;
            }
            let known: Option<(i64, bool)> = transaction
                .prepare_cached("SELECT bytes, confirmed FROM object_sizes WHERE hash = ?1")?
                .query_row([hash.to_hex()], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()?;
            // Bytes an upload sends to an object already stored change nothing the server can
            // see, so only carried bytes, bytes that are the first to arrive, or being able to
            // read it already prove the uploader has the object; knowing its hash and length
            // proves nothing.
            let proven = match known {
                Some((known, _)) if known != to_sql(*bytes) => {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        format!("object {hash} is {known} bytes, not {bytes}"),
                    ));
                }
                Some((_, confirmed)) => {
                    carried
                        || !confirmed
                        || crate::blocks::readable(graph, &transaction, identity, *hash)?
                }
                None => {
                    transaction
                        .prepare_cached("INSERT INTO object_sizes (hash, bytes) VALUES (?1, ?2)")?
                        .execute(params![hash.to_hex(), to_sql(*bytes)])?;
                    true
                }
            };
            if !upload_charged(&transaction, payer, *hash)? {
                pending += bytes;
            }
            if !held_in_organization(&transaction, identity.organization, *hash)? {
                new_to_organization += bytes;
            }
            transaction
                .prepare_cached(
                    "INSERT INTO object_uploads
                         (organization_id, account_id, hash, payer, uploaded, proven)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT (organization_id, account_id, hash)
                     DO UPDATE SET uploaded = excluded.uploaded,
                         proven = proven OR excluded.proven",
                )?
                .execute(params![
                    identity.organization.to_string(),
                    account,
                    hash.to_hex(),
                    payer.to_string(),
                    now,
                    proven
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
    /// uploaders, and ends `until-committed` holds no commit took within it
    /// (`collect_holds`). A running server calls it every `UPLOAD_SWEEP_INTERVAL` and when it
    /// starts. Answers how many objects it deleted.
    pub async fn collect_uploads(&self) -> Result<usize, ServerError> {
        let grace = i64::try_from(self.limits.upload_grace.as_millis()).unwrap_or(i64::MAX);
        let cutoff = be_block_commit::now_milliseconds().saturating_sub(grace);
        let mut deleted = self.collect_holds().await?;
        loop {
            let (freed, more) = self
                .with_database(move |store, database| {
                    let transaction = crate::store::write_transaction(database)?;
                    let expired: Vec<(String, String, String, String)> = {
                        let mut statement = transaction.prepare_cached(
                            "SELECT organization_id, account_id, hash, payer FROM object_uploads
                         WHERE uploaded <= ?1 LIMIT ?2",
                        )?;
                        let rows = statement.query_map(
                            params![cutoff, to_sql(UPLOADS_SWEPT_AT_ONCE as u64)],
                            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                        )?;
                        rows.collect::<Result<_, _>>()?
                    };
                    let mut freed = Vec::new();
                    for (organization, account, hex, payer) in &expired {
                        let hash = Hash::from_hex(hex).ok_or(ServerError::Corrupt)?;
                        transaction.execute(
                            "DELETE FROM object_uploads
                         WHERE organization_id = ?1 AND account_id = ?2 AND hash = ?3",
                            params![organization, account, hex],
                        )?;
                        store.uncharge_upload(&transaction, parse_uuid(payer)?, hash)?;
                        if store.object_refs().count(hash) == 0 {
                            freed.push(hash);
                        }
                    }
                    let freed = forget_objects(&transaction, &freed)?;
                    transaction.commit()?;
                    store.objects().remove_freed(&freed);
                    Ok((freed.len(), expired.len() >= UPLOADS_SWEPT_AT_ONCE))
                })
                .await?;
            deleted += freed;
            if !more {
                return Ok(deleted);
            }
        }
    }

    /// Moves the charge for `block`'s subtree to the payer of where it is going: the new parent's
    /// payer, or at the top the mover's (an agent's creator), so nobody can put what they filled
    /// onto someone else's quota by moving it to the top. Moving to the trash changes nothing until it is
    /// collected. Re-charging touches each block of the moved subtree and its objects, nothing
    /// else, and is refused when it would take the new payer past its quota. Call it before
    /// `BlockGraph::set_parent`, inside the move's transaction.
    pub(crate) fn follow_payer(
        &self,
        graph: &BlockGraph,
        connection: &Connection,
        mover: Identity,
        block: Uuid,
        parent: BlockParent,
    ) -> Result<(), ServerError> {
        let organization = mover.organization;
        if !graph.contains(block) {
            return Err(ServerError::Corrupt);
        }
        let payer = match parent {
            BlockParent::Detached => return Ok(()),
            BlockParent::Block(parent) => block_payer(connection, organization, parent)?,
            BlockParent::Root => payer_of(connection, mover.account)?,
        };
        if block_payer(connection, organization, block)? == payer {
            return Ok(());
        }
        let before = usage_of(connection, payer)?;
        let mut changed = BTreeSet::from([payer]);
        for below in graph.subtree(block) {
            let old = block_payer(connection, organization, below)?;
            if old == payer {
                continue;
            }
            for hash in block_objects(connection, organization, below)? {
                let bytes = self.object_bytes(connection, hash)?;
                if count_account_object(connection, old, hash, false)? {
                    shift_usage(connection, old, Column::Held, bytes, false)?;
                    changed.insert(old);
                }
                if count_account_object(connection, payer, hash, true)? {
                    shift_usage(connection, payer, Column::Held, bytes, true)?;
                }
            }
            connection
                .prepare_cached(
                    "UPDATE blocks SET payer = ?3 WHERE organization_id = ?1 AND id = ?2",
                )?
                .execute(params![
                    organization.to_string(),
                    below.to_string(),
                    payer.to_string()
                ])?;
        }
        let after = usage_of(connection, payer)?;
        if let Some(quota) = self.limits.account_quota
            && after.total() > before.total()
            && after.total() > quota
        {
            return Err(over_account_quota(quota, after.total()));
        }
        for account in changed {
            self.usage_changed(account);
        }
        Ok(())
    }

    /// The caller's blocks took these objects: its uploads of them are used, so they stop
    /// counting as its pending storage, and from now on access to a block that holds them
    /// decides who may hold them again.
    fn take_uploads(
        &self,
        connection: &Connection,
        identity: Identity,
        hashes: &[Hash],
    ) -> Result<(), ServerError> {
        for hash in hashes {
            let payer: Option<String> = connection
                .prepare_cached(
                    "DELETE FROM object_uploads
                     WHERE organization_id = ?1 AND account_id = ?2 AND hash = ?3
                     RETURNING payer",
                )?
                .query_row(
                    params![
                        identity.organization.to_string(),
                        identity.account.to_string(),
                        hash.to_hex()
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(payer) = payer {
                self.uncharge_upload(connection, parse_uuid(&payer)?, *hash)?;
            }
        }
        Ok(())
    }

    /// After an upload row of `payer`'s for `hash` went, stops charging it unless another of
    /// its uploads still holds the same object.
    fn uncharge_upload(
        &self,
        connection: &Connection,
        payer: Uuid,
        hash: Hash,
    ) -> Result<(), ServerError> {
        if !upload_charged(connection, payer, hash)? {
            let bytes = self.object_bytes(connection, hash)?;
            shift_usage(connection, payer, Column::Pending, bytes, false)?;
            self.usage_changed(payer);
        }
        Ok(())
    }

    /// Tells the connections following `payer`'s storage that it changed. The change may not be
    /// committed yet; they read it under the database lock this caller holds, so after it.
    fn usage_changed(&self, payer: Uuid) {
        let _ = self.usage_changes.send(payer);
    }

    /// The account whose quota `account`'s uploads count against: its own, or an agent's
    /// creator's.
    pub async fn payer(&self, account: Uuid) -> Result<Uuid, ServerError> {
        self.with_database(move |_, database| payer_of(database, account))
            .await
    }

    pub async fn storage_usage(&self, identity: Identity) -> Result<AccountStorage, ServerError> {
        self.with_database(move |store, database| {
            let usage = usage_of(&database, payer_of(&database, identity.account)?)?;
            Ok(AccountStorage {
                held: usage.held,
                pending: usage.pending,
                quota: store.limits.account_quota,
            })
        })
        .await
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
                connection
                    .prepare_cached(
                        "INSERT INTO block_sizes (organization_id, block_id, size, tree_size)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT (organization_id, block_id)
                         DO UPDATE SET size = excluded.size, tree_size = excluded.tree_size",
                    )?
                    .execute(params![
                        organization_id,
                        block.to_string(),
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

/// Records that the store holds `objects`, each of the length it was reserved with.
fn confirm(connection: &Connection, objects: &[(Hash, u64)]) -> Result<(), ServerError> {
    let transaction = crate::store::write_transaction(connection)?;
    for (hash, bytes) in objects {
        transaction
            .prepare_cached("UPDATE object_sizes SET confirmed = 1 WHERE hash = ?1 AND bytes = ?2")?
            .execute(params![hash.to_hex(), to_sql(*bytes)])?;
    }
    transaction.commit()?;
    Ok(())
}

/// Of objects no block holds any more, answers those that may be deleted, and forgets their
/// sizes: an object someone has uploaded and not yet used stays until its upload is collected,
/// so a free never races an upload of the same object into an untracked file.
pub(crate) fn forget_objects(
    connection: &Connection,
    freed: &[Hash],
) -> Result<Vec<Hash>, ServerError> {
    let mut removable = Vec::new();
    for hash in distinct(freed) {
        let uploaded: bool = connection
            .prepare_cached("SELECT EXISTS(SELECT 1 FROM object_uploads WHERE hash = ?1)")?
            .query_row([hash.to_hex()], |row| row.get(0))?;
        if uploaded {
            continue;
        }
        connection.execute("DELETE FROM object_sizes WHERE hash = ?1", [hash.to_hex()])?;
        removable.push(hash);
    }
    Ok(removable)
}

/// Who pays for a new block: its parent's payer, so whatever anyone makes inside someone's tree
/// is that tree's owner's; at the top, its author, or an agent's creator. It is stored on the
/// block, so removing an agent never moves what it made onto a fresh quota.
pub(crate) fn payer_for_new(
    connection: &Connection,
    organization: Uuid,
    parent: BlockParent,
    author: Uuid,
) -> Result<Uuid, ServerError> {
    match parent {
        BlockParent::Block(parent) => block_payer(connection, organization, parent),
        BlockParent::Root | BlockParent::Detached => payer_of(connection, author),
    }
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

/// The account a block's objects are charged to (`payer_for_new`, then moves).
fn block_payer(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Uuid, ServerError> {
    let payer: String = connection
        .prepare_cached("SELECT payer FROM blocks WHERE organization_id = ?1 AND id = ?2")?
        .query_row(
            params![organization.to_string(), block.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(ServerError::Corrupt)?;
    parse_uuid(&payer)
}

/// The distinct objects a block holds, by any version or by `HoldObjects`.
fn block_objects(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Vec<Hash>, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT hash FROM commit_objects WHERE organization_id = ?1 AND block_id = ?2
         UNION SELECT hash FROM block_objects WHERE organization_id = ?1 AND block_id = ?2",
    )?;
    let rows = statement.query_map(
        params![organization.to_string(), block.to_string()],
        |row| row.get::<_, String>(0),
    )?;
    let mut hashes = Vec::new();
    for row in rows {
        hashes.push(Hash::from_hex(&row?).ok_or(ServerError::Corrupt)?);
    }
    Ok(hashes)
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

/// Whether `payer` is already charged for an upload of `hash`, by any account it pays for, in
/// any organization.
fn upload_charged(connection: &Connection, payer: Uuid, hash: Hash) -> Result<bool, ServerError> {
    Ok(connection
        .prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM object_uploads WHERE payer = ?1 AND hash = ?2)",
        )?
        .query_row(params![payer.to_string(), hash.to_hex()], |row| row.get(0))?)
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
