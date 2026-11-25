//! Blocks' signed logs (guides/blocks/server.md, "Signed logs"): beside a
//! block's content, entries the server numbers, stamps and hashes into a Merkle tree
//! (`be_block_keys::merkle`, RFC 6962) that no commit can change. `block_logs` holds each entry
//! (its payload sealed, or none once redacted), `block_log_nodes` every perfect subtree of the
//! tree, so a proof for any page or any older head costs O(log n) lookups, and
//! `block_log_heads` each log's size, root and the bytes it is charged.

use be_block_keys::merkle::{self, Frontier, TreeHash};
use be_block_protocol::{ErrorCode, LogEntry, LogHead, LogPage, Logged, MAX_FRAME_BYTES, NewEntry};
use be_block_store::Hash;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{Identity, ServerError, ServerStore, blocks::visibility};

/// The sealed payload one entry carries at most.
pub const MAX_ENTRY_BYTES: usize = be_block_protocol::MAX_LOG_ENTRY_BYTES;
/// A signature's bytes at most: an Ed25519 signature is 64.
pub const MAX_SIGNATURE_BYTES: usize = 128;
/// Entries one `append` carries at most.
pub const MAX_ENTRIES_PER_APPEND: usize = 256;
/// The payload bytes of one `append`, so the push that tells it fits a frame.
pub const MAX_APPEND_BYTES: usize = MAX_FRAME_BYTES / 8;
/// Entries one account may append a minute, over all blocks: a person's sends, and an outbox
/// draining what it kept offline, are far below it.
pub const MAX_ENTRIES_PER_MINUTE: u32 = 1_200;
/// Entries one page answers at most.
pub const MAX_PAGE_ENTRIES: u32 = 1_024;
/// The bytes of entries a page answers at most; it always answers one.
const PAGE_BYTES: usize = MAX_FRAME_BYTES / 2;
/// What an entry is charged beyond its payload: its signature and row.
const ENTRY_OVERHEAD_BYTES: u64 = 128;

fn invalid(message: impl Into<String>) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

fn hash_of(bytes: Vec<u8>) -> Result<TreeHash, ServerError> {
    bytes.try_into().map_err(|_| ServerError::Corrupt)
}

/// A perfect subtree of the log's tree (`merkle::subtree`).
fn node(
    database: &Connection,
    organization: &str,
    block: &str,
    level: u32,
    index: u64,
) -> Result<TreeHash, ServerError> {
    let hash: Option<Vec<u8>> = database
        .prepare_cached(
            "SELECT hash FROM block_log_nodes
             WHERE organization_id = ?1 AND block_id = ?2 AND level = ?3 AND position = ?4",
        )?
        .query_row(params![organization, block, level, index as i64], |row| {
            row.get(0)
        })
        .optional()?;
    hash_of(hash.ok_or(ServerError::Corrupt)?)
}

/// A log's size, root and charged bytes; the empty log for a block with none.
struct Kept {
    size: u64,
    root: TreeHash,
    bytes: u64,
    /// How many of its entries were redacted, which numbers each redaction.
    redactions: u64,
}

fn kept(database: &Connection, organization: &str, block: &str) -> Result<Kept, ServerError> {
    let row: Option<(i64, Vec<u8>, i64, i64)> = database
        .prepare_cached(
            "SELECT size, root, bytes, redactions FROM block_log_heads
             WHERE organization_id = ?1 AND block_id = ?2",
        )?
        .query_row(params![organization, block], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .optional()?;
    Ok(match row {
        Some((size, root, bytes, redactions)) => Kept {
            size: size as u64,
            root: hash_of(root)?,
            bytes: bytes as u64,
            redactions: redactions as u64,
        },
        None => Kept {
            size: 0,
            root: merkle::empty_root(),
            bytes: 0,
            redactions: 0,
        },
    })
}

fn head_of(kept: &Kept) -> LogHead {
    LogHead {
        size: kept.size,
        root: Hash::from_bytes(kept.root),
    }
}

/// The bytes a block's log is charged to its payer (`ServerStore::charge_log`).
pub fn bytes(database: &Connection, organization: Uuid, block: Uuid) -> Result<u64, ServerError> {
    Ok(kept(database, &organization.to_string(), &block.to_string())?.bytes)
}

/// A block's log's head.
pub fn head(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<LogHead, ServerError> {
    Ok(head_of(&kept(
        database,
        &organization.to_string(),
        &block.to_string(),
    )?))
}

/// Whether `head` is a head the block's log had: its size at most the log's, and the root of the
/// log's first `size` entries its root. What a commit naming a log head is checked against.
pub fn holds_prefix(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    head: &LogHead,
) -> Result<bool, ServerError> {
    let (org, id) = (organization.to_string(), block.to_string());
    if head.size > kept(database, &org, &id)?.size {
        return Ok(false);
    }
    let root = merkle::root(head.size, &mut |level, index| {
        node(database, &org, &id, level, index)
    })?;
    Ok(root == *head.root.as_bytes())
}

type EntryRow = (i64, Vec<u8>, String, i64, Vec<u8>, Vec<u8>, Option<Vec<u8>>);

fn entry_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<EntryRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
    ))
}

fn entry_from(row: EntryRow) -> Result<LogEntry, ServerError> {
    let (seq, id, author, arrival, payload_hash, signature, payload) = row;
    Ok(LogEntry {
        seq: seq as u64,
        id: Uuid::from_slice(&id).map_err(|_| ServerError::Corrupt)?,
        author: crate::store::parse_uuid(&author)?,
        arrival,
        payload_hash: Hash::from_bytes(hash_of(payload_hash)?),
        signature,
        payload,
    })
}

/// What `append` did.
#[derive(Debug)]
pub struct Appended {
    /// Where each entry asked for went, in order, those the log held already among them.
    pub logged: Vec<Logged>,
    /// The log's size before.
    pub previous: u64,
    pub head: LogHead,
    /// The entries appended now, which followers are told.
    pub entries: Vec<LogEntry>,
    /// The bytes the entries appended now are charged.
    pub grew: u64,
}

/// Appends `entries` by `author` to the block's log, numbering each next and stamping it with
/// `now` (or the last entry's arrival, should the clock have stepped back, so arrivals never go
/// back along the log). An entry whose id the log holds is answered where it went and not
/// appended again when it is the same entry (its author and payload), and refused when it is
/// not. Runs in the caller's transaction.
pub fn append(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    author: Uuid,
    entries: Vec<NewEntry>,
    now: i64,
) -> Result<Appended, ServerError> {
    let (org, id) = (organization.to_string(), block.to_string());
    let before = kept(database, &org, &id)?;
    let mut frontier = Frontier::of(before.size, &mut |level, index| {
        node(database, &org, &id, level, index)
    })?;
    if frontier.root() != before.root {
        return Err(ServerError::Corrupt);
    }
    let mut arrival: i64 = match before.size {
        0 => now,
        size => database
            .prepare_cached(
                "SELECT arrival FROM block_logs
                 WHERE organization_id = ?1 AND block_id = ?2 AND seq = ?3",
            )?
            .query_row(params![org, id, (size - 1) as i64], |row| row.get(0))?,
    };
    arrival = arrival.max(now);
    let mut logged = Vec::with_capacity(entries.len());
    let mut appended = Vec::new();
    let mut grew = 0;
    for entry in entries {
        let held: Option<(i64, i64, String, Vec<u8>)> = database
            .prepare_cached(
                "SELECT seq, arrival, author, payload_hash FROM block_logs
                 WHERE organization_id = ?1 AND block_id = ?2 AND id = ?3",
            )?
            .query_row(params![org, id, entry.id.as_bytes().as_slice()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .optional()?;
        if let Some((seq, arrival, held_author, held_hash)) = held {
            // Only the same entry sent again is answered: another under its id is refused.
            if held_author != author.to_string()
                || held_hash.as_slice() != Hash::of(&entry.payload).as_bytes()
            {
                return Err(invalid("the log holds another entry under that id"));
            }
            logged.push(Logged {
                seq: seq as u64,
                arrival,
            });
            continue;
        }
        let entry = LogEntry {
            seq: frontier.size(),
            id: entry.id,
            author,
            arrival,
            payload_hash: Hash::of(&entry.payload),
            signature: entry.signature,
            payload: Some(entry.payload),
        };
        database
            .prepare_cached(
                "INSERT INTO block_logs
                     (organization_id, block_id, seq, id, author, arrival, payload_hash,
                      signature, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?
            .execute(params![
                org,
                id,
                entry.seq as i64,
                entry.id.as_bytes().as_slice(),
                author.to_string(),
                entry.arrival,
                entry.payload_hash.as_bytes().as_slice(),
                entry.signature,
                entry.payload,
            ])?;
        let mut insert = database.prepare_cached(
            "INSERT INTO block_log_nodes (organization_id, block_id, level, position, hash)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (level, index, hash) in frontier.push(entry.leaf()) {
            insert.execute(params![org, id, level, index as i64, hash.as_slice()])?;
        }
        grew += charged(&entry);
        logged.push(Logged {
            seq: entry.seq,
            arrival: entry.arrival,
        });
        appended.push(entry);
    }
    let after = Kept {
        size: frontier.size(),
        root: frontier.root(),
        bytes: before.bytes + grew,
        redactions: before.redactions,
    };
    if !appended.is_empty() {
        write_kept(database, &org, &id, &after)?;
    }
    Ok(Appended {
        logged,
        previous: before.size,
        head: head_of(&after),
        entries: appended,
        grew,
    })
}

fn write_kept(
    database: &Connection,
    organization: &str,
    block: &str,
    kept: &Kept,
) -> Result<(), ServerError> {
    database
        .prepare_cached(
            "INSERT INTO block_log_heads
                 (organization_id, block_id, size, root, bytes, redactions)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (organization_id, block_id)
             DO UPDATE SET size = excluded.size, root = excluded.root, bytes = excluded.bytes,
                 redactions = excluded.redactions",
        )?
        .execute(params![
            organization,
            block,
            kept.size as i64,
            kept.root.as_slice(),
            kept.bytes as i64,
            kept.redactions as i64
        ])?;
    Ok(())
}

/// What an entry is charged: its payload while it has one, and what every entry keeps.
fn charged(entry: &LogEntry) -> u64 {
    entry
        .payload
        .as_ref()
        .map_or(0, |payload| payload.len() as u64)
        + ENTRY_OVERHEAD_BYTES
}

/// Checks an append's entries against the limits, before anything is written.
pub fn check(entries: &[NewEntry]) -> Result<(), ServerError> {
    if entries.is_empty() {
        return Err(invalid("an append carries at least one entry"));
    }
    if entries.len() > MAX_ENTRIES_PER_APPEND {
        return Err(crate::limits::too_many(
            "entries one append carries",
            MAX_ENTRIES_PER_APPEND,
        ));
    }
    let mut total = 0;
    let mut ids = std::collections::HashSet::with_capacity(entries.len());
    for entry in entries {
        crate::limits::at_most("an entry's payload", entry.payload.len(), MAX_ENTRY_BYTES)?;
        crate::limits::at_most(
            "an entry's signature",
            entry.signature.len(),
            MAX_SIGNATURE_BYTES,
        )?;
        if !ids.insert(entry.id) {
            return Err(invalid("an append names an entry twice"));
        }
        total += entry.payload.len();
    }
    crate::limits::at_most("an append's payloads", total, MAX_APPEND_BYTES)
}

/// The proofs that tie entries `[previous, head.size)` to `head`, and `head` to the head of
/// size `previous`: what a follower that holds that head needs to take them.
pub fn proofs(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    previous: u64,
    size: u64,
) -> Result<(Vec<Hash>, Vec<Hash>), ServerError> {
    let (org, id) = (organization.to_string(), block.to_string());
    let mut node = |level, index| node(database, &org, &id, level, index);
    let proof = merkle::range_proof(size, previous, size, &mut node)?;
    let consistency = merkle::consistency_proof(previous, size, &mut node)?;
    Ok((hashes(proof), hashes(consistency)))
}

fn hashes(proof: Vec<TreeHash>) -> Vec<Hash> {
    proof.into_iter().map(Hash::from_bytes).collect()
}

/// A page of the block's log from `from`: at most `limit` entries (and `MAX_PAGE_ENTRIES`, and
/// `PAGE_BYTES`, at least one), the head, the proof tying them to it, and with `known`, a size
/// the caller holds a head of, the proof the head extends that one. A `known` past the log's
/// size gets none: the caller finds the log shorter than what it holds. With
/// `redactions_after`, the places of the entries redacted after that many redactions.
pub fn page(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    from: u64,
    limit: u32,
    known: Option<u64>,
    redactions_after: Option<u64>,
) -> Result<LogPage, ServerError> {
    let (org, id) = (organization.to_string(), block.to_string());
    let kept = kept(database, &org, &id)?;
    let from = from.min(kept.size);
    let limit = u64::from(limit.min(MAX_PAGE_ENTRIES));
    let mut statement = database.prepare_cached(
        "SELECT seq, id, author, arrival, payload_hash, signature, payload FROM block_logs
         WHERE organization_id = ?1 AND block_id = ?2 AND seq >= ?3 AND seq < ?4
         ORDER BY seq",
    )?;
    let mut entries = Vec::new();
    let mut room = PAGE_BYTES;
    for row in statement.query_map(
        params![org, id, from as i64, (from + limit).min(kept.size) as i64],
        entry_of,
    )? {
        let entry = entry_from(row?)?;
        let bytes = be_block_protocol::encoded_len(&entry);
        if !entries.is_empty() && bytes > room {
            break;
        }
        room = room.saturating_sub(bytes);
        entries.push(entry);
    }
    let to = from + entries.len() as u64;
    let mut node = |level, index| node(database, &org, &id, level, index);
    let proof = merkle::range_proof(kept.size, from, to, &mut node)?;
    let consistency = match known {
        Some(known) if known <= kept.size => {
            merkle::consistency_proof(known, kept.size, &mut node)?
        }
        _ => Vec::new(),
    };
    let redacted = match redactions_after {
        Some(after) => redacted_since(database, organization, block, after)?.1,
        None => Vec::new(),
    };
    Ok(LogPage {
        head: head_of(&kept),
        entries,
        proof: hashes(proof),
        consistency: hashes(consistency),
        redactions: kept.redactions,
        redacted,
    })
}

/// How many of `block`'s log entries were redacted, and the places of those redacted after
/// the first `after`, in the order they were.
fn redacted_since(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    after: u64,
) -> Result<(u64, Vec<u64>), ServerError> {
    let (org, id) = (organization.to_string(), block.to_string());
    let redactions = kept(database, &org, &id)?.redactions;
    if after >= redactions {
        return Ok((redactions, Vec::new()));
    }
    let redacted = database
        .prepare_cached(
            "SELECT seq FROM block_logs
             WHERE organization_id = ?1 AND block_id = ?2 AND redaction > ?3
             ORDER BY redaction",
        )?
        .query_map(params![org, id, after as i64], |row| row.get::<_, i64>(0))?
        .map(|seq| seq.map(|seq| seq as u64))
        .collect::<Result<Vec<u64>, _>>()?;
    Ok((redactions, redacted))
}

/// Who appended the entry at `seq`, and the bytes redacting its payload frees: none when it
/// holds no payload any more.
pub fn redactable(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    seq: u64,
) -> Result<(Uuid, Option<u64>), ServerError> {
    let row: Option<(String, Option<i64>)> = database
        .prepare_cached(
            "SELECT author, LENGTH(payload) FROM block_logs
             WHERE organization_id = ?1 AND block_id = ?2 AND seq = ?3",
        )?
        .query_row(
            params![organization.to_string(), block.to_string(), seq as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (author, length) = row.ok_or_else(|| invalid("the log holds no entry there"))?;
    Ok((
        crate::store::parse_uuid(&author)?,
        length.map(|length| length as u64),
    ))
}

/// Drops the payload of the entry at `seq`, keeping its hash and leaf, and uncharges its
/// `freed` bytes from the log.
pub fn redact(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    seq: u64,
    freed: u64,
) -> Result<(), ServerError> {
    let (org, id) = (organization.to_string(), block.to_string());
    let mut kept = kept(database, &org, &id)?;
    kept.bytes = kept.bytes.saturating_sub(freed);
    kept.redactions += 1;
    database
        .prepare_cached(
            "UPDATE block_logs SET payload = NULL, redaction = ?4
             WHERE organization_id = ?1 AND block_id = ?2 AND seq = ?3",
        )?
        .execute(params![org, id, seq as i64, kept.redactions as i64])?;
    write_kept(database, &org, &id, &kept)
}

/// Forgets a deleted block's log: its entries, its tree and its head, so no row of it is left.
/// Nothing calls it yet, since no block is deleted for good; a block's log goes with it then.
pub fn forget(database: &Connection, organization: Uuid, block: Uuid) -> Result<(), ServerError> {
    let (org, id) = (organization.to_string(), block.to_string());
    for table in ["block_logs", "block_log_nodes", "block_log_heads"] {
        database.execute(
            &format!("DELETE FROM {table} WHERE organization_id = ?1 AND block_id = ?2"),
            params![org, id],
        )?;
    }
    Ok(())
}

/// One append as its followers are told it (`event.appended`).
#[derive(Clone, Debug)]
pub struct Told {
    pub previous: u64,
    pub head: LogHead,
    pub entries: Vec<LogEntry>,
    pub proof: Vec<Hash>,
    pub consistency: Vec<Hash>,
}

impl ServerStore {
    /// Appends `entries` to `block`'s log as `identity`, charged to the block's payer, in one
    /// transaction: where each went, and what followers are told, none when every entry was
    /// held already.
    pub(crate) async fn append_log(
        &self,
        identity: Identity,
        block: Uuid,
        entries: Vec<NewEntry>,
    ) -> Result<(Vec<Logged>, Option<Told>), ServerError> {
        check(&entries)?;
        self.with_graph(identity.organization, move |store, graph, database| {
            crate::blocks::require_edit(graph, &visibility(identity), block)?;
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let organization = identity.organization;
            let appended = append(
                &transaction,
                organization,
                block,
                identity.account,
                entries,
                be_block_commit::now_milliseconds(),
            )?;
            store.charge_log(
                graph,
                &transaction,
                organization,
                block,
                appended.grew,
                true,
            )?;
            let told = match appended.entries.is_empty() {
                true => None,
                false => {
                    let (proof, consistency) = proofs(
                        &transaction,
                        organization,
                        block,
                        appended.previous,
                        appended.head.size,
                    )?;
                    Some(Told {
                        previous: appended.previous,
                        head: appended.head,
                        entries: appended.entries,
                        proof,
                        consistency,
                    })
                }
            };
            transaction.commit()?;
            Ok((appended.logged, told))
        })
        .await
    }

    /// A page of `block`'s log for `identity`, who may view it (`page`).
    pub(crate) async fn read_log(
        &self,
        identity: Identity,
        block: Uuid,
        from: u64,
        limit: u32,
        known: Option<u64>,
        redactions_after: Option<u64>,
    ) -> Result<LogPage, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            crate::blocks::require(
                graph,
                &visibility(identity),
                block,
                be_block_graph::Access::View,
                "viewable",
            )?;
            page(
                database,
                identity.organization,
                block,
                from,
                limit,
                known,
                redactions_after,
            )
        })
        .await
    }

    /// For each log asked of that `identity` may view, its redactions after the count asked
    /// (`page`'s `redactions_after`), in one read.
    pub(crate) async fn log_redactions(
        &self,
        identity: Identity,
        asked: Vec<be_block_protocol::LogRedactionsQuery>,
    ) -> Result<Vec<be_block_protocol::LogRedactions>, ServerError> {
        if asked.len() > be_block_protocol::MAX_LOG_REDACTIONS_ASKED {
            return Err(crate::limits::too_many(
                "logs asked of at once",
                be_block_protocol::MAX_LOG_REDACTIONS_ASKED,
            ));
        }
        self.with_graph(identity.organization, move |_, graph, database| {
            let seen = visibility(identity);
            let mut answered = Vec::new();
            for ask in asked {
                let viewable = crate::blocks::require(
                    graph,
                    &seen,
                    ask.block,
                    be_block_graph::Access::View,
                    "viewable",
                )
                .is_ok();
                if !viewable {
                    continue;
                }
                let (redactions, redacted) =
                    redacted_since(database, identity.organization, ask.block, ask.after)?;
                answered.push(be_block_protocol::LogRedactions {
                    block: ask.block,
                    redactions,
                    redacted,
                });
            }
            Ok(answered)
        })
        .await
    }

    /// Redacts the entry at `seq` of `block`'s log: by its author, or with Manage on the block.
    /// Whether it held a payload to redact, which its followers are told of.
    pub(crate) async fn redact_log(
        &self,
        identity: Identity,
        block: Uuid,
        seq: u64,
    ) -> Result<bool, ServerError> {
        self.with_graph(identity.organization, move |store, graph, database| {
            let seen = visibility(identity);
            crate::blocks::require(
                graph,
                &seen,
                block,
                be_block_graph::Access::View,
                "viewable",
            )?;
            let organization = identity.organization;
            let (author, length) = redactable(database, organization, block, seq)?;
            if author != identity.account && !seen.can_manage(graph, block) {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "only an entry's author, or a manager of its block, redacts it".into(),
                ));
            }
            let Some(length) = length else {
                return Ok(false);
            };
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            redact(&transaction, organization, block, seq, length)?;
            store.charge_log(graph, &transaction, organization, block, length, false)?;
            transaction.commit()?;
            Ok(true)
        })
        .await
    }
}
