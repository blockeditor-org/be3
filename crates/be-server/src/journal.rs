//! Each block's journal: its session's order since the last seal, sealed entries the server
//! numbered, and the base they apply to (a commit and the sealed session state as of it). It
//! sits beside history, never in it, so retention never sees it. The server numbers what a
//! session's replicas submit here (`submit`), and the lease holder's seals trim it (`rebase`) in
//! the transaction of the publish that seals it.

use be_block_commit::CommitId;
use be_block_protocol::{ErrorCode, Journal, JournalBase, JournalEntry};
use be_block_session::{OpId, Sequencer, Submission};
use be_block_store::Hash;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::ServerError;

/// The entries a journal holds at most. A lease holder commits once its journal passes 1 MiB
/// (`JOURNAL_BYTES` in be-sdk-type-guest), so this only stops a session nobody seals.
pub const MAX_JOURNAL_BYTES: usize = 4 * 1024 * 1024;

/// The session state a base holds at most.
pub const MAX_BASE_STATE_BYTES: usize = 4 * 1024 * 1024;

struct Head {
    tag: Hash,
    base: JournalBase,
    order: Sequencer,
    lost: bool,
}

fn head(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Option<Head>, ServerError> {
    database
        .query_row(
            "SELECT tag, base_commit, base_sequence, base_state, sequence, barrier, lost
             FROM block_journals WHERE organization_id = ?1 AND block_id = ?2",
            params![organization.to_string(), block.to_string()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, bool>(6)?,
                ))
            },
        )
        .optional()?
        .map(
            |(tag, commit, base_sequence, state, sequence, barrier, lost)| {
                Ok(Head {
                    tag: Hash::from_bytes(tag.try_into().map_err(|_| ServerError::Corrupt)?),
                    base: JournalBase {
                        commit: commit.map(|commit| parse_commit(&commit)).transpose()?,
                        sequence: base_sequence as u64,
                        state,
                    },
                    order: Sequencer {
                        sequence: sequence as u64,
                        barrier: barrier as u64,
                    },
                    lost,
                })
            },
        )
        .transpose()
}

fn entries(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Vec<JournalEntry>, ServerError> {
    let mut statement = database.prepare_cached(
        "SELECT sequence, origin, counter, base, floor, received, payload FROM journal_entries
         WHERE organization_id = ?1 AND block_id = ?2 ORDER BY sequence",
    )?;
    Ok(statement
        .query_map(
            params![organization.to_string(), block.to_string()],
            |row| {
                Ok(JournalEntry {
                    sequence: row.get::<_, i64>(0)? as u64,
                    origin: row.get::<_, i64>(1)? as u64,
                    counter: row.get::<_, i64>(2)? as u64,
                    base: row.get::<_, i64>(3)? as u64,
                    floor: row.get::<_, i64>(4)? as u64,
                    received: row.get::<_, Option<i64>>(5)?,
                    payload: row.get(6)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?)
}

/// A block's journal, entries in order; none when it has none.
pub fn read(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Option<Journal>, ServerError> {
    let Some(head) = head(database, organization, block)? else {
        return Ok(None);
    };
    Ok(Some(Journal {
        tag: head.tag,
        base: head.base,
        entries: entries(database, organization, block)?,
        lost: head.lost,
    }))
}

fn refuse(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

/// What `submit` did with a submission.
#[derive(Debug)]
pub enum Submitted {
    /// Numbered as this entry, which every participant is to hear.
    Numbered(JournalEntry),
    /// The journal holds its id already: it was numbered before.
    Held,
}

/// Numbers `submission`, made by the build `tag`, and keeps it in the journal. Refused while the
/// journal holds entries of another build; a journal with none becomes `tag`'s.
pub fn submit(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    tag: Hash,
    submission: Submission,
    received: i64,
) -> Result<Submitted, ServerError> {
    let transaction = be_block_sqlite_schema::write_transaction(database)?;
    let (org, id) = (organization.to_string(), block.to_string());
    let held: Option<i64> = transaction
        .query_row(
            "SELECT sequence FROM journal_entries
             WHERE organization_id = ?1 AND block_id = ?2 AND origin = ?3 AND counter = ?4",
            params![
                org,
                id,
                submission.id.origin as i64,
                submission.id.counter as i64
            ],
            |row| row.get(0),
        )
        .optional()?;
    if held.is_some() {
        return Ok(Submitted::Held);
    }
    let mut order = match head(&transaction, organization, block)? {
        None => {
            // A journal starts on the block's head: the content its first entry was made on.
            transaction.execute(
                "INSERT INTO block_journals
                     (organization_id, block_id, tag, base_commit, base_sequence, base_state,
                      sequence, barrier)
                 VALUES (?1, ?2, ?3,
                         (SELECT head FROM blocks WHERE organization_id = ?1 AND id = ?2),
                         0, x'', 0, 0)",
                params![org, id, tag.as_bytes().as_slice()],
            )?;
            Sequencer::starting(0)
        }
        Some(head) if head.tag != tag => {
            let empty = entries(&transaction, organization, block)?
                .iter()
                .all(|entry| entry.sequence <= head.base.sequence);
            if !empty {
                return Err(refuse("the journal holds another build's entries"));
            }
            transaction.execute(
                "UPDATE block_journals SET tag = ?3 WHERE organization_id = ?1 AND block_id = ?2",
                params![org, id, tag.as_bytes().as_slice()],
            )?;
            head.order
        }
        Some(head) => head.order,
    };
    let bytes: usize = transaction.query_row(
        "SELECT COALESCE(SUM(LENGTH(payload)), 0) FROM journal_entries
         WHERE organization_id = ?1 AND block_id = ?2",
        params![org, id],
        |row| row.get::<_, i64>(0),
    )? as usize;
    crate::limits::at_most(
        "a journal",
        bytes + submission.payload.len(),
        MAX_JOURNAL_BYTES,
    )?;
    let OpId { origin, counter } = submission.id;
    let op = order.number(submission, Some(received));
    transaction.execute(
        "INSERT INTO journal_entries
             (organization_id, block_id, sequence, origin, counter, base, floor, received, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            org,
            id,
            op.sequence as i64,
            origin as i64,
            counter as i64,
            op.base as i64,
            op.floor as i64,
            op.received,
            op.payload
        ],
    )?;
    transaction.execute(
        "UPDATE block_journals SET sequence = ?3, barrier = ?4
         WHERE organization_id = ?1 AND block_id = ?2",
        params![org, id, order.sequence as i64, order.barrier as i64],
    )?;
    transaction.commit()?;
    Ok(Submitted::Numbered(JournalEntry {
        sequence: op.sequence,
        origin,
        counter,
        base: op.base,
        floor: op.floor,
        received: op.received,
        payload: op.payload,
    }))
}

/// The lease holder sealed the order through `base`, as the build `tag`, inside the publish's
/// `transaction`: the journal starts from it, keeping the entries after it, and the order goes
/// on from the number answered. A base that goes back while nothing after the journal's own
/// base was numbered (the last one out) starts the order afresh from it. Entries of another
/// build are dropped, since the holder could not have applied them, and every device opening
/// the block is told so until the journal moves to another commit.
pub fn rebase(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    tag: Hash,
    base: &JournalBase,
) -> Result<u64, ServerError> {
    let transaction = database;
    let (org, id) = (organization.to_string(), block.to_string());
    let before = head(&transaction, organization, block)?;
    let other = before.as_ref().is_some_and(|head| head.tag != tag);
    // A seal holds only what was numbered: a base past the order's last number would leave
    // the order numbering below what is sealed.
    if let Some(head) = &before
        && !other
        && base.sequence > head.order.sequence
    {
        return Err(refuse("the journal's base is past the order's last entry"));
    }
    let order = match &before {
        Some(head) if !other && head.order.sequence > base.sequence.max(head.base.sequence) => {
            head.order
        }
        // Nothing past the base: the order goes on from it, wherever that is.
        Some(head) if !other && base.sequence >= head.base.sequence => head.order,
        _ => Sequencer::starting(base.sequence),
    };
    let cut = match other {
        true => i64::MAX,
        false => base.sequence as i64,
    };
    let dropped: bool = other
        && transaction.query_row(
            "SELECT EXISTS (SELECT 1 FROM journal_entries
                 WHERE organization_id = ?1 AND block_id = ?2 AND sequence > ?3)",
            params![
                org,
                id,
                before.as_ref().map_or(0, |head| head.base.sequence) as i64
            ],
            |row| row.get(0),
        )?;
    let lost = dropped
        || before
            .as_ref()
            .is_some_and(|head| head.lost && head.base.commit == base.commit);
    transaction.execute(
        "DELETE FROM journal_entries
         WHERE organization_id = ?1 AND block_id = ?2 AND sequence <= ?3",
        params![org, id, cut],
    )?;
    transaction.execute(
        "INSERT OR REPLACE INTO block_journals
             (organization_id, block_id, tag, base_commit, base_sequence, base_state, sequence,
              barrier, lost)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            org,
            id,
            tag.as_bytes().as_slice(),
            base.commit.map(|commit| commit.hash().to_hex()),
            base.sequence as i64,
            base.state,
            order.sequence as i64,
            order.barrier as i64,
            lost
        ],
    )?;
    Ok(order.sequence)
}

/// Forgets a deleted block's journal.
pub fn forget(database: &Connection, organization: Uuid, block: Uuid) -> Result<(), ServerError> {
    let (org, id) = (organization.to_string(), block.to_string());
    database.execute(
        "DELETE FROM journal_entries WHERE organization_id = ?1 AND block_id = ?2",
        params![org, id],
    )?;
    database.execute(
        "DELETE FROM block_journals WHERE organization_id = ?1 AND block_id = ?2",
        params![org, id],
    )?;
    Ok(())
}

fn parse_commit(hex: &str) -> Result<CommitId, ServerError> {
    Hash::from_hex(hex)
        .map(CommitId::from_hash)
        .ok_or(ServerError::Corrupt)
}

impl crate::store::ServerStore {
    pub async fn read_journal(
        &self,
        organization: Uuid,
        block: Uuid,
    ) -> Result<Option<Journal>, ServerError> {
        self.with_database(move |_, database| read(database, organization, block))
            .await
    }

    /// The journals of `blocks` that have entries, in order, while they fit in
    /// `budget` bytes; a reader asks again for one left out.
    pub async fn read_journals(
        &self,
        organization: Uuid,
        blocks: Vec<Uuid>,
        budget: usize,
    ) -> Result<Vec<Option<Journal>>, ServerError> {
        self.with_database(move |_, database| {
            let mut left = budget;
            blocks
                .into_iter()
                .map(|block| {
                    let journal = read(database, organization, block)?
                        .filter(|journal| !journal.entries.is_empty() || journal.lost);
                    let size = be_block_protocol::encoded_len(&journal);
                    Ok(match size <= left {
                        true => {
                            left -= size;
                            journal
                        }
                        false => None,
                    })
                })
                .collect()
        })
        .await
    }

    pub async fn submit_journal(
        &self,
        organization: Uuid,
        block: Uuid,
        tag: Hash,
        submission: Submission,
    ) -> Result<Submitted, ServerError> {
        let received = be_block_commit::now_milliseconds();
        self.with_database(move |_, database| {
            submit(database, organization, block, tag, submission, received)
        })
        .await
    }
}
