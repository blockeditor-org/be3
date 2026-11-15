//! Each block's journal: the operations its session's owners sequenced since the last commit,
//! sealed, and the base they apply to (a commit and the sealed session state as of it). It sits
//! beside history, never in it, so retention never sees it.

use be_block_commit::CommitId;
use be_block_protocol::{ErrorCode, Journal, JournalBase, JournalEntry};
use be_block_store::Hash;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::ServerError;

/// The entries a journal holds at most. An owner commits once its journal passes 1 MiB
/// (`JOURNAL_BYTES` in be-sdk-type-guest), so this only stops a client that never does.
pub const MAX_JOURNAL_BYTES: usize = 4 * 1024 * 1024;

/// The session state a base holds at most.
pub const MAX_BASE_STATE_BYTES: usize = 4 * 1024 * 1024;

struct Head {
    tag: Hash,
    generation: u64,
    base: JournalBase,
    lost: bool,
}

fn head(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Option<Head>, ServerError> {
    database
        .query_row(
            "SELECT tag, generation, base_commit, base_sequence, base_state, lost FROM block_journals
             WHERE organization_id = ?1 AND block_id = ?2",
            params![organization.to_string(), block.to_string()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, bool>(5)?,
                ))
            },
        )
        .optional()?
        .map(|(tag, generation, commit, sequence, state, lost)| {
            Ok(Head {
                tag: Hash::from_bytes(tag.try_into().map_err(|_| ServerError::Corrupt)?),
                generation: generation as u64,
                base: JournalBase {
                    commit: commit.map(|commit| parse_commit(&commit)).transpose()?,
                    sequence: sequence as u64,
                    state,
                },
                lost,
            })
        })
        .transpose()
}

fn last_sequence(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Option<u64>, ServerError> {
    Ok(database
        .query_row(
            "SELECT MAX(sequence) FROM journal_entries
             WHERE organization_id = ?1 AND block_id = ?2",
            params![organization.to_string(), block.to_string()],
            |row| row.get::<_, Option<i64>>(0),
        )?
        .map(|sequence| sequence as u64))
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
    let mut statement = database.prepare_cached(
        "SELECT sequence, generation, payload FROM journal_entries
         WHERE organization_id = ?1 AND block_id = ?2 ORDER BY sequence",
    )?;
    let entries = statement
        .query_map(
            params![organization.to_string(), block.to_string()],
            |row| {
                Ok(JournalEntry {
                    sequence: row.get::<_, i64>(0)? as u64,
                    generation: row.get::<_, i64>(1)? as u64,
                    payload: row.get(2)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(Journal {
        tag: head.tag,
        base: head.base,
        entries,
        lost: head.lost,
    }))
}

fn refuse(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

/// Appends what the owner of `generation` sequenced. The caller checked it owns the session.
pub fn append(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    generation: u64,
    tag: Hash,
    entries: &[JournalEntry],
) -> Result<(), ServerError> {
    let transaction = be_block_sqlite_schema::write_transaction(database)?;
    let (org, id) = (organization.to_string(), block.to_string());
    match head(&transaction, organization, block)? {
        None => {
            transaction.execute(
                "INSERT INTO block_journals
                     (organization_id, block_id, tag, generation, base_commit, base_sequence,
                      base_state)
                 VALUES (?1, ?2, ?3, ?4, NULL, 0, x'')",
                params![org, id, tag.as_bytes().as_slice(), generation as i64],
            )?;
        }
        Some(head) if head.tag != tag => {
            return Err(refuse("the journal is another build's: rebase it first"));
        }
        Some(head) if head.generation > generation => {
            return Err(refuse("the session has a newer owner"));
        }
        Some(head) if head.generation < generation => {
            transaction.execute(
                "UPDATE block_journals SET generation = ?3
                 WHERE organization_id = ?1 AND block_id = ?2",
                params![org, id, generation as i64],
            )?;
        }
        Some(_) => {}
    }
    let mut last = last_sequence(&transaction, organization, block)?
        .or(head(&transaction, organization, block)?.map(|head| head.base.sequence))
        .unwrap_or(0);
    let mut bytes: usize = transaction.query_row(
        "SELECT COALESCE(SUM(LENGTH(payload)), 0) FROM journal_entries
         WHERE organization_id = ?1 AND block_id = ?2",
        params![org, id],
        |row| row.get::<_, i64>(0),
    )? as usize;
    {
        let mut insert = transaction.prepare_cached(
            "INSERT INTO journal_entries (organization_id, block_id, sequence, generation, payload)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for entry in entries {
            if entry.sequence <= last {
                continue;
            }
            if entry.generation != generation {
                return Err(refuse("a journal entry of another generation"));
            }
            bytes += entry.payload.len();
            crate::limits::at_most("a journal", bytes, MAX_JOURNAL_BYTES)?;
            insert.execute(params![
                org,
                id,
                entry.sequence as i64,
                generation as i64,
                entry.payload
            ])?;
            last = entry.sequence;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// Starts the journal over from `base`, dropping every entry: an owner rebases after each
/// commit, and on taking a session before it appends.
pub fn rebase(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    generation: u64,
    tag: Hash,
    base: &JournalBase,
) -> Result<(), ServerError> {
    crate::limits::at_most("a journal's base", base.state.len(), MAX_BASE_STATE_BYTES)?;
    let transaction = be_block_sqlite_schema::write_transaction(database)?;
    let (org, id) = (organization.to_string(), block.to_string());
    let before = head(&transaction, organization, block)?;
    if before
        .as_ref()
        .is_some_and(|head| head.generation > generation)
    {
        return Err(refuse("the session has a newer owner"));
    }
    let dropped = last_sequence(&transaction, organization, block)?.is_some();
    // Another build's entries are lost here; every device opening the block is told so until
    // the journal moves to another commit.
    let lost = before.as_ref().is_some_and(|head| {
        (head.tag != tag && dropped) || (head.lost && head.base.commit == base.commit)
    });
    transaction.execute(
        "DELETE FROM journal_entries WHERE organization_id = ?1 AND block_id = ?2",
        params![org, id],
    )?;
    transaction.execute(
        "INSERT OR REPLACE INTO block_journals
             (organization_id, block_id, tag, generation, base_commit, base_sequence, base_state,
              lost)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            org,
            id,
            tag.as_bytes().as_slice(),
            generation as i64,
            base.commit.map(|commit| commit.hash().to_hex()),
            base.sequence as i64,
            base.state,
            lost
        ],
    )?;
    transaction.commit()?;
    Ok(())
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

    /// The journals of `blocks` that have entries, in order, while they fit in `budget` bytes;
    /// a reader asks again for one left out.
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

    pub async fn append_journal(
        &self,
        organization: Uuid,
        block: Uuid,
        generation: u64,
        tag: Hash,
        entries: Vec<JournalEntry>,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            append(database, organization, block, generation, tag, &entries)
        })
        .await
    }

    pub async fn rebase_journal(
        &self,
        organization: Uuid,
        block: Uuid,
        generation: u64,
        tag: Hash,
        base: JournalBase,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            rebase(database, organization, block, generation, tag, &base)
        })
        .await
    }
}
