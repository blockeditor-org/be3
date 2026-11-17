//! The organization's durable change sequence, which `query.tree` pages through and which
//! orders every summary a client is told (`block-summary.changed`).
//!
//! One strictly increasing counter per organization (`organizations.change_sequence`) is
//! advanced in the transaction of every write that changes what a block holds, where it lies
//! or who may view it (every graph change holds the store's one database mutex, so writes are
//! ordered). A block's `changed` is the sequence of its last such write, kept in the database
//! and in the cached graph alike. A block that leaves the tree of an entry point (`entries_above`:
//! the top of its tree, or a block above it that carries a grant) leaves a row in
//! `block_departures` for that entry, so a reader of one tree, or of a subtree shared with it,
//! is told of each block that left, and of no other. The sequence survives restarts.

use be_block_graph::BlockGraph;
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

/// The organization's sequence now: everything up to it has been written.
pub(crate) fn current(database: &Connection, organization: Uuid) -> Result<u64, ServerError> {
    let sequence: i64 = database.query_row(
        "SELECT change_sequence FROM organizations WHERE id = ?1",
        params![organization.to_string()],
        |row| row.get(0),
    )?;
    Ok(sequence as u64)
}

/// Marks blocks as changed (their content, name, references or who may view them), and
/// answers the sequence they changed at.
pub(crate) fn touch(
    database: &Connection,
    graph: &mut BlockGraph,
    organization: Uuid,
    blocks: &[Uuid],
) -> Result<u64, ServerError> {
    let organization = organization.to_string();
    let sequence = next(database, &organization)?;
    mark(database, graph, &organization, blocks, sequence)?;
    Ok(sequence as u64)
}

fn mark(
    database: &Connection,
    graph: &mut BlockGraph,
    organization: &str,
    blocks: &[Uuid],
    sequence: i64,
) -> Result<(), ServerError> {
    let mut update = database
        .prepare_cached("UPDATE blocks SET changed = ?3 WHERE organization_id = ?1 AND id = ?2")?;
    for block in blocks {
        update.execute(params![organization, block.to_string(), sequence])?;
        if graph.contains(*block) {
            graph.set_changed(*block, sequence as u64)?;
        }
    }
    Ok(())
}

/// The entry points above `block` (not `block` itself) whose trees it lies in: the top of its
/// tree (a top-level block, or the block at the top of what lies in the trash), and each block
/// on the way that carries a grant or opens to the blocks inside its owner, which are the only
/// blocks below the top where someone's view can start. Bounded by the grants on one path, not
/// its depth.
pub(crate) fn entries_above(graph: &BlockGraph, block: Uuid) -> Vec<Uuid> {
    let above: Vec<Uuid> = graph.ancestors(block).collect();
    let Some(top) = above.last().copied() else {
        return Vec::new();
    };
    let mut entries: Vec<Uuid> = above
        .iter()
        .copied()
        .filter(|ancestor| {
            *ancestor == top
                || graph.get(*ancestor).is_some_and(|node| {
                    node.is_open_inside() || node.grants.values().any(|level| level.can_view())
                })
        })
        .collect();
    entries.dedup();
    entries
}

/// Marks blocks as moved: they and everything below them may lie in other trees now, and they
/// left the entry points of `left` (those above them before the move and not after).
pub(crate) fn touch_moved(
    database: &Connection,
    graph: &mut BlockGraph,
    organization: Uuid,
    blocks: &[Departing],
    left: &[Uuid],
    now: i64,
) -> Result<u64, ServerError> {
    let organization = organization.to_string();
    let sequence = next(database, &organization)?;
    let ids: Vec<Uuid> = blocks.iter().map(|departing| departing.block).collect();
    mark(database, graph, &organization, &ids, sequence)?;
    departed(database, &organization, left, blocks, sequence, now)?;
    Ok(sequence as u64)
}

/// A block that leaves a tree, and the account that alone may read it if it lies in an
/// owner-only block (read before the move, while it still does).
#[derive(Clone, Copy)]
pub(crate) struct Departing {
    pub block: Uuid,
    pub owner: Option<Uuid>,
}

/// Remembers that `blocks` left the trees of `entries`, and forgets the departures past
/// `DEPARTURES_KEPT`.
fn departed(
    database: &Connection,
    organization: &str,
    entries: &[Uuid],
    blocks: &[Departing],
    sequence: i64,
    now: i64,
) -> Result<(), ServerError> {
    if entries.is_empty() {
        return Ok(());
    }
    database.execute(
        "DELETE FROM block_departures WHERE organization_id = ?1 AND time < ?2",
        params![organization, now - DEPARTURES_KEPT],
    )?;
    // A block in an owner-only block is named only to the account that may read it.
    let mut insert = database.prepare_cached(
        "INSERT OR REPLACE INTO block_departures
             (organization_id, entry, block_id, changed, time, owner)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for entry in entries {
        for departing in blocks {
            insert.execute(params![
                organization,
                entry.to_string(),
                departing.block.to_string(),
                sequence,
                now,
                departing.owner.map(|owner| owner.to_string()),
            ])?;
        }
    }
    Ok(())
}
