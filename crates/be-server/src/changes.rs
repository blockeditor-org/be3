//! The organization's durable change sequence, which `changed-since` pages through
//! (plans/search.md, "Left open").
//!
//! One strictly increasing counter per organization (`organizations.change_sequence`) is
//! advanced in the transaction of every write that changes what a block holds, where it lies
//! or who may view it (every graph change holds the store's one database mutex, so writes are
//! ordered). A block's `changed` is the sequence of its last such write. A block that leaves a
//! top-level block's tree (moved out of it, trashed, or deleted for good while in it) leaves a
//! row in `block_departures` for that root, so a reader of one tree is told of each block
//! that left that tree, and of no other. Unlike the graph's `revision`, which restarts with the
//! process, the sequence survives restarts.

use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::ServerError;

/// How long a departure is remembered, in milliseconds (a day, as the search index keeps its
/// own history).
const DEPARTURES_KEPT: i64 = 24 * 60 * 60 * 1000;

/// The next sequence of the organization.
fn next(database: &Connection, organization: &str) -> Result<i64, ServerError> {
    Ok(database.query_row(
        "UPDATE organizations SET change_sequence = change_sequence + 1
         WHERE id = ?1 RETURNING change_sequence",
        params![organization],
        |row| row.get(0),
    )?)
}

/// Marks blocks as changed (their content, name, references or who may view them).
pub(crate) fn touch(
    database: &Connection,
    organization: Uuid,
    blocks: &[Uuid],
) -> Result<(), ServerError> {
    if blocks.is_empty() {
        return Ok(());
    }
    let organization = organization.to_string();
    let sequence = next(database, &organization)?;
    mark(database, &organization, blocks, sequence)
}

fn mark(
    database: &Connection,
    organization: &str,
    blocks: &[Uuid],
    sequence: i64,
) -> Result<(), ServerError> {
    let mut update = database
        .prepare_cached("UPDATE blocks SET changed = ?3 WHERE organization_id = ?1 AND id = ?2")?;
    for block in blocks {
        update.execute(params![organization, block.to_string(), sequence])?;
    }
    Ok(())
}

/// Marks blocks as moved: they and everything below them lie in another tree now, and they
/// left `from`, the top-level block of the tree they were in (none when it was not a tree:
/// from the trash).
pub(crate) fn touch_moved(
    database: &Connection,
    organization: Uuid,
    blocks: &[Departing],
    from: Option<Uuid>,
    now: i64,
) -> Result<(), ServerError> {
    let organization = organization.to_string();
    let sequence = next(database, &organization)?;
    let ids: Vec<Uuid> = blocks.iter().map(|departing| departing.block).collect();
    mark(database, &organization, &ids, sequence)?;
    if let Some(from) = from {
        departed(database, &organization, from, blocks, sequence, now)?;
    }
    Ok(())
}

/// A block that leaves a tree, and the account that alone may read it if it lies in an
/// owner-only block (read before the move, while it still does).
#[derive(Clone, Copy)]
pub(crate) struct Departing {
    pub block: Uuid,
    pub owner: Option<Uuid>,
}

/// Records that a block is deleted for good while in the tree of `root`, which a block in the
/// trash already recorded when it was trashed.
pub(crate) fn removed(
    database: &Connection,
    organization: Uuid,
    root: Option<Uuid>,
    block: Departing,
    now: i64,
) -> Result<(), ServerError> {
    let Some(root) = root else {
        return Ok(());
    };
    let organization = organization.to_string();
    let sequence = next(database, &organization)?;
    departed(database, &organization, root, &[block], sequence, now)
}

/// Remembers that `blocks` left `root`'s tree, and forgets the departures past
/// `DEPARTURES_KEPT`.
fn departed(
    database: &Connection,
    organization: &str,
    root: Uuid,
    blocks: &[Departing],
    sequence: i64,
    now: i64,
) -> Result<(), ServerError> {
    database.execute(
        "DELETE FROM block_departures WHERE organization_id = ?1 AND time < ?2",
        params![organization, now - DEPARTURES_KEPT],
    )?;
    // A block in an owner-only block is named only to the account that may read it.
    let mut insert = database.prepare_cached(
        "INSERT OR REPLACE INTO block_departures
             (organization_id, root, block_id, changed, time, owner)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for departing in blocks {
        insert.execute(params![
            organization,
            root.to_string(),
            departing.block.to_string(),
            sequence,
            now,
            departing.owner.map(|owner| owner.to_string()),
        ])?;
    }
    Ok(())
}
