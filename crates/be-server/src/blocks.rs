use be_commit::CommitId;
use be_graph::{
    ANYONE_INSIDE, Access, BlockGraph, BlockNode, BlockParent, ObjectRefs, SETTINGS_TYPE,
};
use be_protocol::{
    AccessEntry, AccessList, BlockSummary, ErrorCode, HistoryEntry, KeyKind, KeySet,
    OrganizationRole,
};
use be_store::{Hash, ObjectStore};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use uuid::Uuid;

use crate::{
    ServerError,
    keys::{forget_block_keys, store_keys},
    store::{Collected, Identity, ServerStore, encode_access, encode_parent, parse_uuid},
};

pub enum PublishOutcome {
    Published(CommitId, Option<Vec<Uuid>>),
    Rejected(Option<CommitId>),
}

pub(crate) struct Visibility {
    account: Uuid,
    administrator: bool,
    visible: std::cell::OnceCell<BTreeMap<Uuid, Access>>,
}

impl Visibility {
    fn of(account: Uuid, role: OrganizationRole) -> Self {
        Self {
            account,
            administrator: role == OrganizationRole::Administrator,
            visible: std::cell::OnceCell::new(),
        }
    }

    fn inherited(&self, graph: &BlockGraph, block: Uuid) -> Access {
        match (graph.contains(block), self.administrator) {
            (false, _) => Access::None,
            (true, true) => Access::Manage,
            (true, false) => graph.inherited(block, self.account),
        }
    }

    pub(crate) fn can_view(&self, graph: &BlockGraph, block: Uuid) -> bool {
        self.inherited(graph, block).can_view()
    }

    pub(crate) fn can_edit(&self, graph: &BlockGraph, block: Uuid) -> bool {
        self.inherited(graph, block).can_edit()
    }

    pub(crate) fn can_manage(&self, graph: &BlockGraph, block: Uuid) -> bool {
        self.inherited(graph, block).can_manage()
    }

    pub(crate) fn administrator(&self) -> bool {
        self.administrator
    }

    fn access(&self, graph: &BlockGraph, block: Uuid) -> Access {
        let inherited = self.inherited(graph, block);
        if inherited.can_view() || !graph.contains(block) {
            return inherited;
        }
        self.visible
            .get_or_init(|| graph.visible(self.account))
            .get(&block)
            .copied()
            .unwrap_or_default()
    }
}

pub(crate) fn visibility(identity: Identity) -> Visibility {
    Visibility::of(identity.account, identity.role)
}

fn require_edit(graph: &BlockGraph, seen: &Visibility, block: Uuid) -> Result<(), ServerError> {
    require(graph, seen, block, Access::Edit, "editable")
}

fn require_manage(graph: &BlockGraph, seen: &Visibility, block: Uuid) -> Result<(), ServerError> {
    require(graph, seen, block, Access::Manage, "managed")
}

fn require(
    graph: &BlockGraph,
    seen: &Visibility,
    block: Uuid,
    level: Access,
    verb: &str,
) -> Result<(), ServerError> {
    if graph.get(block).is_none() {
        return Err(ServerError::Refused(
            ErrorCode::BlockNotFound,
            format!("block {block} does not exist"),
        ));
    }
    if seen.inherited(graph, block) >= level {
        return Ok(());
    }
    Err(ServerError::Refused(
        ErrorCode::PermissionDenied,
        format!("block {block} is not {verb} by this account"),
    ))
}

fn require_parent_of(
    graph: &BlockGraph,
    seen: &Visibility,
    content_type: Uuid,
    parent: Uuid,
) -> Result<(), ServerError> {
    match content_type == SETTINGS_TYPE {
        true => require_manage(graph, seen, parent),
        false => require_edit(graph, seen, parent),
    }
}

fn require_keeping_settings(
    graph: &BlockGraph,
    seen: &Visibility,
    block: Uuid,
    references: &[Uuid],
) -> Result<(), ServerError> {
    let dropped = graph.settings_of(block).into_iter().any(|settings| {
        graph.references(block).contains(&settings) && !references.contains(&settings)
    });
    match dropped {
        true => require_manage(graph, seen, block),
        false => Ok(()),
    }
}

fn summary(graph: &BlockGraph, identity: Identity, block: Uuid) -> Option<BlockSummary> {
    summary_with(graph, &visibility(identity), block)
}

fn summary_with(graph: &BlockGraph, seen: &Visibility, block: Uuid) -> Option<BlockSummary> {
    let node = graph.get(block)?;
    let access = seen.access(graph, block);
    let revision = graph.revision();
    if !access.can_view() {
        return Some(BlockSummary {
            revision,
            ..BlockSummary::no_access(block, node.content_type, access)
        });
    }
    Some(BlockSummary {
        id: block,
        content_type: node.content_type,
        access,
        author: Some(node.author),
        parent: Some(node.parent),
        head: node.head,
        references: node.references.clone(),
        metadata: node.metadata.clone(),
        version: Some(node.version),
        key: node.key,
        scope: Some(graph.scope(block)),
        revision,
        held: graph.holds(block),
        opens_inside: node.is_open_inside() && graph.settings_owner(block).is_some(),
        sets_access: node.sets_access(),
    })
}

impl ServerStore {
    #[allow(clippy::too_many_arguments)]
    pub async fn create_block(
        &self,
        identity: Identity,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        metadata: Vec<u8>,
        references: Vec<Uuid>,
        key: Uuid,
        keys: KeySet,
    ) -> Result<BlockSummary, ServerError> {
        let references = distinct(references);
        self.with_graph(identity.organization, move |graph, database| {
            if let BlockParent::Block(target) = parent {
                require_parent_of(graph, &visibility(identity), content_type, target)?;
            }
            if !keys
                .keys
                .iter()
                .any(|row| row.id == key && row.block == block && row.kind == KeyKind::Data)
            {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "a new block needs a data key of its own".into(),
                ));
            }
            let mut node = BlockNode::new(content_type, identity.account, parent);
            node.metadata.clone_from(&metadata);
            graph.insert(block, node)?;
            let (kind, parent_id) = encode_parent(parent);
            let transaction = database.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO blocks
                    (organization_id, id, content_type, author, parent_kind, parent_id, head, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
                params![
                    identity.organization.to_string(),
                    block.to_string(),
                    content_type.to_string(),
                    identity.account.to_string(),
                    kind,
                    parent_id,
                    metadata,
                ],
            )?;
            store_edges(
                &transaction,
                &identity.organization.to_string(),
                block,
                &references,
            )?;
            store_keys(
                graph,
                &transaction,
                identity.organization,
                identity.account,
                &keys,
                |_, owner| owner == block,
            )?;
            transaction.commit()?;
            graph.set_references(block, &references)?;
            summary(graph, identity, block).ok_or(ServerError::Corrupt)
        })
        .await
    }

    pub async fn set_metadata(
        &self,
        identity: Identity,
        block: Uuid,
        metadata: Vec<u8>,
        references: Vec<Uuid>,
    ) -> Result<(BlockSummary, Vec<Uuid>), ServerError> {
        let references = distinct(references);
        self.with_graph(identity.organization, move |graph, database| {
            require_edit(graph, &visibility(identity), block)?;
            if let Some(head) = graph.head(block) {
                return Err(ServerError::Refused(
                    ErrorCode::BlockHasContent,
                    format!("block {block} has content at {head}, so its metadata is published with it"),
                ));
            }
            require_keeping_settings(graph, &visibility(identity), block, &references)?;
            let relinked = relinked(graph, block, &references);
            let organization = identity.organization.to_string();
            let transaction = database.unchecked_transaction()?;
            transaction.execute(
                "UPDATE blocks SET metadata = ?3, version = version + 1 WHERE organization_id = ?1 AND id = ?2",
                params![organization, block.to_string(), metadata],
            )?;
            if relinked.is_some() {
                store_edges(&transaction, &organization, block, &references)?;
            }
            transaction.commit()?;
            graph.set_references(block, &references)?;
            graph.set_metadata(block, metadata)?;
            graph.touch(block)?;
            let summary = summary(graph, identity, block).ok_or(ServerError::Corrupt)?;
            Ok((summary, relinked.unwrap_or_default()))
        })
        .await
    }

    pub async fn graph_views(
        &self,
        organization: Uuid,
        members: Vec<Identity>,
        blocks: Vec<Uuid>,
        subtree: bool,
    ) -> Result<Vec<Vec<(be_protocol::ServerMessage, bool)>>, ServerError> {
        self.with_graph(organization, move |graph, _| {
            let mut touched = Vec::new();
            for block in blocks {
                match subtree {
                    true => touched.extend(graph.subtree(block)),
                    false => touched.push(block),
                }
            }
            let referenced: Vec<Uuid> = touched
                .iter()
                .flat_map(|block| graph.references(*block))
                .collect();
            touched.extend(referenced);
            touched.retain(|block| graph.contains(*block));
            touched.sort_unstable();
            touched.dedup();
            let revision = graph.revision();
            Ok(members
                .into_iter()
                .map(|identity| {
                    let seen = visibility(identity);
                    let shared: HashSet<Uuid> = match seen.administrator {
                        true => HashSet::new(),
                        false => graph.shared(identity.account).into_iter().collect(),
                    };
                    touched
                        .iter()
                        .map(|block| match summary_with(graph, &seen, *block) {
                            Some(found) if found.access.can_know_exists() => {
                                let entry = found.access.can_view() && shared.contains(block);
                                (
                                    be_protocol::ServerMessage::BlockChanged { block: found },
                                    entry,
                                )
                            }
                            _ => (
                                be_protocol::ServerMessage::BlockRemoved {
                                    block: *block,
                                    revision,
                                },
                                false,
                            ),
                        })
                        .collect()
                })
                .collect())
        })
        .await
    }

    pub async fn audience(
        &self,
        organization: Uuid,
        blocks: Vec<Uuid>,
    ) -> Result<BTreeSet<Uuid>, ServerError> {
        self.with_graph(organization, move |graph, _| {
            Ok(blocks
                .into_iter()
                .flat_map(|block| graph.audience(block))
                .collect())
        })
        .await
    }

    pub async fn read_block(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<BlockSummary, ServerError> {
        self.with_graph(identity.organization, move |graph, _| {
            if !graph.contains(block) {
                return Err(ServerError::Refused(
                    ErrorCode::BlockNotFound,
                    format!("block {block} does not exist"),
                ));
            }
            let seen = visibility(identity);
            if !seen.can_view(graph, block) {
                let access = graph.effective_access(block, identity.account);
                let node = graph.get(block).ok_or(ServerError::Corrupt)?;
                return match access.can_know_exists() {
                    true => Ok(BlockSummary {
                        revision: graph.revision(),
                        ..BlockSummary::no_access(block, node.content_type, access)
                    }),
                    false => Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("block {block} is not readable by this account"),
                    )),
                };
            }
            summary_with(graph, &seen, block).ok_or(ServerError::Corrupt)
        })
        .await
    }

    pub async fn read_contents(
        &self,
        identity: Identity,
        blocks: Vec<Uuid>,
        held: Vec<CommitId>,
    ) -> Result<(Vec<BlockSummary>, Vec<Hash>), ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let seen = visibility(identity);
            let held: BTreeSet<CommitId> = held.into_iter().collect();
            let mut statement = database.prepare(
                "SELECT hash FROM commit_objects
                 WHERE organization_id = ?1 AND block_id = ?2 AND commit_id = ?3
                 ORDER BY position",
            )?;
            let mut summaries = Vec::with_capacity(blocks.len());
            let mut objects = Vec::new();
            for block in blocks {
                if !graph.contains(block) {
                    return Err(ServerError::Refused(
                        ErrorCode::BlockNotFound,
                        format!("block {block} does not exist"),
                    ));
                }
                if !seen.can_view(graph, block) {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("block {block} is not readable by this account"),
                    ));
                }
                let summary = summary_with(graph, &seen, block).ok_or(ServerError::Corrupt)?;
                if let Some(head) = summary.head
                    && !held.contains(&head)
                {
                    let rows = statement.query_map(
                        params![
                            identity.organization.to_string(),
                            block.to_string(),
                            head.hash().to_hex()
                        ],
                        |row| row.get::<_, String>(0),
                    )?;
                    for row in rows {
                        objects.push(Hash::from_hex(&row?).ok_or(ServerError::Corrupt)?);
                    }
                }
                summaries.push(summary);
            }
            Ok((summaries, objects))
        })
        .await
    }

    pub async fn publish(
        &self,
        identity: Identity,
        block: Uuid,
        commit: CommitId,
        expected: Option<CommitId>,
        parents: Vec<CommitId>,
        chunks: Vec<Hash>,
        time: i64,
        pinned: bool,
        references: Vec<Uuid>,
        metadata: Vec<u8>,
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
        self.with_graph(identity.organization, move |graph, database| {
            require_edit(graph, &visibility(identity), block)?;
            let current = graph.head(block);
            if current != expected {
                return Ok(PublishOutcome::Rejected(current));
            }
            if current == Some(commit) {
                return Ok(PublishOutcome::Published(commit, None));
            }
            let references = distinct(references);
            require_keeping_settings(graph, &visibility(identity), block, &references)?;
            let relinked = relinked(graph, block, &references);
            let redescribed = (relinked.is_some()
                || graph
                    .get(block)
                    .is_some_and(|node| node.metadata != metadata))
            .then(|| relinked.clone().unwrap_or_default());
            let transaction = database.unchecked_transaction()?;
            let organization = identity.organization.to_string();
            let existing: Option<i64> = transaction
                .query_row(
                    "SELECT 1 FROM block_commits
                     WHERE organization_id = ?1 AND block_id = ?2 AND commit_id = ?3",
                    params![organization, block.to_string(), commit.hash().to_hex()],
                    |row| row.get(0),
                )
                .optional()?;
            if existing.is_none() {
                transaction.execute(
                    "INSERT INTO block_commits
                        (organization_id, block_id, commit_id, parents, time, pinned)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        organization,
                        block.to_string(),
                        commit.hash().to_hex(),
                        encode_parents(&parents),
                        time,
                        pinned
                    ],
                )?;
                let mut refs = self.object_refs();
                for (position, hash) in held.iter().enumerate() {
                    transaction.execute(
                        "INSERT INTO commit_objects
                            (organization_id, block_id, commit_id, position, hash)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            organization,
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
                "UPDATE blocks SET head = ?3, metadata = ?4, version = version + 1
                 WHERE organization_id = ?1 AND id = ?2",
                params![
                    organization,
                    block.to_string(),
                    commit.hash().to_hex(),
                    metadata
                ],
            )?;
            if relinked.is_some() {
                store_edges(&transaction, &organization, block, &references)?;
            }
            transaction.commit()?;
            graph.set_references(block, &references)?;
            graph.set_head(block, commit)?;
            graph.set_metadata(block, metadata)?;
            graph.touch(block)?;
            Ok(PublishOutcome::Published(commit, redescribed))
        })
        .await
    }

    pub async fn may_read_object(
        &self,
        identity: Identity,
        hash: Hash,
    ) -> Result<bool, ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let mut statement = database.prepare(
                "SELECT block_id FROM commit_objects WHERE organization_id = ?1 AND hash = ?2
                 UNION
                 SELECT block_id FROM block_objects WHERE organization_id = ?1 AND hash = ?2",
            )?;
            let rows = statement.query_map(
                params![identity.organization.to_string(), hash.to_hex()],
                |row| row.get::<_, String>(0),
            )?;
            let seen = visibility(identity);
            for row in rows {
                if seen.can_view(graph, parse_uuid(&row?)?) {
                    return Ok(true);
                }
            }
            Ok(false)
        })
        .await
    }

    pub async fn hold_objects(
        &self,
        identity: Identity,
        block: Uuid,
        objects: Vec<Hash>,
    ) -> Result<(), ServerError> {
        for hash in &objects {
            if !self.objects().has(*hash)? {
                return Err(ServerError::Refused(
                    ErrorCode::ObjectNotFound,
                    format!("object {hash} must be uploaded before it can be held"),
                ));
            }
        }
        self.with_graph(identity.organization, move |graph, database| {
            require_edit(graph, &visibility(identity), block)?;
            let transaction = database.unchecked_transaction()?;
            let organization = identity.organization.to_string();
            let mut fresh = Vec::new();
            for hash in objects {
                let inserted = transaction.execute(
                    "INSERT OR IGNORE INTO block_objects (organization_id, block_id, hash)
                     VALUES (?1, ?2, ?3)",
                    params![organization, block.to_string(), hash.to_hex()],
                )?;
                if inserted > 0 {
                    fresh.push(hash);
                }
            }
            let mut refs = self.object_refs();
            refs.retain(&fresh);
            persist_refs(&transaction, &refs, &fresh)?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    pub async fn set_parent(
        &self,
        identity: Identity,
        block: Uuid,
        parent: BlockParent,
        keys: KeySet,
    ) -> Result<BTreeSet<Uuid>, ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let seen = visibility(identity);
            require_edit(graph, &seen, block)?;
            if let BlockParent::Block(target) = parent {
                let content_type = graph.get(block).ok_or(ServerError::Corrupt)?.content_type;
                require_parent_of(graph, &seen, content_type, target)?;
            }
            let before = graph.audience(block);
            let transaction = database.unchecked_transaction()?;
            store_keys(
                graph,
                &transaction,
                identity.organization,
                identity.account,
                &keys,
                |graph, owner| {
                    (owner == block || graph.ancestors(owner).any(|above| above == block))
                        && seen.can_view(graph, owner)
                },
            )?;
            graph.set_parent(block, parent)?;
            graph.touch(block)?;
            let (kind, parent_id) = encode_parent(parent);
            transaction.execute(
                "UPDATE blocks SET parent_kind = ?3, parent_id = ?4, version = version + 1
                 WHERE organization_id = ?1 AND id = ?2",
                params![
                    identity.organization.to_string(),
                    block.to_string(),
                    kind,
                    parent_id
                ],
            )?;
            transaction.commit()?;
            Ok(before)
        })
        .await
    }

    pub async fn list_shared(&self, identity: Identity) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.organization, move |graph, _| {
            let seen = visibility(identity);
            if seen.administrator {
                return Ok(Vec::new());
            }
            Ok(graph
                .shared(identity.account)
                .into_iter()
                .filter_map(|block| summary_with(graph, &seen, block))
                .filter(|block| block.access.can_view())
                .collect())
        })
        .await
    }

    pub async fn list_children(
        &self,
        identity: Identity,
        parent: BlockParent,
    ) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.organization, move |graph, _| {
            let seen = visibility(identity);
            Ok(graph
                .children(parent)
                .into_iter()
                .filter_map(|block| summary_with(graph, &seen, block))
                .filter(|block| block.access.can_know_exists())
                .collect())
        })
        .await
    }

    pub async fn list_deleted(&self, identity: Identity) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.organization, move |graph, _| {
            let seen = visibility(identity);
            Ok(graph
                .deleted_roots()
                .into_iter()
                .filter_map(|block| summary_with(graph, &seen, block))
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
        self.with_graph(identity.organization, move |graph, _| {
            let seen = visibility(identity);
            Ok(graph
                .backrefs(block)
                .into_iter()
                .filter_map(|referrer| summary_with(graph, &seen, referrer))
                .filter(|block| block.access.can_know_exists())
                .collect())
        })
        .await
    }

    pub async fn list_open_settings(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.organization, move |graph, _| {
            let seen = visibility(identity);
            let mut found: BTreeMap<Uuid, BlockSummary> = BTreeMap::new();
            for owner in graph.ancestors(block) {
                if seen.can_view(graph, owner) || !graph.is_inside(owner, block) {
                    continue;
                }
                for kind in graph.kinds_of(owner) {
                    let Some(node) = graph.get(kind) else {
                        continue;
                    };
                    if !node.is_open_inside() || found.contains_key(&node.content_type) {
                        continue;
                    }
                    if let Some(summary) = summary_with(graph, &seen, kind)
                        .filter(|summary| summary.access.can_view())
                    {
                        found.insert(node.content_type, summary);
                    }
                }
            }
            Ok(found.into_values().collect())
        })
        .await
    }

    pub async fn list_references(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.organization, move |graph, _| {
            let seen = visibility(identity);
            if !seen.can_view(graph, block) {
                return Ok(Vec::new());
            }
            Ok(graph
                .references(block)
                .into_iter()
                .filter_map(|reference| summary_with(graph, &seen, reference))
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
        self.with_graph(identity.organization, move |graph, database| {
            if !visibility(identity).can_view(graph, block) {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("block {block} is not readable by this account"),
                ));
            }
            read_history(database, identity.organization, block)
        })
        .await
    }

    pub async fn prune_history(
        &self,
        identity: Identity,
        block: Uuid,
        drop: Vec<CommitId>,
    ) -> Result<usize, ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            require_edit(graph, &visibility(identity), block)?;
            let head = graph.head(block);
            let transaction = database.unchecked_transaction()?;
            let pinned: Vec<CommitId> = read_history(&transaction, identity.organization, block)?
                .into_iter()
                .filter(|entry| entry.pinned)
                .map(|entry| entry.commit)
                .collect();
            let mut refs = self.object_refs();
            let mut freed = Vec::new();
            for commit in drop {
                if head == Some(commit) || pinned.contains(&commit) {
                    continue;
                }
                freed.extend(release_commit(
                    &transaction,
                    &mut refs,
                    identity.organization,
                    block,
                    commit,
                )?);
            }
            transaction.commit()?;
            remove_objects(self.objects(), &freed)?;
            Ok(freed.len())
        })
        .await
    }

    pub async fn delete_block(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<BTreeSet<Uuid>, ServerError> {
        self.set_parent(identity, block, BlockParent::Detached, KeySet::default())
            .await
    }

    pub async fn collect_detached(&self, identity: Identity) -> Result<Collected, ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let seen = visibility(identity);
            let detached: Vec<Uuid> = graph
                .detached()
                .into_iter()
                .filter(|block| seen.inherited(graph, *block).can_edit())
                .collect();
            let transaction = database.unchecked_transaction()?;
            let mut refs = self.object_refs();
            let mut freed = Vec::new();
            let mut removed = Vec::new();
            for block in detached {
                for commit in read_history(&transaction, identity.organization, block)? {
                    freed.extend(release_commit(
                        &transaction,
                        &mut refs,
                        identity.organization,
                        block,
                        commit.commit,
                    )?);
                }
                freed.extend(release_held(
                    &transaction,
                    &mut refs,
                    identity.organization,
                    block,
                )?);
                let organization = identity.organization.to_string();
                transaction.execute(
                    "DELETE FROM block_edges WHERE organization_id = ?1 AND block_id = ?2",
                    params![organization, block.to_string()],
                )?;
                transaction.execute(
                    "DELETE FROM block_edges WHERE organization_id = ?1 AND reference_id = ?2",
                    params![organization, block.to_string()],
                )?;
                transaction.execute(
                    "DELETE FROM block_access WHERE organization_id = ?1 AND block_id = ?2",
                    params![organization, block.to_string()],
                )?;
                transaction.execute(
                    "DELETE FROM blocks WHERE organization_id = ?1 AND id = ?2",
                    params![organization, block.to_string()],
                )?;
                forget_block_keys(&transaction, identity.organization, block)?;
                graph.remove(block);
                removed.push(block);
            }
            transaction.commit()?;
            remove_objects(self.objects(), &freed)?;
            Ok(Collected {
                blocks: removed,
                objects: freed.len(),
                revision: graph.revision(),
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
    ) -> Result<BTreeSet<Uuid>, ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            let leaving = account == identity.account && access == Access::None;
            match leaving {
                true if !graph.contains(block) => {
                    return Err(ServerError::Refused(
                        ErrorCode::BlockNotFound,
                        format!("block {block} does not exist"),
                    ));
                }
                true => {}
                false if account == ANYONE_INSIDE && graph.get(block).is_some_and(BlockNode::is_edit_kind) => {
                    require_edit(graph, &visibility(identity), block)?;
                }
                false => require_manage(graph, &visibility(identity), block)?,
            }
            if account == ANYONE_INSIDE {
                if graph.settings_owner(block).is_none() || access > Access::View {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "only a kind of settings opens to anyone who can view a block inside, and only to view".into(),
                    ));
                }
            } else if access > Access::None && account != identity.account {
                if !crate::store::is_member_of(database, account, identity.organization)? {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        "that account is not a member of this organization".into(),
                    ));
                }
                if !crate::contacts::is_contact(database, identity.account, account)? {
                    return Err(ServerError::Refused(
                        ErrorCode::NotAContact,
                        "share only with your contacts: send them a contact link first".into(),
                    ));
                }
            }
            let before = graph.audience(block);
            let transaction = database.unchecked_transaction()?;
            revoke_or_grant(
                graph,
                &transaction,
                identity.organization,
                block,
                account,
                access,
            )?;
            transaction.commit()?;
            Ok(before)
        })
        .await
    }

    pub async fn list_access(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<AccessList, ServerError> {
        self.with_graph(identity.organization, move |graph, database| {
            require_manage(graph, &visibility(identity), block)?;
            let company = crate::keys::organization_kind(database, identity.organization)?
                == be_protocol::OrganizationKind::Company;
            let node = graph.get(block).ok_or(ServerError::Corrupt)?;
            let contacts = crate::contacts::contacts_of(database, identity.account)?;
            let mut listed: BTreeSet<Uuid> = node.grants.keys().copied().collect();
            listed.insert(node.author);
            listed.insert(identity.account);
            listed.extend(
                graph
                    .audience(block)
                    .into_iter()
                    .filter(|account| graph.effective_access(block, *account).can_view()),
            );
            listed.extend(contacts.iter().copied());
            let mut statement = database.prepare_cached(
                "SELECT CASE WHEN accounts.local_profile THEN '' ELSE accounts.email END, accounts.display_name, memberships.role
                 FROM memberships
                 JOIN accounts ON accounts.id = memberships.account_id
                 WHERE memberships.organization_id = ?1 AND memberships.account_id = ?2",
            )?;
            let mut entries = Vec::new();
            for account in listed {
                let row = statement
                    .query_row(
                        params![identity.organization.to_string(), account.to_string()],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                            ))
                        },
                    )
                    .optional()?;
                if account == ANYONE_INSIDE {
                    entries.push(AccessEntry {
                        account,
                        email: String::new(),
                        display_name: "Anyone who can view a block inside".into(),
                        role: OrganizationRole::Editor,
                        author: false,
                        granted: node.grants.get(&account).copied(),
                        effective: node.grants.get(&account).copied().unwrap_or_default(),
                        source: node.grants.contains_key(&account).then_some(block),
                        contact: false,
                        waiting: false,
                    });
                    continue;
                }
                let Some((email, display_name, role)) = row else {
                    continue;
                };
                let role = crate::store::decode_role(&role)?;
                let (effective, source) = match role {
                    OrganizationRole::Administrator => (Access::Manage, None),
                    OrganizationRole::Editor => match graph.inherited_from(block, account) {
                        (level, source) if level.can_know_exists() => (level, source),
                        _ => (graph.effective_access(block, account), None),
                    },
                };
                entries.push(AccessEntry {
                    account,
                    email,
                    display_name,
                    role,
                    author: node.author == account,
                    granted: node.grants.get(&account).copied(),
                    effective,
                    source,
                    contact: contacts.contains(&account),
                    waiting: node
                        .grants
                        .get(&account)
                        .is_some_and(|level| level.can_view())
                        && crate::keys::waiting_for_key(
                            database,
                            identity.organization,
                            block,
                            account,
                        )?,
                });
            }
            Ok(AccessList {
                entries,
                company,
                set_inside: u32::try_from(graph.set_inside(block)).unwrap_or(u32::MAX),
            })
        })
        .await
    }
}

pub(crate) fn revoke_or_grant(
    graph: &mut BlockGraph,
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    account: Uuid,
    access: Access,
) -> Result<(), ServerError> {
    let viewed = crate::keys::is_viewer(database, graph, organization, account, block)?;
    write_grant(graph, database, organization, block, account, access)?;
    if viewed && !crate::keys::is_viewer(database, graph, organization, account, block)? {
        crate::keys::lost_view(database, organization, block, account)?;
    }
    Ok(())
}

pub(crate) fn write_grant(
    graph: &mut BlockGraph,
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    account: Uuid,
    access: Access,
) -> Result<(), ServerError> {
    graph.grant(block, account, access)?;
    graph.touch(block)?;
    let organization = organization.to_string();
    database.execute(
        "UPDATE blocks SET version = version + 1 WHERE organization_id = ?1 AND id = ?2",
        params![organization, block.to_string()],
    )?;
    match encode_access(access) {
        None => {
            database.execute(
                "DELETE FROM block_access
                 WHERE organization_id = ?1 AND block_id = ?2 AND account_id = ?3",
                params![organization, block.to_string(), account.to_string()],
            )?;
        }
        Some(encoded) => {
            database.execute(
                "INSERT OR REPLACE INTO block_access
                    (organization_id, block_id, account_id, access)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    organization,
                    block.to_string(),
                    account.to_string(),
                    encoded
                ],
            )?;
        }
    }
    Ok(())
}

fn read_history(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Vec<HistoryEntry>, ServerError> {
    let mut statement = connection.prepare(
        "SELECT commit_id, parents, time, pinned FROM block_commits
         WHERE organization_id = ?1 AND block_id = ?2
         ORDER BY time DESC",
    )?;
    let rows = statement.query_map(
        params![organization.to_string(), block.to_string()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, bool>(3)?,
            ))
        },
    )?;
    let mut entries = Vec::new();
    for row in rows {
        let (commit, parents, time, pinned) = row?;
        entries.push(HistoryEntry {
            commit: parse_commit(&commit)?,
            parents: decode_parents(&parents)?,
            time,
            pinned,
        });
    }
    Ok(entries)
}

fn release_commit(
    connection: &Connection,
    refs: &mut ObjectRefs,
    organization: Uuid,
    block: Uuid,
    commit: CommitId,
) -> Result<Vec<Hash>, ServerError> {
    let mut held = Vec::new();
    {
        let mut statement = connection.prepare(
            "SELECT hash FROM commit_objects
             WHERE organization_id = ?1 AND block_id = ?2 AND commit_id = ?3",
        )?;
        let rows = statement.query_map(
            params![
                organization.to_string(),
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
         WHERE organization_id = ?1 AND block_id = ?2 AND commit_id = ?3",
        params![
            organization.to_string(),
            block.to_string(),
            commit.hash().to_hex()
        ],
    )?;
    connection.execute(
        "DELETE FROM block_commits
         WHERE organization_id = ?1 AND block_id = ?2 AND commit_id = ?3",
        params![
            organization.to_string(),
            block.to_string(),
            commit.hash().to_hex()
        ],
    )?;
    let freed = refs.release(&held);
    persist_refs(connection, refs, &held)?;
    Ok(freed)
}

fn release_held(
    connection: &Connection,
    refs: &mut ObjectRefs,
    organization: Uuid,
    block: Uuid,
) -> Result<Vec<Hash>, ServerError> {
    let mut held = Vec::new();
    {
        let mut statement = connection.prepare(
            "SELECT hash FROM block_objects WHERE organization_id = ?1 AND block_id = ?2",
        )?;
        let rows = statement.query_map(
            params![organization.to_string(), block.to_string()],
            |row| row.get::<_, String>(0),
        )?;
        for row in rows {
            held.push(Hash::from_hex(&row?).ok_or(ServerError::Corrupt)?);
        }
    }
    connection.execute(
        "DELETE FROM block_objects WHERE organization_id = ?1 AND block_id = ?2",
        params![organization.to_string(), block.to_string()],
    )?;
    let freed = refs.release(&held);
    persist_refs(connection, refs, &held)?;
    Ok(freed)
}

fn remove_objects(objects: &be_store::FileStore, freed: &[Hash]) -> Result<(), ServerError> {
    for hash in freed {
        objects.remove(*hash)?;
    }
    Ok(())
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

fn parse_commit(hex: &str) -> Result<CommitId, ServerError> {
    Hash::from_hex(hex)
        .map(CommitId::from_hash)
        .ok_or(ServerError::Corrupt)
}

fn encode_parents(parents: &[CommitId]) -> String {
    parents
        .iter()
        .map(|parent| parent.hash().to_hex())
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_parents(parents: &str) -> Result<Vec<CommitId>, ServerError> {
    parents.split_whitespace().map(parse_commit).collect()
}

fn distinct(mut references: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    references.retain(|reference| seen.insert(*reference));
    references
}

fn relinked(graph: &BlockGraph, block: Uuid, references: &[Uuid]) -> Option<Vec<Uuid>> {
    let before = graph.references(block);
    if before == references {
        return None;
    }
    let added = references
        .iter()
        .filter(|reference| !before.contains(reference))
        .copied()
        .collect::<Vec<_>>();
    Some(
        before
            .into_iter()
            .filter(|reference| !references.contains(reference))
            .chain(added)
            .collect(),
    )
}

fn store_edges(
    transaction: &rusqlite::Transaction<'_>,
    organization: &str,
    block: Uuid,
    references: &[Uuid],
) -> Result<(), ServerError> {
    transaction.execute(
        "DELETE FROM block_edges WHERE organization_id = ?1 AND block_id = ?2",
        params![organization, block.to_string()],
    )?;
    for (position, reference) in references.iter().enumerate() {
        transaction.execute(
            "INSERT OR IGNORE INTO block_edges
                (organization_id, block_id, reference_id, position)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                organization,
                block.to_string(),
                reference.to_string(),
                i64::try_from(position).map_err(|_| ServerError::Corrupt)?
            ],
        )?;
    }
    Ok(())
}
