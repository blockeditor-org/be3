use be_commit::CommitId;
use be_graph::{Access, BlockGraph, BlockNode, BlockParent, ObjectRefs};
use be_protocol::{AccessEntry, BlockSummary, ErrorCode, HistoryEntry};
use be_store::{Hash, ObjectStore};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{
    ServerError,
    store::{Collected, Identity, ServerStore, encode_access, encode_parent, parse_uuid},
};

pub enum PublishOutcome {
    Published(CommitId),
    Rejected(Option<CommitId>),
}

fn access_for(graph: &BlockGraph, member: bool, account: Uuid, block: Uuid) -> Access {
    if graph.get(block).is_none() {
        return Access::None;
    }
    let granted = graph.access(block, account);
    if member {
        granted.max(Access::Edit)
    } else {
        granted
    }
}

fn effective_access(graph: &BlockGraph, identity: Identity, block: Uuid) -> Access {
    access_for(graph, true, identity.account, block)
}

fn require_edit(graph: &BlockGraph, identity: Identity, block: Uuid) -> Result<(), ServerError> {
    if graph.get(block).is_none() {
        return Err(ServerError::Refused(
            ErrorCode::BlockNotFound,
            format!("block {block} does not exist"),
        ));
    }
    if effective_access(graph, identity, block).can_edit() {
        return Ok(());
    }
    Err(ServerError::Refused(
        ErrorCode::PermissionDenied,
        format!("block {block} is not editable by this account"),
    ))
}

fn summary(graph: &BlockGraph, identity: Identity, block: Uuid) -> Option<BlockSummary> {
    let node = graph.get(block)?;
    Some(BlockSummary {
        id: block,
        content_type: node.content_type,
        author: node.author,
        parent: node.parent,
        head: node.head,
        access: effective_access(graph, identity, block),
    })
}

impl ServerStore {
    pub async fn create_block(
        &self,
        identity: Identity,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
    ) -> Result<BlockSummary, ServerError> {
        self.with_graph(identity.workspace, move |graph, database| {
            if let BlockParent::Block(target) = parent {
                require_edit(graph, identity, target)?;
            }
            graph.insert(block, BlockNode::new(content_type, identity.account, parent))?;
            let (kind, parent_id) = encode_parent(parent);
            database.execute(
                "INSERT INTO blocks (workspace_id, id, content_type, author, parent_kind, parent_id, head)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                params![
                    identity.workspace.to_string(),
                    block.to_string(),
                    content_type.to_string(),
                    identity.account.to_string(),
                    kind,
                    parent_id,
                ],
            )?;
            summary(graph, identity, block).ok_or(ServerError::Corrupt)
        })
        .await
    }

    pub async fn read_block(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<BlockSummary, ServerError> {
        self.with_graph(identity.workspace, move |graph, _| {
            let Some(found) = summary(graph, identity, block) else {
                return Err(ServerError::Refused(
                    ErrorCode::BlockNotFound,
                    format!("block {block} does not exist"),
                ));
            };
            if !found.access.can_view() {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("block {block} is not readable by this account"),
                ));
            }
            Ok(found)
        })
        .await
    }

    pub async fn publish(
        &self,
        identity: Identity,
        block: Uuid,
        commit: CommitId,
        expected: Option<CommitId>,
        chunks: Vec<Hash>,
        time: i64,
        pinned: bool,
        references_added: Vec<Uuid>,
        references_removed: Vec<Uuid>,
    ) -> Result<PublishOutcome, ServerError> {
        let mut held = vec![commit.hash()];
        held.extend(chunks.iter().copied());
        for hash in &held {
            if !self.objects().has(*hash)? {
                return Err(ServerError::Refused(
                    ErrorCode::ObjectNotFound,
                    format!("object {hash} must be uploaded before it can be published"),
                ));
            }
        }
        self.with_graph(identity.workspace, move |graph, database| {
            require_edit(graph, identity, block)?;
            let current = graph.head(block);
            if current != expected {
                return Ok(PublishOutcome::Rejected(current));
            }
            if current == Some(commit) {
                return Ok(PublishOutcome::Published(commit));
            }
            let transaction = database.unchecked_transaction()?;
            let workspace = identity.workspace.to_string();
            let existing: Option<i64> = transaction
                .query_row(
                    "SELECT 1 FROM block_commits
                     WHERE workspace_id = ?1 AND block_id = ?2 AND commit_id = ?3",
                    params![workspace, block.to_string(), commit.hash().to_hex()],
                    |row| row.get(0),
                )
                .optional()?;
            if existing.is_none() {
                transaction.execute(
                    "INSERT INTO block_commits (workspace_id, block_id, commit_id, time, pinned)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        workspace,
                        block.to_string(),
                        commit.hash().to_hex(),
                        time,
                        pinned
                    ],
                )?;
                let mut refs = self.object_refs();
                for (position, hash) in held.iter().enumerate() {
                    transaction.execute(
                        "INSERT INTO commit_objects
                            (workspace_id, block_id, commit_id, position, hash)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            workspace,
                            block.to_string(),
                            commit.hash().to_hex(),
                            i64::try_from(position).map_err(|_| ServerError::Corrupt)?,
                            hash.to_hex()
                        ],
                    )?;
                }
                refs.retain(&held);
                persist_refs(&transaction, &refs, &held)?;
            }
            transaction.execute(
                "UPDATE blocks SET head = ?3 WHERE workspace_id = ?1 AND id = ?2",
                params![workspace, block.to_string(), commit.hash().to_hex()],
            )?;
            for reference in &references_removed {
                transaction.execute(
                    "DELETE FROM block_edges
                     WHERE workspace_id = ?1 AND block_id = ?2 AND reference_id = ?3",
                    params![workspace, block.to_string(), reference.to_string()],
                )?;
            }
            for reference in &references_added {
                transaction.execute(
                    "INSERT OR IGNORE INTO block_edges (workspace_id, block_id, reference_id)
                     VALUES (?1, ?2, ?3)",
                    params![workspace, block.to_string(), reference.to_string()],
                )?;
            }
            transaction.commit()?;
            graph.apply_references(block, &references_added, &references_removed)?;
            graph.set_head(block, commit)?;
            Ok(PublishOutcome::Published(commit))
        })
        .await
    }

    pub async fn set_parent(
        &self,
        identity: Identity,
        block: Uuid,
        parent: BlockParent,
    ) -> Result<(), ServerError> {
        self.with_graph(identity.workspace, move |graph, database| {
            require_edit(graph, identity, block)?;
            if let BlockParent::Block(target) = parent {
                require_edit(graph, identity, target)?;
            }
            graph.set_parent(block, parent)?;
            let (kind, parent_id) = encode_parent(parent);
            database.execute(
                "UPDATE blocks SET parent_kind = ?3, parent_id = ?4
                 WHERE workspace_id = ?1 AND id = ?2",
                params![
                    identity.workspace.to_string(),
                    block.to_string(),
                    kind,
                    parent_id
                ],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn list_children(
        &self,
        identity: Identity,
        parent: BlockParent,
    ) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.workspace, move |graph, _| {
            Ok(graph
                .children(parent)
                .into_iter()
                .filter_map(|block| summary(graph, identity, block))
                .filter(|block| block.access.can_know_exists())
                .collect())
        })
        .await
    }

    pub async fn list_backrefs(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.workspace, move |graph, _| {
            Ok(graph
                .backrefs(block)
                .into_iter()
                .filter_map(|referrer| summary(graph, identity, referrer))
                .filter(|block| block.access.can_know_exists())
                .collect())
        })
        .await
    }

    pub async fn list_history(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<Vec<HistoryEntry>, ServerError> {
        self.with_graph(identity.workspace, move |graph, database| {
            if !effective_access(graph, identity, block).can_view() {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("block {block} is not readable by this account"),
                ));
            }
            read_history(database, identity.workspace, block)
        })
        .await
    }

    pub async fn prune_history(
        &self,
        identity: Identity,
        block: Uuid,
        drop: Vec<CommitId>,
    ) -> Result<usize, ServerError> {
        self.with_graph(identity.workspace, move |graph, database| {
            require_edit(graph, identity, block)?;
            let head = graph.head(block);
            let transaction = database.unchecked_transaction()?;
            let mut refs = self.object_refs();
            let mut freed = 0;
            for commit in drop {
                if head == Some(commit) {
                    continue;
                }
                freed += release_commit(
                    &transaction,
                    &mut refs,
                    self.objects(),
                    identity.workspace,
                    block,
                    commit,
                )?;
            }
            transaction.commit()?;
            Ok(freed)
        })
        .await
    }

    pub async fn delete_block(&self, identity: Identity, block: Uuid) -> Result<(), ServerError> {
        self.set_parent(identity, block, BlockParent::Detached)
            .await
    }

    pub async fn collect_detached(&self, identity: Identity) -> Result<Collected, ServerError> {
        self.with_graph(identity.workspace, move |graph, database| {
            let detached = graph.detached();
            let transaction = database.unchecked_transaction()?;
            let mut refs = self.object_refs();
            let mut freed = 0;
            let mut removed = Vec::new();
            for block in detached {
                if !effective_access(graph, identity, block).can_edit() {
                    continue;
                }
                for commit in read_history(&transaction, identity.workspace, block)? {
                    freed += release_commit(
                        &transaction,
                        &mut refs,
                        self.objects(),
                        identity.workspace,
                        block,
                        commit.commit,
                    )?;
                }
                let workspace = identity.workspace.to_string();
                transaction.execute(
                    "DELETE FROM block_edges WHERE workspace_id = ?1 AND block_id = ?2",
                    params![workspace, block.to_string()],
                )?;
                transaction.execute(
                    "DELETE FROM block_edges WHERE workspace_id = ?1 AND reference_id = ?2",
                    params![workspace, block.to_string()],
                )?;
                transaction.execute(
                    "DELETE FROM block_access WHERE workspace_id = ?1 AND block_id = ?2",
                    params![workspace, block.to_string()],
                )?;
                transaction.execute(
                    "DELETE FROM blocks WHERE workspace_id = ?1 AND id = ?2",
                    params![workspace, block.to_string()],
                )?;
                graph.remove(block);
                removed.push(block);
            }
            transaction.commit()?;
            Ok(Collected {
                blocks: removed,
                objects: freed,
            })
        })
        .await
    }

    pub async fn set_access(
        &self,
        identity: Identity,
        block: Uuid,
        account: Uuid,
        access: Access,
    ) -> Result<(), ServerError> {
        self.with_graph(identity.workspace, move |graph, database| {
            require_edit(graph, identity, block)?;
            graph.grant(block, account, access)?;
            let workspace = identity.workspace.to_string();
            match encode_access(access) {
                None => {
                    database.execute(
                        "DELETE FROM block_access
                         WHERE workspace_id = ?1 AND block_id = ?2 AND account_id = ?3",
                        params![workspace, block.to_string(), account.to_string()],
                    )?;
                }
                Some(encoded) => {
                    database.execute(
                        "INSERT OR REPLACE INTO block_access
                            (workspace_id, block_id, account_id, access)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![workspace, block.to_string(), account.to_string(), encoded],
                    )?;
                }
            }
            Ok(())
        })
        .await
    }

    pub async fn list_access(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<Vec<AccessEntry>, ServerError> {
        self.with_graph(identity.workspace, move |graph, database| {
            require_edit(graph, identity, block)?;
            let node = graph.get(block).ok_or(ServerError::Corrupt)?;
            let mut entries = Vec::new();
            let mut statement = database.prepare(
                "SELECT accounts.id, accounts.email, accounts.display_name FROM memberships
                 JOIN accounts ON accounts.id = memberships.account_id
                 WHERE memberships.workspace_id = ?1",
            )?;
            let rows = statement.query_map([identity.workspace.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (account, email, display_name) = row?;
                let account = parse_uuid(&account)?;
                entries.push(AccessEntry {
                    account,
                    email,
                    display_name,
                    granted: node.grants.get(&account).copied(),
                    effective: access_for(graph, true, account, block),
                });
            }
            Ok(entries)
        })
        .await
    }
}

fn read_history(
    connection: &Connection,
    workspace: Uuid,
    block: Uuid,
) -> Result<Vec<HistoryEntry>, ServerError> {
    let mut statement = connection.prepare(
        "SELECT commit_id, time, pinned FROM block_commits
         WHERE workspace_id = ?1 AND block_id = ?2
         ORDER BY time DESC",
    )?;
    let rows = statement.query_map(params![workspace.to_string(), block.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, bool>(2)?,
        ))
    })?;
    let mut entries = Vec::new();
    for row in rows {
        let (commit, time, pinned) = row?;
        entries.push(HistoryEntry {
            commit: CommitId::from_hash(Hash::from_hex(&commit).ok_or(ServerError::Corrupt)?),
            time,
            pinned,
        });
    }
    Ok(entries)
}

fn release_commit(
    connection: &Connection,
    refs: &mut ObjectRefs,
    objects: &be_store::FileStore,
    workspace: Uuid,
    block: Uuid,
    commit: CommitId,
) -> Result<usize, ServerError> {
    let mut held = Vec::new();
    {
        let mut statement = connection.prepare(
            "SELECT hash FROM commit_objects
             WHERE workspace_id = ?1 AND block_id = ?2 AND commit_id = ?3",
        )?;
        let rows = statement.query_map(
            params![
                workspace.to_string(),
                block.to_string(),
                commit.hash().to_hex()
            ],
            |row| row.get::<_, String>(0),
        )?;
        for row in rows {
            held.push(Hash::from_hex(&row?).ok_or(ServerError::Corrupt)?);
        }
    }
    connection.execute(
        "DELETE FROM commit_objects
         WHERE workspace_id = ?1 AND block_id = ?2 AND commit_id = ?3",
        params![
            workspace.to_string(),
            block.to_string(),
            commit.hash().to_hex()
        ],
    )?;
    connection.execute(
        "DELETE FROM block_commits
         WHERE workspace_id = ?1 AND block_id = ?2 AND commit_id = ?3",
        params![
            workspace.to_string(),
            block.to_string(),
            commit.hash().to_hex()
        ],
    )?;
    let freed = refs.release(&held);
    persist_refs(connection, refs, &held)?;
    for hash in &freed {
        objects.remove(*hash)?;
    }
    Ok(freed.len())
}

fn persist_refs(
    connection: &Connection,
    refs: &ObjectRefs,
    touched: &[Hash],
) -> Result<(), ServerError> {
    for hash in touched {
        let count = refs.count(*hash);
        if count == 0 {
            connection.execute("DELETE FROM object_refs WHERE hash = ?1", [hash.to_hex()])?;
        } else {
            connection.execute(
                "INSERT OR REPLACE INTO object_refs (hash, count) VALUES (?1, ?2)",
                params![hash.to_hex(), i64::from(count)],
            )?;
        }
    }
    Ok(())
}
