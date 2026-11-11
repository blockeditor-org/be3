//! The organization's durable change sequence, which `changed-since` pages through
//! (plans/search.md, "Step 1, designed").
//!
//! One strictly increasing counter per organization (`organizations.change_sequence`) is
//! advanced in the transaction of every write that changes what a block holds or where it
//! lies, under the lock `ServerStore::with_graph` takes. A block's `changed` is the sequence
//! of its last such write and `moved` that of its last move (a new parent, trash included);
//! a block collected for good leaves a row in `block_removals`. Unlike the graph's
//! `revision`, which restarts with the process, the sequence survives restarts.

use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::ServerError;

/// How long a collected block is remembered, in milliseconds (a day, as the search index
/// keeps its own history).
const REMOVALS_KEPT: i64 = 24 * 60 * 60 * 1000;

/// The next sequence of the organization.
fn next(database: &Connection, organization: &str) -> Result<i64, ServerError> {
    Ok(database.query_row(
        "UPDATE organizations SET change_sequence = change_sequence + 1
         WHERE id = ?1 RETURNING change_sequence",
        params![organization],
        |row| row.get(0),
    )?)
}

/// Marks blocks as changed (their content, name or references).
pub(crate) fn touch(
    database: &Connection,
    organization: Uuid,
    blocks: &[Uuid],
) -> Result<(), ServerError> {
    mark(database, organization, blocks, false)
}

/// Marks blocks as moved: a new parent, so a different tree, which changes for every block
/// below as well.
pub(crate) fn touch_moved(
    database: &Connection,
    organization: Uuid,
    blocks: &[Uuid],
) -> Result<(), ServerError> {
    mark(database, organization, blocks, true)
}

fn mark(
    database: &Connection,
    organization: Uuid,
    blocks: &[Uuid],
    moved: bool,
) -> Result<(), ServerError> {
    if blocks.is_empty() {
        return Ok(());
    }
    let organization = organization.to_string();
    let sequence = next(database, &organization)?;
    let mut update = database.prepare_cached(
        "UPDATE blocks SET changed = ?3, moved = CASE WHEN ?4 THEN ?3 ELSE moved END
         WHERE organization_id = ?1 AND id = ?2",
    )?;
    for block in blocks {
        update.execute(params![organization, block.to_string(), sequence, moved])?;
    }
    Ok(())
}

/// Remembers a block that is deleted for good, and forgets the removals past `REMOVALS_KEPT`.
pub(crate) fn removed(
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    now: i64,
) -> Result<(), ServerError> {
    let organization = organization.to_string();
    let sequence = next(database, &organization)?;
    database.execute(
        "DELETE FROM block_removals WHERE organization_id = ?1 AND time < ?2",
        params![organization, now - REMOVALS_KEPT],
    )?;
    database.execute(
        "INSERT OR REPLACE INTO block_removals (organization_id, block_id, changed, time)
         VALUES (?1, ?2, ?3, ?4)",
        params![organization, block.to_string(), sequence, now],
    )?;
    Ok(())
}
