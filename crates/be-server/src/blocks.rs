use be_block_commit::CommitId;
use be_block_graph::{
    ANYONE_INSIDE, Access, BlockGraph, BlockNode, BlockParent, ObjectRefs, SEARCH_INDEX_TYPE,
    SETTINGS_TYPE,
};
use be_block_protocol::{
    AccessEntry, AccessList, BlockSummary, ErrorCode, HistoryEntry, Hold, KeyKind, KeySet,
    ObjectChange, OrganizationRole, ReferenceChange,
};
use be_block_store::Hash;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use uuid::Uuid;

use crate::{
    ServerError,
    keys::{forget_block_keys, store_keys},
    limits::{MAX_BLOCKS_PER_ACCOUNT, MAX_METADATA_BYTES},
    store::{
        Collected, Identity, ServerStore, encode_access, encode_parent, parse_uuid,
        usage::{forget_objects, payer_for_new, persist_sizes},
    },
};

pub enum PublishOutcome {
    Published(CommitId, Option<Vec<Uuid>>),
    Rejected(Option<CommitId>),
}

pub(crate) struct Visibility {
    account: Uuid,
    organization: Uuid,
    administrator: bool,
    /// The block a block's agent is bound to, outside which it reaches nothing.
    bound: Option<Uuid>,
    visible: std::cell::OnceCell<BTreeMap<Uuid, Access>>,
}

impl Visibility {
    fn of(identity: Identity) -> Self {
        Self {
            account: identity.account,
            organization: identity.organization,
            // A block's agent never administers, whatever its role says.
            administrator: identity.role == OrganizationRole::Administrator
                && identity.bound.is_none(),
            bound: identity.bound,
            visible: std::cell::OnceCell::new(),
        }
    }

    /// Whether `block` is within what this caller may reach at all: anything, or for a block's
    /// agent its block and the blocks under it.
    fn reaches(&self, graph: &BlockGraph, block: Uuid) -> bool {
        self.bound
            .is_none_or(|root| block == root || graph.ancestors(block).any(|above| above == root))
    }

    fn inherited(&self, graph: &BlockGraph, block: Uuid) -> Access {
        if !self.reaches(graph, block) {
            return Access::None;
        }
        match (graph.contains(block), self.administrator) {
            (false, _) => Access::None,
            (true, administrator) if graph.closed_to(block, self.account) => match administrator {
                true => Access::KnowExists,
                false => graph.inherited(block, self.account).min(Access::KnowExists),
            },
            (true, true) => Access::Manage,
            (true, false) if crate::organization::in_settings(graph, self.organization, block) => {
                Access::View
            }
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

    /// The blocks a member who administers nothing may know of, and perhaps some more: what lies
    /// below the member's own entry points and in the organization's settings. A member's call
    /// walks these rather than the whole organization, as the personal one holds every
    /// account's blocks. `None` for an administrator, who may know of everything.
    fn candidates(&self, graph: &BlockGraph) -> Option<BTreeSet<Uuid>> {
        if let Some(root) = self.bound {
            return Some(graph.subtree(root).into_iter().collect());
        }
        if self.administrator {
            return None;
        }
        let mut blocks: BTreeSet<Uuid> = self
            .visible
            .get_or_init(|| graph.visible(self.account))
            .keys()
            .copied()
            .collect();
        blocks.extend(graph.subtree(crate::organization::settings_of(self.organization)));
        Some(blocks)
    }

    fn access(&self, graph: &BlockGraph, block: Uuid) -> Access {
        if !self.reaches(graph, block) {
            return Access::None;
        }
        let inherited = self.inherited(graph, block);
        if inherited.can_view() || !graph.contains(block) {
            return inherited;
        }
        self.visible
            .get_or_init(|| graph.visible(self.account))
            .get(&block)
            .copied()
            .unwrap_or_default()
            .max(inherited)
    }
}

pub(crate) fn visibility(identity: Identity) -> Visibility {
    Visibility::of(identity)
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
    if graph.contains(block) && seen.inherited(graph, block) >= level {
        return Ok(());
    }
    Err(refusal(graph, seen, block, verb))
}

/// Why a block below the access a call needs is refused. A block the caller may not know exists
/// answers as one that does not, so no refusal tells anyone what another organization or account
/// holds.
fn refusal(graph: &BlockGraph, seen: &Visibility, block: Uuid, verb: &str) -> ServerError {
    match seen.access(graph, block).can_know_exists() {
        true => ServerError::Refused(
            ErrorCode::PermissionDenied,
            format!("block {block} is not {verb} by this account"),
        ),
        false => not_found(block),
    }
}

pub(crate) fn not_found(block: Uuid) -> ServerError {
    ServerError::Refused(
        ErrorCode::BlockNotFound,
        format!("block {block} does not exist"),
    )
}

fn verb(level: Access) -> &'static str {
    match level {
        Access::None | Access::KnowExists => "known",
        Access::View => "readable",
        Access::Edit => "editable",
        Access::Manage => "managed",
    }
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

/// A search index lies under the block whose tree it indexes, one held per block: the server
/// cannot read the `SEARCH_INDEX` metadata that names it, so it holds to the type and parent
/// (plans/search.md). `index` is the index being placed, which may already be the one held.
fn require_index_place(
    graph: &BlockGraph,
    index: Uuid,
    parent: BlockParent,
) -> Result<(), ServerError> {
    let BlockParent::Block(parent) = parent else {
        return Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            "a search index lies under the block it indexes".into(),
        ));
    };
    if graph
        .get(parent)
        .is_none_or(|node| node.parent != BlockParent::Root)
    {
        return Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            "a search index lies under a top-level block, the root of the tree it indexes".into(),
        ));
    }
    let held = graph
        .children(BlockParent::Block(parent))
        .into_iter()
        .any(|child| {
            child != index
                && graph.holds(child)
                && graph
                    .get(child)
                    .is_some_and(|node| node.content_type == SEARCH_INDEX_TYPE)
        });
    match held {
        true => Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            format!("block {parent} already holds a search index"),
        )),
        false => Ok(()),
    }
}

/// A block holds at most one search index among its children.
fn require_one_index(
    graph: &BlockGraph,
    block: Uuid,
    references: &[Uuid],
) -> Result<(), ServerError> {
    let indexes = references
        .iter()
        .filter(|reference| {
            graph.get(**reference).is_some_and(|node| {
                node.content_type == SEARCH_INDEX_TYPE && node.parent == BlockParent::Block(block)
            })
        })
        .count();
    match indexes > 1 {
        true => Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            format!("block {block} would hold more than one search index"),
        )),
        false => Ok(()),
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

/// Whether `seen` may delete a detached `block` for good: one it may edit, or an owner-only block
/// (or one inside it) whose place it may edit, since deleting ciphertext nobody else opens
/// reveals nothing, and leaving it would strand it below a parent that is gone.
fn may_collect(graph: &BlockGraph, seen: &Visibility, block: Uuid) -> bool {
    seen.can_edit(graph, block)
        || graph
            .owner_only_root(block)
            .and_then(|root| graph.get(root))
            .and_then(|root| root.parent.block())
            .is_some_and(|place| seen.can_edit(graph, place))
}

/// The `chosen` blocks that can go, children before their parents: a block goes only with every
/// child it has, so no block is ever left below one that is gone.
fn removable(graph: &BlockGraph, mut chosen: BTreeSet<Uuid>) -> Vec<Uuid> {
    loop {
        let kept: BTreeSet<Uuid> = chosen
            .iter()
            .copied()
            .filter(|block| {
                graph
                    .children(BlockParent::Block(*block))
                    .iter()
                    .all(|child| chosen.contains(child))
            })
            .collect();
        if kept.len() == chosen.len() {
            break;
        }
        chosen = kept;
    }
    let mut ordered: Vec<Uuid> = chosen.into_iter().collect();
    ordered.sort_by_key(|block| std::cmp::Reverse(graph.ancestors(*block).count()));
    ordered
}

/// An owner-only block is made only in its maker's own settings: a `Settings` or
/// `SettingsProfile` block the maker authored, so no one plants one in a block someone else
/// pays for and could then neither open nor delete.
fn require_own_settings(
    graph: &BlockGraph,
    identity: Identity,
    parent: BlockParent,
) -> Result<(), ServerError> {
    let own = parent
        .block()
        .and_then(|parent| graph.get(parent))
        .is_some_and(|parent| {
            parent.author == identity.account
                && !parent.owner_only
                && be_block::holds_owner_only(parent.content_type)
        });
    match own {
        true => Ok(()),
        false => Err(ServerError::Refused(
            ErrorCode::PermissionDenied,
            "a block readable by its author alone is made only in the author's own settings".into(),
        )),
    }
}

/// Only the author of an owner-only block, or of the one it lies in, moves or shares it, whatever
/// access anyone else is given (decisions/owner_only_settings_are_never_escrowed.md).
fn require_owner_of_owner_only(
    graph: &BlockGraph,
    identity: Identity,
    block: Uuid,
    verb: &str,
) -> Result<(), ServerError> {
    match graph.closed_to(block, identity.account) {
        true => Err(ServerError::Refused(
            ErrorCode::PermissionDenied,
            format!("block {block} is readable by its author alone, and only its author {verb} it"),
        )),
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
        size: node.size,
        tree_size: node.tree_size,
        owner_only: node.owner_only,
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
        owner_only: bool,
    ) -> Result<BlockSummary, ServerError> {
        let references = distinct(references);
        crate::limits::at_most("metadata", metadata.len(), MAX_METADATA_BYTES)?;
        self.with_graph(identity.organization, move |store, graph, database| {
            if graph.authored(identity.account) >= MAX_BLOCKS_PER_ACCOUNT {
                return Err(crate::limits::too_many(
                    "blocks one account makes in an organization",
                    MAX_BLOCKS_PER_ACCOUNT,
                ));
            }
            if let BlockParent::Block(target) = parent {
                require_parent_of(graph, &visibility(identity), content_type, target)?;
            }
            if owner_only {
                require_own_settings(graph, identity, parent)?;
            }
            if content_type == SEARCH_INDEX_TYPE {
                require_index_place(graph, block, parent)?;
            }
            store.may_create(graph, database, identity, block, content_type, parent)?;
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
            node.owner_only = owner_only;
            graph.insert(block, node)?;
            let (kind, parent_id) = encode_parent(parent);
            let transaction = database.unchecked_transaction()?;
            let payer = payer_for_new(&transaction, identity.organization, parent, identity.account)?;
            transaction.execute(
                "INSERT INTO blocks
                    (organization_id, id, content_type, author, parent_kind, parent_id, head, metadata,
                     payer, owner_only)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9)",
                params![
                    identity.organization.to_string(),
                    block.to_string(),
                    content_type.to_string(),
                    identity.account.to_string(),
                    kind,
                    parent_id,
                    metadata,
                    payer.to_string(),
                    owner_only,
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
        crate::limits::at_most("metadata", metadata.len(), MAX_METADATA_BYTES)?;
        self.with_graph(identity.organization, move |_, graph, database| {
            require_edit(graph, &visibility(identity), block)?;
            if let Some(head) = graph.head(block) {
                return Err(ServerError::Refused(
                    ErrorCode::BlockHasContent,
                    format!("block {block} has content at {head}, so its metadata is published with it"),
                ));
            }
            require_keeping_settings(graph, &visibility(identity), block, &references)?;
            require_one_index(graph, block, &references)?;
            let relinked = relinked(graph, block, &references);
            let organization = identity.organization.to_string();
            let transaction = database.unchecked_transaction()?;
            transaction.execute(
                "UPDATE blocks SET metadata = ?3, version = version + 1 WHERE organization_id = ?1 AND id = ?2",
                params![organization, block.to_string(), metadata],
            )?;
            if relinked.is_some() {
                let before = graph.references(block);
                change_edges(&transaction, &organization, block, &before, &references)?;
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
    ) -> Result<Vec<Vec<(crate::watch::Push, bool)>>, ServerError> {
        self.with_graph(organization, move |_, graph, _| {
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
                                (crate::watch::Push::BlockChanged(found), entry)
                            }
                            _ => (
                                crate::watch::Push::BlockRemoved {
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
        self.with_graph(organization, move |_, graph, _| {
            Ok(blocks
                .into_iter()
                .flat_map(|block| graph.audience(block))
                .collect())
        })
        .await
    }

    /// Refuses unless the caller has at least `level` on every one of `blocks`: the check the
    /// dispatcher makes for a call's block arguments before its handler runs. A refusal here
    /// keeps the cached graph, unlike an error inside a change.
    pub async fn require_access(
        &self,
        identity: Identity,
        blocks: Vec<Uuid>,
        level: Access,
    ) -> Result<(), ServerError> {
        self.with_graph(identity.organization, move |_, graph, _| {
            let seen = visibility(identity);
            Ok(blocks
                .into_iter()
                .find(|block| !graph.contains(*block) || seen.access(graph, *block) < level)
                .map_or(Ok(()), |block| {
                    Err(refusal(graph, &seen, block, verb(level)))
                }))
        })
        .await?
    }

    pub async fn read_block(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<BlockSummary, ServerError> {
        self.with_graph(identity.organization, move |_, graph, _| {
            let Some(node) = graph.get(block) else {
                return Err(not_found(block));
            };
            let seen = visibility(identity);
            if !seen.can_view(graph, block) {
                let access = match seen.reaches(graph, block) {
                    true => graph
                        .effective_access(block, identity.account)
                        .max(seen.access(graph, block)),
                    false => Access::None,
                };
                return match access.can_know_exists() {
                    true => Ok(BlockSummary {
                        revision: graph.revision(),
                        ..BlockSummary::no_access(block, node.content_type, access)
                    }),
                    false => Err(not_found(block)),
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
        self.with_graph(identity.organization, move |_, graph, database| {
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
                require(graph, &seen, block, Access::View, "readable")?;
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
        held: ObjectChange,
        time: i64,
        pinned: bool,
        references: ReferenceChange,
        metadata: Vec<u8>,
    ) -> Result<PublishOutcome, ServerError> {
        crate::limits::at_most("metadata", metadata.len(), MAX_METADATA_BYTES)?;
        let change = held;
        let mut sent = vec![commit.hash()];
        sent.extend(change.added.iter().copied());
        self.confirm_uploads(identity, sent.clone()).await?;
        self.with_graph(identity.organization, move |store, graph, database| {
            store.keeping_refs(database, || {
                require_edit(graph, &visibility(identity), block)?;
                let current = graph.head(block);
                if current != expected {
                    return Ok(PublishOutcome::Rejected(current));
                }
                if current == Some(commit) {
                    return Ok(PublishOutcome::Published(commit, None));
                }
                let organization = identity.organization.to_string();
                let base = match expected {
                    Some(expected) => {
                        let mut base = commit_objects(database, &organization, block, expected)?;
                        base.retain(|hash| *hash != expected.hash());
                        base
                    }
                    None => Vec::new(),
                };
                let mut held = vec![commit.hash()];
                held.extend(change.apply(&base));
                // A first commit lists every reference, replacing any its block was made with.
                let listed = match expected {
                    Some(_) => graph.references(block),
                    None => Vec::new(),
                };
                let references = distinct(references.apply(&listed));
                require_keeping_settings(graph, &visibility(identity), block, &references)?;
                require_one_index(graph, block, &references)?;
                let relinked = relinked(graph, block, &references);
                let redescribed = (relinked.is_some()
                    || graph
                        .get(block)
                        .is_some_and(|node| node.metadata != metadata))
                .then(|| relinked.clone().unwrap_or_default());
                let transaction = database.unchecked_transaction()?;
                let existing: Option<i64> = transaction
                    .query_row(
                        "SELECT 1 FROM block_commits
                     WHERE organization_id = ?1 AND block_id = ?2 AND commit_id = ?3",
                        params![organization, block.to_string(), commit.hash().to_hex()],
                        |row| row.get(0),
                    )
                    .optional()?;
                if existing.is_none() {
                    store.require_holdable(graph, database, identity, &sent, "published")?;
                    let holding =
                        store.before_holding(&transaction, graph, identity, block, &held)?;
                    transaction.execute(
                        "INSERT INTO block_commits
                        (organization_id, block_id, commit_id, parents, time, pinned, received)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            organization,
                            block.to_string(),
                            commit.hash().to_hex(),
                            encode_parents(&parents),
                            time,
                            pinned,
                            be_block_commit::now_milliseconds()
                        ],
                    )?;
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
                    let grew = store.charge_holding(graph, &transaction, &holding)?;
                    let mut refs = store.object_refs();
                    refs.retain(&held);
                    persist_refs(&transaction, &refs, &held)?;
                    // The commit now holds what the block held until one did, so this frees and
                    // uncharges nothing.
                    release_committed(
                        &transaction,
                        &mut refs,
                        identity.organization,
                        block,
                        &held,
                    )?;
                    std::mem::drop(refs);
                    graph.add_size(block, grew, true)?;
                    persist_sizes(
                        &transaction,
                        graph,
                        identity.organization,
                        chain(graph, block),
                    )?;
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
                    let before = graph.references(block);
                    change_edges(&transaction, &organization, block, &before, &references)?;
                }
                let content_type = graph.get(block).ok_or(ServerError::Corrupt)?.content_type;
                let policy = crate::retention::policy(
                    content_type,
                    &store.policies(graph, database, identity.organization)?,
                );
                let freed = store.thin_history(
                    &transaction,
                    graph,
                    identity.organization,
                    block,
                    commit,
                    &policy,
                )?;
                transaction.commit()?;
                graph.set_references(block, &references)?;
                graph.set_head(block, commit)?;
                graph.set_metadata(block, metadata)?;
                graph.touch(block)?;
                store.objects().remove_freed(&freed);
                Ok(PublishOutcome::Published(commit, redescribed))
            })
        })
        .await
    }

    /// Which of `hashes` the caller may read, in order: those a block it may view holds. A held
    /// object was confirmed stored before any block could take it (`holdable`), so this asks
    /// no object store.
    pub async fn may_read_objects(
        &self,
        identity: Identity,
        hashes: Vec<Hash>,
    ) -> Result<Vec<bool>, ServerError> {
        // Downloads take neither the database lock nor the graph job's: the holders come from the
        // read-only connection and access from the cached graph, which a revocation changes
        // before its call returns. Only a graph not cached yet costs a graph job.
        let organization = identity.organization;
        let asked = hashes.clone();
        let cached = self
            .with_reader(move |store, reader| {
                let holders = asked
                    .iter()
                    .map(|hash| holders_of(reader, organization, *hash))
                    .collect::<Result<Vec<_>, _>>()?;
                let seen = visibility(identity);
                Ok(store.read_graph(organization, |graph| {
                    holders
                        .iter()
                        .map(|holders| holders.iter().any(|block| seen.can_view(graph, *block)))
                        .collect::<Vec<bool>>()
                }))
            })
            .await?;
        if let Some(readable) = cached {
            return Ok(readable);
        }
        self.with_graph(organization, move |_, graph, database| {
            hashes
                .iter()
                .map(|hash| readable(graph, database, identity, *hash))
                .collect()
        })
        .await
    }

    /// Which of `hashes` the caller lacks, as far as it may know: an object it neither uploaded
    /// nor may read is missing to it whether or not someone else stored it.
    pub async fn missing_objects(
        &self,
        identity: Identity,
        hashes: Vec<Hash>,
    ) -> Result<Vec<Hash>, ServerError> {
        self.confirm_uploads(identity, hashes.clone()).await?;
        self.with_graph(identity.organization, move |store, graph, database| {
            let mut missing = Vec::new();
            for hash in hashes {
                if !store.holdable(graph, database, identity, hash)? {
                    missing.push(hash);
                }
            }
            Ok(missing)
        })
        .await
    }

    /// Applies retention as a commit arrives, in the publish's transaction: the plan
    /// (`be_block_commit::retention`) drops the commit the new one replaces in its slot and
    /// whatever has aged into a wider one. The head, bookmarks, merges' parents and everything
    /// inside the organization's `history_kept_for` stay. Returns the objects no commit holds any
    /// more, to remove once the transaction commits.
    fn thin_history(
        &self,
        transaction: &Connection,
        graph: &mut BlockGraph,
        organization: Uuid,
        block: Uuid,
        head: CommitId,
        policy: &be_block_commit::RetentionPolicy,
    ) -> Result<Vec<Hash>, ServerError> {
        let history = read_history(transaction, organization, block)?;
        let merged: BTreeSet<CommitId> = history
            .iter()
            .filter(|entry| entry.parents.len() > 1)
            .flat_map(|entry| entry.parents.iter().copied())
            .collect();
        let now = be_block_commit::now_milliseconds();
        let mut received = BTreeMap::new();
        {
            let mut statement = transaction.prepare_cached(
                "SELECT commit_id, received FROM block_commits
                 WHERE organization_id = ?1 AND block_id = ?2",
            )?;
            let rows = statement.query_map(
                params![organization.to_string(), block.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )?;
            for row in rows {
                let (commit, at) = row?;
                received.insert(parse_commit(&commit)?, at);
            }
        }
        // A commit's own time comes from its author, so what the organization keeps is also
        // judged by when the server received it: a backdated commit is not dropped early.
        let kept = |entry: &HistoryEntry| {
            policy.keep_all_within > 0
                && received
                    .get(&entry.commit)
                    .is_some_and(|at| *at > now - policy.keep_all_within)
        };
        let summaries: Vec<_> = history
            .iter()
            .map(|entry| be_block_commit::CommitSummary {
                id: entry.commit,
                time: entry.time,
                pinned: entry.pinned || merged.contains(&entry.commit) || kept(entry),
            })
            .collect();
        // The head is the newest commit, which wins its slot, unless its author's clock is
        // behind: it stays either way.
        let mut plan = be_block_commit::plan_retention(&summaries, now, policy);
        plan.drop.retain(|commit| *commit != head);
        if plan.drop.is_empty() {
            return Ok(Vec::new());
        }
        let mut refs = self.object_refs();
        let mut freed = Vec::new();
        let mut released = Vec::new();
        for commit in plan.drop {
            let (rows, gone) = release_commit(transaction, &mut refs, organization, block, commit)?;
            released.extend(rows);
            freed.extend(gone);
        }
        std::mem::drop(refs);
        let shrank = self.release_charged(transaction, graph, organization, block, &released)?;
        let freed = forget_objects(transaction, &freed)?;
        graph.add_size(block, shrank, false)?;
        persist_sizes(transaction, graph, organization, chain(graph, block))?;
        Ok(freed)
    }

    /// Whether the caller may have a block hold `hash`: the server has it (its bytes were
    /// confirmed, `confirm_uploads`), and the caller uploaded
    /// it in this organization or may already read it. Knowing a hash proves nothing, so a hash
    /// someone else stored is not the caller's to hold, publish or then read.
    fn holdable(
        &self,
        graph: &BlockGraph,
        database: &Connection,
        identity: Identity,
        hash: Hash,
    ) -> Result<bool, ServerError> {
        Ok(crate::store::stored(database, hash)?
            && (crate::store::uploaded(database, identity, hash)?
                || readable(graph, database, identity, hash)?))
    }

    fn require_holdable(
        &self,
        graph: &BlockGraph,
        database: &Connection,
        identity: Identity,
        hashes: &[Hash],
        verb: &str,
    ) -> Result<(), ServerError> {
        for hash in hashes {
            if !self.holdable(graph, database, identity, *hash)? {
                return Err(ServerError::Refused(
                    ErrorCode::ObjectNotFound,
                    format!("object {hash} must be uploaded before it can be {verb}"),
                ));
            }
        }
        Ok(())
    }

    pub async fn hold_objects(
        &self,
        identity: Identity,
        block: Uuid,
        objects: Vec<Hash>,
        hold: Hold,
    ) -> Result<(), ServerError> {
        self.confirm_uploads(identity, objects.clone()).await?;
        let since = match hold {
            Hold::Lasting => None,
            Hold::UntilCommitted => Some(be_block_commit::now_milliseconds()),
        };
        self.with_graph(identity.organization, move |store, graph, database| {
            store.keeping_refs(database, || {
                require_edit(graph, &visibility(identity), block)?;
                store.require_holdable(graph, database, identity, &objects, "held")?;
                let transaction = database.unchecked_transaction()?;
                let holding =
                    store.before_holding(&transaction, graph, identity, block, &objects)?;
                let organization = identity.organization.to_string();
                let mut fresh = Vec::new();
                for hash in objects {
                    let held: Option<Option<i64>> = transaction
                        .prepare_cached(
                            "SELECT held_since FROM block_objects
                         WHERE organization_id = ?1 AND block_id = ?2 AND hash = ?3",
                        )?
                        .query_row(
                            params![organization, block.to_string(), hash.to_hex()],
                            |row| row.get(0),
                        )
                        .optional()?;
                    match held {
                        None => {
                            transaction.execute(
                            "INSERT INTO block_objects (organization_id, block_id, hash, held_since)
                             VALUES (?1, ?2, ?3, ?4)",
                            params![organization, block.to_string(), hash.to_hex(), since],
                        )?;
                            fresh.push(hash);
                        }
                        // A lasting hold outlasts any other, and holding again extends a hold.
                        Some(None) => {}
                        Some(Some(held)) => {
                            let extended = since.map(|since| since.max(held));
                            transaction.execute(
                                "UPDATE block_objects SET held_since = ?4
                             WHERE organization_id = ?1 AND block_id = ?2 AND hash = ?3",
                                params![organization, block.to_string(), hash.to_hex(), extended],
                            )?;
                        }
                    }
                }
                let grew = store.charge_holding(graph, &transaction, &holding)?;
                let mut refs = store.object_refs();
                refs.retain(&fresh);
                persist_refs(&transaction, &refs, &fresh)?;
                graph.add_size(block, grew, true)?;
                persist_sizes(
                    &transaction,
                    graph,
                    identity.organization,
                    chain(graph, block),
                )?;
                transaction.commit()?;
                Ok(())
            })
        })
        .await
    }

    /// Ends the `until-committed` holds past their grace (`hold_objects`), uncharging their
    /// blocks, and deletes what nothing holds any more. A block whose journal holds entries keeps
    /// its holds, since an operation there may name a part no commit took yet: they end once a
    /// commit rebases the journal and the grace has passed. Answers how many objects it
    /// deleted.
    pub(crate) async fn collect_holds(&self) -> Result<usize, ServerError> {
        let grace = i64::try_from(self.upload_grace().as_millis()).unwrap_or(i64::MAX);
        let cutoff = be_block_commit::now_milliseconds().saturating_sub(grace);
        let organizations: Vec<Uuid> = self
            .with_database(move |_, database| {
                let mut statement = database.prepare(
                    "SELECT DISTINCT organization_id FROM block_objects
                     WHERE held_since IS NOT NULL AND held_since <= ?1
                     AND NOT EXISTS(SELECT 1 FROM journal_entries
                         WHERE journal_entries.organization_id = block_objects.organization_id
                           AND journal_entries.block_id = block_objects.block_id)",
                )?;
                let rows = statement.query_map([cutoff], |row| row.get::<_, String>(0))?;
                rows.map(|row| parse_uuid(&row?)).collect()
            })
            .await?;
        let mut deleted = 0;
        for organization in organizations {
            deleted += self
                .with_graph(organization, move |store, graph, database| store.keeping_refs(database, || {
                    let mut expired: BTreeMap<Uuid, Vec<Hash>> = BTreeMap::new();
                    {
                        let mut statement = database.prepare(
                            "SELECT block_id, hash FROM block_objects
                             WHERE organization_id = ?1 AND held_since IS NOT NULL
                               AND held_since <= ?2
                               AND NOT EXISTS(SELECT 1 FROM journal_entries
                                   WHERE journal_entries.organization_id = block_objects.organization_id
                                     AND journal_entries.block_id = block_objects.block_id)",
                        )?;
                        let rows = statement
                            .query_map(params![organization.to_string(), cutoff], |row| {
                                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                            })?;
                        for row in rows {
                            let (block, hash) = row?;
                            expired
                                .entry(parse_uuid(&block)?)
                                .or_default()
                                .push(Hash::from_hex(&hash).ok_or(ServerError::Corrupt)?);
                        }
                    }
                    let transaction = database.unchecked_transaction()?;
                    let mut refs = store.object_refs();
                    let mut released = Vec::new();
                    let mut freed = Vec::new();
                    for (block, hashes) in &expired {
                        for hash in hashes {
                            transaction.execute(
                                "DELETE FROM block_objects
                                 WHERE organization_id = ?1 AND block_id = ?2 AND hash = ?3",
                                params![organization.to_string(), block.to_string(), hash.to_hex()],
                            )?;
                        }
                        freed.extend(refs.release(hashes));
                        released.extend(hashes.iter().copied());
                    }
                    persist_refs(&transaction, &refs, &released)?;
                    std::mem::drop(refs);
                    for (block, hashes) in &expired {
                        if !graph.contains(*block) {
                            continue;
                        }
                        let shrank = store.release_charged(
                            &transaction,
                            graph,
                            organization,
                            *block,
                            hashes,
                        )?;
                        graph.add_size(*block, shrank, false)?;
                        persist_sizes(&transaction, graph, organization, chain(graph, *block))?;
                    }
                    let freed = forget_objects(&transaction, &freed)?;
                    transaction.commit()?;
                    store.objects().remove_freed(&freed);
                    Ok(freed.len())
                }))
                .await?;
        }
        Ok(deleted)
    }

    pub async fn set_parent(
        &self,
        identity: Identity,
        block: Uuid,
        parent: BlockParent,
        keys: KeySet,
    ) -> Result<(BTreeSet<Uuid>, BTreeSet<Uuid>), ServerError> {
        self.with_graph(identity.organization, move |store, graph, database| {
            let seen = visibility(identity);
            require_edit(graph, &seen, block)?;
            if block == crate::organization::settings_of(identity.organization) {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "an organization's settings stay where they are for as long as it exists"
                        .into(),
                ));
            }
            require_owner_of_owner_only(graph, identity, block, "moves")?;
            if graph.get(block).is_some_and(BlockNode::unfiled) {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "an account's Unfiled stays at the top of the organization".into(),
                ));
            }
            let access = seen.inherited(graph, block);
            if let Some(refusal) = graph.move_refusal(block, parent, identity.account, access) {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    refusal.to_string(),
                ));
            }
            let node = graph.get(block).ok_or(ServerError::Corrupt)?;
            if node.content_type == SEARCH_INDEX_TYPE && parent != BlockParent::Detached {
                // It indexes the tree of the block it was made under, and takes that block's
                // access: moving it under another block would read that tree's text as the
                // new parent's.
                if node.parent != parent {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "a search index stays under the block it indexes".into(),
                    ));
                }
                require_index_place(graph, block, parent)?;
            }
            match parent {
                BlockParent::Block(target) if graph.owner_only_root(target).is_some() => {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("block {target} is readable by its author alone, and nothing moves into it"),
                    ));
                }
                BlockParent::Block(target) => {
                    require_parent_of(graph, &seen, node.content_type, target)?;
                }
                BlockParent::Root if node.parent != BlockParent::Root => {
                    store.may_add_workspace(graph, database, identity, block, node.content_type)?;
                }
                BlockParent::Root | BlockParent::Detached => {}
            }
            let before = graph.audience(block);
            let viewing = match parent {
                BlockParent::Detached => None,
                _ => Some(crate::keys::viewing_below(
                    database,
                    graph,
                    identity.organization,
                    block,
                )?),
            };
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
            store.follow_payer(graph, &transaction, identity, block, parent)?;
            let mut resized = chain(graph, block);
            graph.set_parent(block, parent)?;
            graph.touch(block)?;
            resized.extend(graph.ancestors(block));
            persist_sizes(&transaction, graph, identity.organization, resized)?;
            let lost = match viewing {
                Some(viewing) => {
                    crate::keys::lost_below(&transaction, graph, identity.organization, viewing)?
                }
                None => BTreeSet::new(),
            };
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
            let forgotten = crate::store::forget_orphaned_block_agents(
                graph,
                &transaction,
                identity.organization,
            )?;
            let trashed = crate::store::trashes_block_agents(
                graph,
                &transaction,
                identity.organization,
                block,
            )?;
            transaction.commit()?;
            if !forgotten.is_empty() || trashed {
                store.sessions_changed();
            }
            Ok((before, lost))
        })
        .await
    }

    pub async fn list_shared(&self, identity: Identity) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, _| {
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
        self.with_graph(identity.organization, move |_, graph, _| {
            let seen = visibility(identity);
            let settings = crate::organization::settings_of(identity.organization);
            Ok(graph
                .children(parent)
                .into_iter()
                .filter(|block| *block != settings)
                .filter_map(|block| summary_with(graph, &seen, block))
                .filter(|block| block.access.can_know_exists())
                .collect())
        })
        .await
    }

    pub async fn list_deleted(&self, identity: Identity) -> Result<Vec<BlockSummary>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, _| {
            let seen = visibility(identity);
            let deleted = match seen.candidates(graph) {
                None => graph.deleted_roots(),
                Some(blocks) => blocks
                    .into_iter()
                    .filter(|block| graph.contains(*block) && !graph.holds(*block))
                    .collect(),
            };
            Ok(deleted
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
        self.with_graph(identity.organization, move |_, graph, _| {
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
        self.with_graph(identity.organization, move |_, graph, _| {
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
                    if let Some(summary) =
                        summary_with(graph, &seen, kind).filter(|summary| summary.access.can_view())
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
        self.with_graph(identity.organization, move |_, graph, _| {
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

    /// Which of `blocks` are `root` or below it.
    pub async fn within(
        &self,
        organization: Uuid,
        root: Uuid,
        blocks: Vec<Uuid>,
    ) -> Result<BTreeSet<Uuid>, ServerError> {
        self.with_graph(organization, move |_, graph, _| {
            Ok(blocks
                .into_iter()
                .filter(|block| {
                    *block == root || graph.ancestors(*block).any(|above| above == root)
                })
                .collect())
        })
        .await
    }

    pub async fn list_history(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<Vec<HistoryEntry>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            require(
                graph,
                &visibility(identity),
                block,
                Access::View,
                "readable",
            )?;
            let head = graph.head(block);
            let mut entries = read_history(database, identity.organization, block)?;
            for entry in &mut entries {
                entry.head = head == Some(entry.commit);
            }
            Ok(entries)
        })
        .await
    }

    pub async fn collect_detached(&self, identity: Identity) -> Result<Collected, ServerError> {
        self.with_graph(identity.organization, move |store, graph, database| {
            store.keeping_refs(database, || {
                let seen = visibility(identity);
                let kept_for = store
                    .policies(graph, database, identity.organization)?
                    .history_kept_for();
                let since = be_block_commit::now_milliseconds() - kept_for;
                let mut chosen = BTreeSet::new();
                let candidates = match seen.candidates(graph) {
                    None => graph.detached(),
                    Some(blocks) => blocks
                        .into_iter()
                        .filter(|block| graph.contains(*block) && !graph.is_live(*block))
                        .collect(),
                };
                for block in candidates {
                    if !may_collect(graph, &seen, block) {
                        continue;
                    }
                    if kept_for > 0
                        && newest_version(database, identity.organization, block)? > since
                    {
                        continue;
                    }
                    chosen.insert(block);
                }
                let detached = removable(graph, chosen);
                let transaction = database.unchecked_transaction()?;
                let mut refs = store.object_refs();
                let mut freed = Vec::new();
                let mut removed = Vec::new();
                for block in detached {
                    let mut released = Vec::new();
                    for commit in read_history(&transaction, identity.organization, block)? {
                        let (rows, gone) = release_commit(
                            &transaction,
                            &mut refs,
                            identity.organization,
                            block,
                            commit.commit,
                        )?;
                        released.extend(rows);
                        freed.extend(gone);
                    }
                    let (rows, gone) =
                        release_held(&transaction, &mut refs, identity.organization, block)?;
                    released.extend(rows);
                    freed.extend(gone);
                    let shrank = store.release_charged(
                        &transaction,
                        graph,
                        identity.organization,
                        block,
                        &released,
                    )?;
                    graph.add_size(block, shrank, false)?;
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
                        "DELETE FROM opened_kinds WHERE organization_id = ?1 AND block_id = ?2",
                        params![organization, block.to_string()],
                    )?;
                    transaction.execute(
                        "DELETE FROM blocks WHERE organization_id = ?1 AND id = ?2",
                        params![organization, block.to_string()],
                    )?;
                    forget_block_keys(&transaction, identity.organization, block)?;
                    crate::journal::forget(&transaction, identity.organization, block)?;
                    crate::notifications::forget(&transaction, identity.organization, block)?;
                    let resized = chain(graph, block);
                    graph.remove(block);
                    persist_sizes(&transaction, graph, identity.organization, resized)?;
                    removed.push(block);
                }
                std::mem::drop(refs);
                let freed = forget_objects(&transaction, &freed)?;
                let forgotten = crate::store::forget_orphaned_block_agents(
                    graph,
                    &transaction,
                    identity.organization,
                )?;
                transaction.commit()?;
                if !forgotten.is_empty() {
                    store.sessions_changed();
                }
                store.objects().remove_freed(&freed);
                Ok(Collected {
                    blocks: removed,
                    objects: freed.len(),
                    revision: graph.revision(),
                })
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
        self.with_graph(identity.organization, move |store, graph, database| {
            let leaving = account == identity.account && access == Access::None;
            match leaving {
                true if !visibility(identity)
                    .access(graph, block)
                    .can_know_exists() =>
                {
                    return Err(not_found(block));
                }
                true => {}
                false
                    if account == ANYONE_INSIDE
                        && access == Access::View
                        && graph.get(block).is_some_and(|node| {
                            node.is_edit_kind()
                                && node.author == identity.account
                                && !node.grants.contains_key(&ANYONE_INSIDE)
                        })
                        && !was_opened(database, identity.organization, block)? =>
                {
                    require_edit(graph, &visibility(identity), block)?;
                }
                false => require_manage(graph, &visibility(identity), block)?,
            }
            if !leaving {
                if graph
                    .get(block)
                    .is_some_and(|node| node.content_type == SEARCH_INDEX_TYPE)
                {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "a search index takes its access from the block it indexes alone".into(),
                    ));
                }
                require_owner_of_owner_only(graph, identity, block, "shares")?;
                if graph.owner_only_root(block).is_some() && access > Access::None {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("block {block} is readable by its author alone, so it is shared with no one"),
                    ));
                }
            }
            if account == ANYONE_INSIDE {
                if graph.settings_owner(block).is_none() || access > Access::View {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "only a kind of settings opens to anyone who can view a block inside, and only to view".into(),
                    ));
                }
            } else if account == crate::organization::server_account(database)? {
                if access > Access::View {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "the server only reads what is shared with it".into(),
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
            let forgotten = crate::store::forget_orphaned_block_agents(
                graph,
                &transaction,
                identity.organization,
            )?;
            transaction.commit()?;
            if !forgotten.is_empty() {
                store.sessions_changed();
            }
            Ok(before)
        })
        .await
    }

    pub async fn list_access(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<AccessList, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            require_manage(graph, &visibility(identity), block)?;
            let company = crate::keys::organization_kind(database, identity.organization)?
                == be_block_protocol::OrganizationKind::Company;
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
                        inherited: Access::None,
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
                if company
                    && role == OrganizationRole::Administrator
                    && account != identity.account
                {
                    continue;
                }
                let granted = node.grants.get(&account).copied();
                let (effective, source) = match role {
                    OrganizationRole::Administrator => (Access::Manage, None),
                    OrganizationRole::Editor => match graph.inherited_from(block, account) {
                        (level, source) if level.can_know_exists() => (level, source),
                        _ => (graph.effective_access(block, account), None),
                    },
                };
                let inherited = match (role, granted) {
                    (OrganizationRole::Administrator, _) | (_, None) => effective,
                    (OrganizationRole::Editor, Some(_)) => graph.inherited_above(block, account),
                };
                entries.push(AccessEntry {
                    account,
                    email,
                    display_name,
                    role,
                    author: node.author == account,
                    granted,
                    inherited,
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

/// Whether a block the caller can view holds `hash`, by a commit or by `HoldObjects`.
pub(crate) fn readable(
    graph: &BlockGraph,
    database: &Connection,
    identity: Identity,
    hash: Hash,
) -> Result<bool, ServerError> {
    let seen = visibility(identity);
    Ok(holders_of(database, identity.organization, hash)?
        .into_iter()
        .any(|block| seen.can_view(graph, block)))
}

/// The blocks of `organization` that hold `hash`, by a commit or by `HoldObjects`.
fn holders_of(
    database: &Connection,
    organization: Uuid,
    hash: Hash,
) -> Result<Vec<Uuid>, ServerError> {
    let mut statement = database.prepare_cached(
        "SELECT block_id FROM commit_objects WHERE organization_id = ?1 AND hash = ?2
         UNION
         SELECT block_id FROM block_objects WHERE organization_id = ?1 AND hash = ?2",
    )?;
    let rows = statement.query_map(params![organization.to_string(), hash.to_hex()], |row| {
        row.get::<_, String>(0)
    })?;
    rows.map(|row| parse_uuid(&row?)).collect()
}

pub(crate) fn revoke_or_grant(
    graph: &mut BlockGraph,
    database: &Connection,
    organization: Uuid,
    block: Uuid,
    account: Uuid,
    access: Access,
) -> Result<(), ServerError> {
    if account == be_block_graph::ANYONE_INSIDE {
        return open_or_close_inside(graph, database, organization, block, access);
    }
    let viewed = crate::keys::is_viewer(database, graph, organization, account, block)?;
    let kinds = crate::keys::open_kinds_viewed(database, graph, organization, account, block)?;
    write_grant(graph, database, organization, block, account, access)?;
    if viewed && !crate::keys::is_viewer(database, graph, organization, account, block)? {
        crate::keys::lost_view(database, organization, block, account)?;
    }
    crate::keys::lost_open_kinds(database, graph, organization, account, kinds)
}

fn open_or_close_inside(
    graph: &mut BlockGraph,
    database: &Connection,
    organization: Uuid,
    kind: Uuid,
    access: Access,
) -> Result<(), ServerError> {
    let mut viewed = Vec::new();
    if let Some(owner) = graph.settings_owner(kind) {
        for account in graph.inside_viewers(owner) {
            if crate::keys::is_viewer(database, graph, organization, account, kind)? {
                viewed.push(account);
            }
        }
    }
    write_grant(
        graph,
        database,
        organization,
        kind,
        be_block_graph::ANYONE_INSIDE,
        access,
    )?;
    if access > Access::None {
        database.execute(
            "INSERT OR IGNORE INTO opened_kinds (organization_id, block_id) VALUES (?1, ?2)",
            params![organization.to_string(), kind.to_string()],
        )?;
    }
    for account in viewed {
        if !crate::keys::is_viewer(database, graph, organization, account, kind)? {
            crate::keys::lost_view(database, organization, kind, account)?;
        }
    }
    Ok(())
}

fn was_opened(database: &Connection, organization: Uuid, kind: Uuid) -> Result<bool, ServerError> {
    Ok(database
        .prepare_cached("SELECT 1 FROM opened_kinds WHERE organization_id = ?1 AND block_id = ?2")?
        .exists(params![organization.to_string(), kind.to_string()])?)
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

fn newest_version(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<i64, ServerError> {
    let mut statement = connection.prepare_cached(
        "SELECT commit_id, MAX(time, received) FROM block_commits
         WHERE organization_id = ?1 AND block_id = ?2",
    )?;
    let rows = statement.query_map(
        params![organization.to_string(), block.to_string()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let mut newest = i64::MIN;
    for row in rows {
        let (_, at) = row?;
        newest = newest.max(at);
    }
    Ok(newest)
}

fn read_history(
    connection: &Connection,
    organization: Uuid,
    block: Uuid,
) -> Result<Vec<HistoryEntry>, ServerError> {
    let mut statement = connection.prepare(
        "SELECT commit_id, parents, time, pinned FROM block_commits
         WHERE organization_id = ?1 AND block_id = ?2
         ORDER BY time DESC, rowid DESC",
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
            head: false,
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
) -> Result<(Vec<Hash>, Vec<Hash>), ServerError> {
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
    Ok((held, freed))
}

/// Ends the block's `until-committed` holds of objects among `held`, which a commit of it now
/// holds.
fn release_committed(
    connection: &Connection,
    refs: &mut ObjectRefs,
    organization: Uuid,
    block: Uuid,
    held: &[Hash],
) -> Result<(), ServerError> {
    let pending = {
        let mut statement = connection.prepare_cached(
            "SELECT hash FROM block_objects
             WHERE organization_id = ?1 AND block_id = ?2 AND held_since IS NOT NULL",
        )?;
        let rows = statement.query_map(
            params![organization.to_string(), block.to_string()],
            |row| row.get::<_, String>(0),
        )?;
        rows.map(|row| Hash::from_hex(&row?).ok_or(ServerError::Corrupt))
            .collect::<Result<Vec<_>, _>>()?
    };
    if pending.is_empty() {
        return Ok(());
    }
    let held: HashSet<Hash> = held.iter().copied().collect();
    let taken: Vec<Hash> = pending
        .into_iter()
        .filter(|hash| held.contains(hash))
        .collect();
    for hash in &taken {
        connection.execute(
            "DELETE FROM block_objects WHERE organization_id = ?1 AND block_id = ?2 AND hash = ?3",
            params![organization.to_string(), block.to_string(), hash.to_hex()],
        )?;
    }
    refs.release(&taken);
    persist_refs(connection, refs, &taken)?;
    Ok(())
}

fn release_held(
    connection: &Connection,
    refs: &mut ObjectRefs,
    organization: Uuid,
    block: Uuid,
) -> Result<(Vec<Hash>, Vec<Hash>), ServerError> {
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
    Ok((held, freed))
}

/// A block and its ancestors: the blocks whose sizes change with its own.
fn chain(graph: &BlockGraph, block: Uuid) -> Vec<Uuid> {
    std::iter::once(block)
        .chain(graph.ancestors(block))
        .collect()
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

/// Positions of a block's edges are spaced this far apart, so a reference put in between two
/// others takes a free position instead of moving the rest.
const EDGE_SPACING: i64 = 1 << 20;

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
    let positions = (0..).map(|index: i64| index * EDGE_SPACING);
    insert_edges(
        transaction,
        organization,
        block,
        references.iter().copied().zip(positions),
    )
}

fn insert_edges(
    transaction: &rusqlite::Transaction<'_>,
    organization: &str,
    block: Uuid,
    edges: impl IntoIterator<Item = (Uuid, i64)>,
) -> Result<(), ServerError> {
    let mut statement = transaction.prepare_cached(
        "INSERT OR REPLACE INTO block_edges
            (organization_id, block_id, reference_id, position)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (reference, position) in edges {
        statement.execute(params![
            organization,
            block.to_string(),
            reference.to_string(),
            position
        ])?;
    }
    Ok(())
}

/// Turns a block's edges from `before` into `after`, writing only the edges that left, joined
/// or moved; an edge put in between two others takes a position between theirs.
fn change_edges(
    transaction: &rusqlite::Transaction<'_>,
    organization: &str,
    block: Uuid,
    before: &[Uuid],
    after: &[Uuid],
) -> Result<(), ServerError> {
    let change = ReferenceChange::between(before, after);
    {
        let mut removing = transaction.prepare_cached(
            "DELETE FROM block_edges
             WHERE organization_id = ?1 AND block_id = ?2 AND reference_id = ?3",
        )?;
        for reference in &change.removed {
            removing.execute(params![
                organization,
                block.to_string(),
                reference.to_string()
            ])?;
        }
    }
    if change.added.is_empty() {
        return Ok(());
    }
    let mut positions: HashMap<Uuid, i64> = HashMap::new();
    {
        let mut statement = transaction.prepare_cached(
            "SELECT reference_id, position FROM block_edges
             WHERE organization_id = ?1 AND block_id = ?2",
        )?;
        let rows = statement.query_map(params![organization, block.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows {
            let (reference, position) = row?;
            let reference = Uuid::parse_str(&reference).map_err(|_| ServerError::Corrupt)?;
            positions.insert(reference, position);
        }
    }
    let added: HashSet<Uuid> = change.added.iter().map(|placed| placed.block).collect();
    let mut placed = Vec::with_capacity(added.len());
    let mut low = None;
    let mut index = 0;
    while index < after.len() {
        if !added.contains(&after[index]) {
            low = positions.get(&after[index]).copied();
            index += 1;
            continue;
        }
        let end = (index..after.len())
            .find(|at| !added.contains(&after[*at]))
            .unwrap_or(after.len());
        let count = i64::try_from(end - index).map_err(|_| ServerError::Corrupt)?;
        let high = after.get(end).and_then(|next| positions.get(next).copied());
        let (first, step) = match (low, high) {
            (None, None) => (0, EDGE_SPACING),
            (None, Some(high)) => (high - EDGE_SPACING * count, EDGE_SPACING),
            (Some(low), None) => (low + EDGE_SPACING, EDGE_SPACING),
            (Some(low), Some(high)) if high - low > count => {
                let step = (high - low) / (count + 1);
                (low + step, step)
            }
            (Some(_), Some(_)) => return store_edges(transaction, organization, block, after),
        };
        for (offset, reference) in after[index..end].iter().enumerate() {
            let offset = i64::try_from(offset).map_err(|_| ServerError::Corrupt)?;
            placed.push((*reference, first + step * offset));
        }
        low = placed.last().map(|(_, position)| *position);
        index = end;
    }
    insert_edges(transaction, organization, block, placed)
}

/// The objects a commit of `block` holds, in the order it listed them: the commit first.
fn commit_objects(
    database: &Connection,
    organization: &str,
    block: Uuid,
    commit: CommitId,
) -> Result<Vec<Hash>, ServerError> {
    let mut statement = database.prepare_cached(
        "SELECT hash FROM commit_objects
         WHERE organization_id = ?1 AND block_id = ?2 AND commit_id = ?3
         ORDER BY position",
    )?;
    let rows = statement.query_map(
        params![organization, block.to_string(), commit.hash().to_hex()],
        |row| row.get::<_, String>(0),
    )?;
    let mut objects = Vec::new();
    for row in rows {
        objects.push(Hash::from_hex(&row?).ok_or(ServerError::Corrupt)?);
    }
    Ok(objects)
}
