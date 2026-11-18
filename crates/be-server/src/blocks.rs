use be_block_commit::CommitId;
use be_block_graph::{
    ANYONE_INSIDE, Access, BlockGraph, BlockNode, BlockParent, ObjectRefs, SEARCH_INDEX_TYPE,
    SETTINGS_TYPE,
};
use be_block_protocol::{
    AccessEntry, AccessList, AccountName, BlockSummary, ChangeCursor, ChangedPage, ErrorCode,
    HistoryEntry, Hold, Listed, Listing, ObjectChange, OrganizationRole, ReferenceChange,
};
use be_block_store::Hash;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use uuid::Uuid;

/// What `changes` needs of a block that leaves a tree.
fn departing(graph: &BlockGraph, block: Uuid) -> crate::changes::Departing {
    crate::changes::Departing {
        block,
        owner: graph
            .owner_only_root(block)
            .and_then(|root| graph.get(root))
            .map(|root| root.author),
    }
}

/// The most blocks of either kind a page of `query.tree` holds.
const MAX_CHANGES_PER_PAGE: usize = 500;
/// The summaries of one `query.tree` page: once they come to this, the page ends, so with
/// one summary more, however large, it stays inside a frame.
const MAX_PAGE_SUMMARY_BYTES: usize = be_block_protocol::MAX_FRAME_BYTES / 4;
const _: () = assert!(
    MAX_PAGE_SUMMARY_BYTES
        + crate::LARGEST_SUMMARY_BYTES
        + MAX_CHANGES_PER_PAGE * 17
        + be_block_protocol::FRAME_ENVELOPE_BYTES
        <= be_block_protocol::MAX_FRAME_BYTES
);

/// The most changes one page of `query.tree` looks at, those of other trees included.
const MAX_SCANNED: usize = if cfg!(test) { 6 } else { 20_000 };

use crate::{
    ServerError,
    limits::{MAX_BLOCKS_PER_ACCOUNT, MAX_COMMIT_AHEAD, MAX_METADATA_BYTES},
    store::{
        Identity, ServerStore, encode_access, encode_parent, parse_uuid,
        usage::{forget_objects, payer_for_new, persist_sizes},
    },
};

pub enum PublishOutcome {
    /// The head moved, or was the commit already; with the blocks the publish started or
    /// stopped referencing, and what the publish's seal did to its block's journal, when it
    /// trimmed it.
    Published(CommitId, Vec<Uuid>, Option<crate::journal::Trimmed>),
    Rejected(Option<CommitId>),
}

/// What a move did besides moving its block.
pub struct Moved {
    /// Who could know of the block before.
    pub before: BTreeSet<Uuid>,
    /// Who no longer views something below it.
    pub lost: BTreeSet<Uuid>,
    /// The blocks above it before, whose followers hear it left.
    pub above: Vec<Uuid>,
    /// The scope above where it was, none from the top or the trash (`keys::Whole::Moved`).
    pub scope_before: Option<Uuid>,
}

pub(crate) struct Visibility {
    account: Uuid,
    organization: Uuid,
    administrator: bool,
    /// A guest of the organization (guides/blocks/keys.md, "Share links"): it reaches only what
    /// links granted it, not the organization's settings, and is served its log as a proof path.
    guest: bool,
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
            guest: identity.role == OrganizationRole::Guest,
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
            (true, false)
                if !self.guest
                    && crate::organization::in_settings(graph, self.organization, block) =>
            {
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

    /// Whether a block this caller may view lies below `block`, whose log it is then served to
    /// check that block's ancestry and the records resting on it (guides/blocks/keys.md, "Parent
    /// records").
    pub(crate) fn reads_below(&self, graph: &BlockGraph, block: Uuid) -> bool {
        graph
            .entry_points(self.account)
            .into_iter()
            .any(|entry| graph.ancestors(entry).any(|above| above == block))
    }

    /// Whether this caller is served `block`'s whole access log rather than a proof path
    /// (guides/blocks/keys.md, "Who is served what"): it has Edit or above there, or on a block
    /// below it, whose sealing checks every ancestor's log too.
    pub(crate) fn served_whole(&self, graph: &BlockGraph, block: Uuid) -> bool {
        self.can_edit(graph, block)
            || graph.entry_points(self.account).into_iter().any(|entry| {
                graph.ancestors(entry).any(|above| above == block) && self.can_edit(graph, entry)
            })
    }

    /// Whether this caller is served the organization's whole log rather than a proof path: it
    /// administers the organization, or has Edit or above on any of its blocks.
    /// A guest never is: it would learn every member.
    pub(crate) fn served_whole_organization(&self, graph: &BlockGraph) -> bool {
        if self.guest {
            return false;
        }
        match self.candidates(graph) {
            None => true,
            Some(blocks) => blocks.into_iter().any(|block| self.can_edit(graph, block)),
        }
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
        if !self.guest {
            blocks.extend(graph.subtree(crate::organization::settings_of(self.organization)));
        }
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

pub(crate) fn require_edit(
    graph: &BlockGraph,
    seen: &Visibility,
    block: Uuid,
) -> Result<(), ServerError> {
    require(graph, seen, block, Access::Edit, "editable")
}

pub(crate) fn require_manage(
    graph: &BlockGraph,
    seen: &Visibility,
    block: Uuid,
) -> Result<(), ServerError> {
    require(graph, seen, block, Access::Manage, "managed")
}

pub(crate) fn require(
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
        Access::Join => "joinable",
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

/// A head moves only to a commit that descends from it: a publish onto a head lists that head
/// among its parents. A publish onto no head may list parents: a merge onto a block with no
/// commit yet carries the commit the server refused it. Other parents need not be on the server
/// yet either: a merge carries them, objects and all.
fn require_descends(expected: Option<CommitId>, parents: &[CommitId]) -> Result<(), ServerError> {
    match expected {
        Some(expected) if !parents.contains(&expected) => Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            format!("a commit onto {expected} lists it among its parents"),
        )),
        _ => Ok(()),
    }
}

/// A search index lies under the block whose tree it indexes, one held per block: the server cannot
/// read the `SEARCH_INDEX` metadata that names it, so it holds to the type and parent
/// (guides/blocks/host.md, "Search"). `index` is the index being placed, which may already be the
/// one held.
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

/// The account's own block (`be_block::account_block`) is made only at its id in the personal
/// organization, owner-only and at the top, so it holds the account's records alone and no one
/// else reads it (guides/blocks/keys.md, "Contacts").
fn require_account_block(
    connection: &rusqlite::Connection,
    identity: Identity,
    content_type: Uuid,
    parent: BlockParent,
    owner_only: bool,
) -> Result<(), ServerError> {
    let personal = crate::keys::organization_kind(connection, identity.organization)?
        == be_block_protocol::OrganizationKind::Personal;
    match personal
        && owner_only
        && parent == BlockParent::Root
        && content_type == <be_block::AccountBlock as be_block_content::Root>::CONTENT_TYPE
    {
        true => Ok(()),
        false => Err(ServerError::Refused(
            ErrorCode::PermissionDenied,
            "an account's own block is made only by it, readable by it alone, at the top of the personal organization".into(),
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
    if !access.can_view() {
        // Its own `changed` still, so this lesser summary orders after the one a reader held
        // while it could view the block.
        return Some(BlockSummary {
            changed: node.changed,
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
        metadata: node.metadata.clone(),
        version: Some(node.version),
        key: node.key,
        scope: Some(graph.scope(block)),
        changed: node.changed,
        entry: Some(entry_of(graph, seen, block)),
        held: graph.holds(block),
        opens_inside: node.is_open_inside() && graph.settings_owner(block).is_some(),
        sets_access: node.sets_access(),
        size: node.size,
        tree_size: node.tree_size,
        owner_only: node.owner_only,
    })
}

/// The entry point a caller who may view `block` follows to hear of its changes: the highest
/// block above it, or itself, that the caller may view through every block between them. That
/// is the top of its tree, or the block where the caller's view starts below a parent it may
/// not view. O(depth) access checks, each O(depth).
fn entry_of(graph: &BlockGraph, seen: &Visibility, block: Uuid) -> Uuid {
    let mut entry = block;
    while let Some(BlockParent::Block(parent)) = graph.get(entry).map(|node| node.parent) {
        if !graph.contains(parent) || !seen.can_view(graph, parent) {
            break;
        }
        entry = parent;
    }
    entry
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
        owner_only: bool,
        root: Vec<u8>,
        placement: Vec<u8>,
    ) -> Result<BlockSummary, ServerError> {
        let references = distinct(references);
        crate::limits::at_most("metadata", metadata.len(), MAX_METADATA_BYTES)?;
        crate::limits::references(&references)?;
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
            let account_block = block == be_block::account_block(identity.organization, identity.account);
            if account_block {
                require_account_block(database, identity, content_type, parent, owner_only)?;
            } else if owner_only {
                require_own_settings(graph, identity, parent)?;
            }
            if content_type == SEARCH_INDEX_TYPE {
                require_index_place(graph, block, parent)?;
            }
            store.may_create(graph, database, identity, block, content_type, parent)?;
            let mut node = BlockNode::new(content_type, identity.account, parent);
            node.metadata.clone_from(&metadata);
            node.owner_only = owner_only;
            graph.insert(block, node)?;
            // Its key row comes in a `put-keys` of the same write, checked at the write's end
            // (`keys::Whole::Made`).
            graph.set_key(block, key)?;
            let (kind, parent_id) = encode_parent(parent);
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let payer = payer_for_new(&transaction, identity.organization, parent, identity.account)?;
            transaction.execute(
                "INSERT INTO blocks
                    (organization_id, id, content_type, author, parent_kind, parent_id, head, metadata,
                     payer, owner_only, key_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10)",
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
                    key.to_string(),
                ],
            )?;
            crate::access::store_root(
                &transaction,
                identity.organization,
                identity.account,
                block,
                parent,
                &root,
                &placement,
            )?;
            // The blocks it references are known to exist by whoever views it: they changed too.
            let mut touched = vec![block];
            touched.extend(references.iter().copied());
            crate::changes::touch(&transaction, graph, identity.organization, &touched)?;
            store_edges(
                &transaction,
                &identity.organization.to_string(),
                block,
                &references,
            )?;
            transaction.commit()?;
            graph.set_references(block, &references)?;
            summary(graph, identity, block).ok_or(ServerError::Corrupt)
        })
        .await
    }

    /// The block's access log, root first (`query.log`), to a caller who may view or join it
    /// or view a block below it: whole to one with Edit or above on it, or who views it with
    /// Edit below, else a proof path of the caller's own right, of that of each lease holder in
    /// `hosts` (each session the caller is in, by its block, with its holder's account) whose
    /// session's block is `block` or lies below it, whom a Join member or viewer checks before it
    /// speaks to it (plans/hosted_blocks.md), and, for a block above what the caller views, of
    /// whoever signs below on authority from it. A host is proved nowhere else, so a member of
    /// one session learns nothing of its host's records on unrelated blocks. Served whole, only
    /// the entries after `held` when the log holds it (`access::served_log`).
    pub async fn access_log(
        &self,
        identity: Identity,
        block: Uuid,
        hosts: Vec<(Uuid, Uuid)>,
        held: Option<be_block_protocol::HeldEntry>,
    ) -> Result<be_block_protocol::AccessLogPage, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            let seen = visibility(identity);
            let viewed = graph.contains(block) && seen.can_view(graph, block);
            let joins = graph.contains(block) && seen.inherited(graph, block).can_join();
            let below = !viewed && seen.reads_below(graph, block);
            if !viewed && !joins && !below {
                return Err(refusal(graph, &seen, block, "visible"));
            }
            let whole = viewed && seen.served_whole(graph, block);
            let reader = (!whole).then_some(identity.account);
            // Above what the caller views: whoever signs below on authority from it, and whoever
            // sealed the caller a key, who may hold Edit here (`KeyBook::vouch`).
            let mut also = match below {
                false => Vec::new(),
                true => {
                    let mut also: Vec<Uuid> =
                        crate::access::inherited_signers(database, identity.organization, block)?
                            .into_iter()
                            .filter(|(below, _)| {
                                graph.contains(*below)
                                    && (seen.can_view(graph, *below)
                                        || seen.reads_below(graph, *below))
                            })
                            .map(|(_, signer)| signer)
                            .collect();
                    also.extend(crate::access::all_sealers(
                        database,
                        identity.organization,
                        identity.account,
                    )?);
                    also
                }
            };
            also.extend(
                hosts
                    .into_iter()
                    .filter(|(session, _)| {
                        *session == block || graph.ancestors(*session).any(|above| above == block)
                    })
                    .map(|(_, host)| host),
            );
            crate::access::served_log(
                database,
                be_block_keys::LogKind::Block,
                identity.organization,
                block,
                reader,
                also,
                held,
            )
        })
        .await
    }

    /// A publish with no commit: sets `block`'s metadata and references while its head is
    /// `expected`, which stays the head, answering the block and the blocks it started or
    /// stopped referencing; else the head, as a stale publish is rejected. `references` is a
    /// change from what `expected` holds, the whole list with none.
    pub async fn describe(
        &self,
        identity: Identity,
        block: Uuid,
        expected: Option<CommitId>,
        metadata: Vec<u8>,
        references: ReferenceChange,
    ) -> Result<Result<(BlockSummary, Vec<Uuid>), Option<CommitId>>, ServerError> {
        crate::limits::at_most("metadata", metadata.len(), MAX_METADATA_BYTES)?;
        crate::limits::references(&references.removed)?;
        if references.added.len() > crate::limits::MAX_REFERENCES {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                format!(
                    "a block may reference at most {} blocks",
                    crate::limits::MAX_REFERENCES
                ),
            ));
        }
        self.with_graph(identity.organization, move |_, graph, database| {
            require_edit(graph, &visibility(identity), block)?;
            let head = graph.head(block);
            if head != expected {
                return Ok(Err(head));
            }
            let listed = match expected {
                Some(_) => graph.references(block),
                None => Vec::new(),
            };
            let references = references.apply(&listed).ok_or_else(|| {
                ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "a reference change must be as `ReferenceChange::between` makes it".into(),
                )
            })?;
            crate::limits::references(&references)?;
            require_keeping_settings(graph, &visibility(identity), block, &references)?;
            require_one_index(graph, block, &references)?;
            let relinked = relinked(graph, block, &references);
            let organization = identity.organization.to_string();
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            transaction.execute(
                "UPDATE blocks SET metadata = ?3, version = version + 1 WHERE organization_id = ?1 AND id = ?2",
                params![organization, block.to_string(), metadata],
            )?;
            let mut touched = vec![block];
            touched.extend(relinked.iter().flatten().copied());
            crate::changes::touch(&transaction, graph, identity.organization, &touched)?;
            if relinked.is_some() {
                let before = graph.references(block);
                change_edges(&transaction, &organization, block, &before, &references)?;
            }
            transaction.commit()?;
            graph.set_references(block, &references)?;
            graph.set_metadata(block, metadata)?;
            graph.touch(block)?;
            let summary = summary(graph, identity, block).ok_or(ServerError::Corrupt)?;
            Ok(Ok((summary, relinked.unwrap_or_default())))
        })
        .await
    }

    /// The organization's sequence now, and the blocks on the paths of `blocks` (each, and
    /// every block above it; with `subtree`, every block below them too), and `above` (blocks
    /// that were above them before a move), whose followers a change of `blocks` advanced
    /// (`WatchHub::followers`).
    pub(crate) async fn advanced(
        &self,
        organization: Uuid,
        blocks: Vec<Uuid>,
        subtree: bool,
        above: Vec<Uuid>,
    ) -> Result<(u64, Vec<Uuid>), ServerError> {
        self.with_graph(organization, move |_, graph, database| {
            let mut path: BTreeSet<Uuid> = above.into_iter().collect();
            for block in blocks {
                if subtree {
                    path.extend(graph.subtree(block));
                }
                if path.insert(block) || subtree {
                    path.extend(graph.ancestors(block));
                }
            }
            Ok((
                crate::changes::current(database, organization)?,
                path.into_iter().collect(),
            ))
        })
        .await
    }

    /// Whether each caller may view the block paired with it.
    pub(crate) async fn viewing(
        &self,
        organization: Uuid,
        asked: Vec<(Identity, Uuid)>,
    ) -> Result<Vec<bool>, ServerError> {
        self.with_graph(organization, move |_, graph, _| {
            Ok(asked
                .into_iter()
                .map(|(identity, block)| visibility(identity).can_view(graph, block))
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

    /// The summaries of the first of `blocks` that fit in `room` bytes (the first always does),
    /// as a listing has them: none for a block the caller may not know of or that is gone.
    pub async fn read_summaries(
        &self,
        identity: Identity,
        blocks: Vec<Uuid>,
        mut room: usize,
    ) -> Result<Vec<Option<BlockSummary>>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, _| {
            let seen = visibility(identity);
            let mut summaries = Vec::new();
            for block in blocks {
                let summary = summary_with(graph, &seen, block)
                    .filter(|summary| summary.access.can_know_exists());
                let size = be_block_protocol::encoded_len(&summary);
                if size > room && !summaries.is_empty() {
                    break;
                }
                room = room.saturating_sub(size);
                summaries.push(summary);
            }
            Ok(summaries)
        })
        .await
    }

    /// The summaries of the first of `blocks` that fit in `room` bytes (the first always does),
    /// and the objects of their heads but those `held`.
    pub async fn read_contents(
        &self,
        identity: Identity,
        blocks: Vec<Uuid>,
        held: Vec<CommitId>,
        mut room: usize,
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
                let size = be_block_protocol::encoded_len(&summary);
                if size > room && !summaries.is_empty() {
                    break;
                }
                room = room.saturating_sub(size);
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
        trim: Option<crate::journal::Asked>,
        log_through: Option<be_block_protocol::LogHead>,
    ) -> Result<PublishOutcome, ServerError> {
        crate::limits::at_most("metadata", metadata.len(), MAX_METADATA_BYTES)?;
        if let Some(crate::journal::Asked { base, .. }) = &trim {
            if base.commit != Some(commit) {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "a journal base names the commit published".into(),
                ));
            }
            crate::limits::at_most(
                "a journal's base",
                base.state.len(),
                crate::journal::MAX_BASE_STATE_BYTES,
            )?;
        }
        // Checked before the graph is taken, so no change too large to keep holds it up.
        crate::limits::references(&references.removed)?;
        if references.added.len() > crate::limits::MAX_REFERENCES {
            return Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                format!(
                    "a block may reference at most {} blocks",
                    crate::limits::MAX_REFERENCES
                ),
            ));
        }
        let latest = be_block_commit::now_milliseconds().saturating_add(MAX_COMMIT_AHEAD);
        if time > latest {
            return Err(ServerError::Refused(
                ErrorCode::CommitAhead,
                format!("a commit's time may be at most {latest}, not {time}: is this clock fast?"),
            ));
        }
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
                // A commit names only a head its block's signed log had.
                if let Some(through) = &log_through
                    && !crate::log::holds_prefix(database, identity.organization, block, through)?
                {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        format!(
                            "the commit names a head of the block's log ({} entries) that the log never had",
                            through.size
                        ),
                    ));
                }
                // A trim with no new commit: the head is the base's commit already. Only the
                // lease holder trims so, held as the holder while the trim is written; anyone
                // else's publish goes through and trims nothing.
                if current == Some(commit) {
                    let trimmed = match trim {
                        Some(asked) => match (asked.holding)() {
                            Some(_holding) => {
                                let transaction =
                                    be_block_sqlite_schema::write_transaction(database)?;
                                let trimmed = crate::journal::rebase(
                                    &transaction,
                                    identity.organization,
                                    block,
                                    &crate::journal::Seal {
                                        tag: asked.tag,
                                        run: asked.run,
                                        base: asked.base,
                                        new_commit: false,
                                    },
                                )?;
                                transaction.commit()?;
                                trimmed
                            }
                            None => None,
                        },
                        None => None,
                    };
                    return Ok(PublishOutcome::Published(commit, Vec::new(), trimmed));
                }
                require_descends(expected, &parents)?;
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
                if graph
                    .get(block)
                    .is_some_and(|node| node.content_type == SEARCH_INDEX_TYPE)
                {
                    // The base's total in one query, then only what the change moves: the
                    // objects it adds that the base lacks, and those it drops that the base
                    // held and it does not add back (`ObjectChange::apply` keeps those). An
                    // object with no size recorded counts as nothing, so the total errs low
                    // only for objects the server does not hold, which the publish refuses.
                    let base_set: BTreeSet<Hash> = base.iter().copied().collect();
                    let added_set: BTreeSet<Hash> = change.added.iter().copied().collect();
                    let added: Vec<Hash> = std::iter::once(commit.hash())
                        .chain(change.added.iter().copied())
                        .filter(|hash| !base_set.contains(hash))
                        .collect();
                    let removed: Vec<Hash> =
                        expected
                            .map(CommitId::hash)
                            .into_iter()
                            .chain(change.removed.iter().copied().filter(|hash| {
                                base_set.contains(hash) && !added_set.contains(hash)
                            }))
                            .collect();
                    let base_bytes = match expected {
                        Some(expected) => {
                            store.commit_bytes(database, identity.organization, block, expected)?
                        }
                        None => 0,
                    };
                    let bytes = base_bytes
                        .saturating_add(store.stored_bytes(database, &added)?)
                        .saturating_sub(store.stored_bytes(database, &removed)?);
                    if bytes > store.max_index_bytes() {
                        return Err(crate::limits::too_many(
                            "a search index's bytes",
                            store.max_index_bytes(),
                        ));
                    }
                }
                // A first commit lists every reference, replacing any its block was made with.
                let listed = match expected {
                    Some(_) => graph.references(block),
                    None => Vec::new(),
                };
                let references = references.apply(&listed).ok_or_else(|| {
                    ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "a reference change must be as `ReferenceChange::between` makes it".into(),
                    )
                })?;
                crate::limits::references(&references)?;
                require_keeping_settings(graph, &visibility(identity), block, &references)?;
                require_one_index(graph, block, &references)?;
                let relinked = relinked(graph, block, &references);
                let transaction = be_block_sqlite_schema::write_transaction(database)?;
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
                // The blocks it started or stopped referencing changed for whoever knows of
                // them through it.
                let mut touched = vec![block];
                touched.extend(relinked.iter().flatten().copied());
                crate::changes::touch(&transaction, graph, identity.organization, &touched)?;
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
                // A seal trims the journal only as its commit wins the head, in one transaction.
                let trimmed = match trim {
                    Some(asked) => crate::journal::rebase(
                        &transaction,
                        identity.organization,
                        block,
                        &crate::journal::Seal {
                            tag: asked.tag,
                            run: asked.run,
                            base: asked.base,
                            new_commit: true,
                        },
                    )?,
                    None => None,
                };
                transaction.commit()?;
                graph.set_references(block, &references)?;
                graph.set_head(block, commit)?;
                graph.set_metadata(block, metadata)?;
                graph.touch(block)?;
                store.objects().remove_freed(&freed);
                Ok(PublishOutcome::Published(
                    commit,
                    relinked.unwrap_or_default(),
                    trimmed,
                ))
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

    /// Whether the caller holds each of `hashes`, as far as it may know: an object it neither
    /// uploaded nor may read is not held by it whether or not someone else stored it.
    pub async fn held_objects(
        &self,
        identity: Identity,
        hashes: Vec<Hash>,
    ) -> Result<Vec<bool>, ServerError> {
        self.confirm_uploads(identity, hashes.clone()).await?;
        self.with_graph(identity.organization, move |store, graph, database| {
            hashes
                .into_iter()
                .map(|hash| store.holdable(graph, database, identity, hash))
                .collect()
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
                let transaction = be_block_sqlite_schema::write_transaction(database)?;
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
                    let transaction = be_block_sqlite_schema::write_transaction(database)?;
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
        record: Vec<u8>,
    ) -> Result<Moved, ServerError> {
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
            let scope_before = match node.parent {
                BlockParent::Block(above) => Some(graph.scope(above)),
                BlockParent::Root | BlockParent::Detached => None,
            };
            let viewing = match parent {
                BlockParent::Detached => None,
                _ => Some(crate::keys::viewing_below(
                    database,
                    graph,
                    identity.organization,
                    block,
                )?),
            };
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            crate::access::append_parent(
                &transaction,
                identity.organization,
                block,
                identity.account,
                parent,
                &record,
            )?;
            store.follow_payer(graph, &transaction, identity, block, parent)?;
            // The entry points above it, for `query.tree` to name it to their readers once it
            // has left them.
            let entries_before = crate::changes::entries_above(graph, block);
            let above: Vec<Uuid> = graph.ancestors(block).collect();
            // What leaves with it, read before it moves.
            let leaving: Vec<crate::changes::Departing> = graph
                .subtree(block)
                .into_iter()
                .map(|below| departing(graph, below))
                .collect();
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
            // The block and everything below it lie elsewhere now: they left each entry point
            // above them before that is not above them still, within one tree too, so a reader of
            // a subtree hears of a block moved out of it.
            let entries_after = crate::changes::entries_above(graph, block);
            let left: Vec<Uuid> = entries_before
                .iter()
                .copied()
                .filter(|entry| !entries_after.contains(entry))
                .collect();
            crate::changes::touch_moved(
                &transaction,
                graph,
                identity.organization,
                &leaving,
                &left,
                be_block_commit::now_milliseconds(),
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
            Ok(Moved {
                before,
                lost,
                above,
                scope_before,
            })
        })
        .await
    }

    /// A page of `listing` in the order of block ids: the blocks after `after`, the first whose
    /// summaries fit in `room` bytes (at least one), then the next ids that fit in half that,
    /// and `next` when the listing goes on. Only the page's blocks are summarized.
    pub(crate) async fn list_page(
        &self,
        identity: Identity,
        listing: Listing,
        after: Option<Uuid>,
    ) -> Result<Listed, ServerError> {
        self.with_graph(identity.organization, move |store, graph, database| {
            let seen = visibility(identity);
            let ids = listing_ids(graph, &seen, identity, listing);
            let start = after.map_or(0, |after| ids.partition_point(|id| *id <= after));
            let mut page = Listed {
                sequence: crate::changes::current(database, identity.organization)?,
                blocks: Vec::new(),
                rest: Vec::new(),
                next: None,
            };
            let mut room = store.listing_page_bytes();
            let mut rest_room = room / 2;
            let mut ids = ids[start..].iter().copied().peekable();
            let mut last = None;
            // The first blocks, summarized while their summaries fit (at least one).
            while let Some(id) = ids.peek().copied() {
                let summary = summary_with(graph, &seen, id).ok_or(ServerError::Corrupt)?;
                let size = be_block_protocol::encoded_len(&summary);
                if size > room && !page.blocks.is_empty() {
                    break;
                }
                room = room.saturating_sub(size);
                page.blocks.push(summary);
                last = Some(id);
                ids.next();
            }
            // Then ids, with no summary made.
            while let Some(id) = ids.peek().copied() {
                let size = be_block_protocol::encoded_len(&id);
                if size > rest_room {
                    break;
                }
                rest_room -= size;
                page.rest.push(id);
                last = Some(id);
                ids.next();
            }
            if ids.peek().is_some() {
                page.next = last;
            }
            Ok(page)
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

    /// What changed after `since` in the tree of `entry` (`changes.rs`), from `after`: the
    /// blocks in the tree now that changed, and the blocks that left that tree (moved out, or
    /// deleted for good from it) since, and are not in it. `entry` is any block the caller may
    /// view, though departures are written only for entry points (`changes::entries_above`). A
    /// page ends at `limit` blocks of either kind, at `MAX_PAGE_SUMMARY_BYTES` of summaries, or
    /// at `MAX_SCANNED` changes looked at (the changes of other trees count), with `next` set.
    pub async fn changed_since(
        &self,
        identity: Identity,
        entry: Uuid,
        after: ChangeCursor,
        limit: u32,
    ) -> Result<ChangedPage, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            let seen = visibility(identity);
            require(graph, &seen, entry, Access::View, "readable")?;
            let root = entry;
            let organization = identity.organization.to_string();
            let limit = (limit as usize).clamp(1, MAX_CHANGES_PER_PAGE);
            let sequence: i64 = database.query_row(
                "SELECT change_sequence FROM organizations WHERE id = ?1",
                params![organization],
                |row| row.get(0),
            )?;
            let mut summaries = Vec::new();
            let mut removed = Vec::new();
            // What changed in the tree and what left it, in one order: blocks held, and the
            // departures from this tree (moved out, trashed, or deleted for good while in it).
            let mut rows = database.prepare_cached(
                "SELECT id, changed, 0 AS departed FROM blocks
                 WHERE organization_id = ?1 AND (changed > ?2 OR (changed = ?2 AND id > ?3))
                 UNION ALL
                 SELECT block_id, changed, 1 FROM block_departures
                 WHERE organization_id = ?1 AND entry = ?4 AND (owner IS NULL OR owner = ?5)
                   AND (changed > ?2 OR (changed = ?2 AND block_id > ?3))
                 ORDER BY 2, 1",
            )?;
            let mut rows = rows.query(params![
                organization,
                after.changed as i64,
                after.block.to_string(),
                root.to_string(),
                identity.account.to_string(),
            ])?;
            // A page that stops on a block of another tree must not name it in its cursor:
            // it reads on to the end of that sequence (one write's blocks) and ends on a
            // cursor that names no block.
            let (mut scanned, mut next, mut summary_bytes) = (0, None, 0);
            let mut finishing: Option<i64> = None;
            while let Some(row) = rows.next()? {
                let (id, changed, departed): (String, i64, bool) =
                    (row.get(0)?, row.get(1)?, row.get(2)?);
                if let Some(sequence) = finishing
                    && changed != sequence
                {
                    break;
                }
                let Ok(id) = id.parse::<Uuid>() else {
                    continue;
                };
                scanned += 1;
                let inside = graph.contains(id)
                    && (id == root || graph.ancestors(id).any(|above| above == root));
                let mut ours = false;
                if !departed && inside {
                    if seen.can_view(graph, id)
                        && let Some(summary) = summary_with(graph, &seen, id)
                    {
                        summary_bytes += be_block_protocol::encoded_len(&summary);
                        summaries.push(summary);
                        ours = true;
                    }
                } else if departed && !inside {
                    removed.push(id);
                    ours = true;
                }
                // Finishing, the first block of this tree ends the page, which may name it.
                if finishing.is_some() && ours {
                    next = Some(ChangeCursor {
                        changed: changed as u64,
                        block: id,
                    });
                    break;
                }
                if finishing.is_none()
                    && (summaries.len() >= limit
                        || summary_bytes >= MAX_PAGE_SUMMARY_BYTES
                        || removed.len() >= limit
                        || scanned >= MAX_SCANNED)
                {
                    match ours {
                        true => {
                            next = Some(ChangeCursor {
                                changed: changed as u64,
                                block: id,
                            });
                            break;
                        }
                        false => {
                            finishing = Some(changed);
                            next = Some(ChangeCursor {
                                changed: changed as u64,
                                block: Uuid::max(),
                            });
                        }
                    }
                }
            }
            Ok(ChangedPage {
                summaries,
                removed,
                next,
                sequence: sequence as u64,
            })
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

    pub async fn set_access(
        &self,
        identity: Identity,
        block: Uuid,
        account: Uuid,
        access: Access,
        record: Option<Vec<u8>>,
    ) -> Result<Option<BTreeSet<Uuid>>, ServerError> {
        if let Some(record) = record
            .as_ref()
            .filter(|record| crate::access::is_owner_record(record))
        {
            return self
                .transfer_owner(identity, block, account, access, record.clone())
                .await;
        }
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
                if graph.settings_owner(block).is_none()
                    || !matches!(access, Access::None | Access::View)
                {
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
            // A grant to the server is a standing exception no device seals to by grant, so it
            // is a row alone. Any other grant is a record in the block's access log (opening a
            // kind to anyone inside, an opening record), as is the revocation of one the log
            // names.
            let standing = account == crate::organization::server_account(database)?;
            let record = match (standing, record) {
                (true, Some(_)) => {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "a standing grant takes no record".into(),
                    ));
                }
                (true, None) => None,
                (false, Some(record)) => Some(record),
                (false, None)
                    if access == Access::None
                        && !crate::access::names_grant(
                            database,
                            identity.organization,
                            block,
                            account,
                        )? =>
                {
                    None
                }
                (false, None) => {
                    return Err(ServerError::Refused(
                        ErrorCode::InvalidRequest,
                        "a grant is set by the record its granter signed".into(),
                    ));
                }
            };
            // A grant that changes nothing writes nothing and tells no one: a device that shares
            // again what it may already have shared (the organization's settings, on each
            // administrator's connect) costs its audience nothing. Its record still goes in the
            // log when the log lacks it (a contact link's grant, applied on accepting, is signed
            // when its maker's device finishes the link).
            let granted = graph
                .get(block)
                .map(|node| node.grants.get(&account).copied().unwrap_or(Access::None));
            if granted == Some(access) {
                if let Some(record) = record {
                    crate::access::append_grant(
                        database,
                        identity.organization,
                        block,
                        identity.account,
                        account,
                        access,
                        &record,
                    )?;
                }
                return Ok(None);
            }
            let before = graph.audience(block);
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            revoke_or_grant(
                graph,
                &transaction,
                identity.organization,
                block,
                account,
                access,
            )?;
            if let Some(record) = record {
                crate::access::append_grant(
                    &transaction,
                    identity.organization,
                    block,
                    identity.account,
                    account,
                    access,
                    &record,
                )?;
            }
            let forgotten = crate::store::forget_orphaned_block_agents(
                graph,
                &transaction,
                identity.organization,
            )?;
            transaction.commit()?;
            if !forgotten.is_empty() {
                store.sessions_changed();
            }
            Ok(Some(before))
        })
        .await
    }

    /// `set-access` carrying an owner record: ownership of `block` moves from its owner, the
    /// caller, to `account` (guides/blocks/keys.md, "Owner records"). The new owner holds what
    /// authorship gives and the former keeps Edit (`be_block_keys::FORMER_OWNER`), in the log as in
    /// the graph. Answers who could view the block before, as a grant does.
    async fn transfer_owner(
        &self,
        identity: Identity,
        block: Uuid,
        account: Uuid,
        access: Access,
        record: Vec<u8>,
    ) -> Result<Option<BTreeSet<Uuid>>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            require_manage(graph, &visibility(identity), block)?;
            let node = graph.get(block).ok_or(ServerError::Corrupt)?;
            if node.author != identity.account {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("only the owner of block {block} gives its ownership away"),
                ));
            }
            let fixed = node.owner_only
                || graph.owner_only_root(block).is_some()
                || node.is_search_index()
                || node.is_edit_kind()
                || node.unfiled()
                || be_block::holds_owner_only(node.content_type);
            if fixed {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    format!("block {block} belongs to what holds it, so its ownership never moves"),
                ));
            }
            if access != Access::Manage
                || account == identity.account
                || account == ANYONE_INSIDE
                || account == crate::organization::server_account(database)?
            {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "ownership moves to another member, who manages the block".into(),
                ));
            }
            if !crate::store::is_member_of(database, account, identity.organization)? {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "that account is not a member of this organization".into(),
                ));
            }
            if !crate::contacts::is_contact(database, identity.account, account)? {
                return Err(ServerError::Refused(
                    ErrorCode::NotAContact,
                    "give ownership only to your contacts: send them a contact link first".into(),
                ));
            }
            // The block becomes theirs to keep, so it counts against their bound as making one
            // would: no one fills an account up by giving it blocks. Checked last, so an owner
            // learns how many blocks an account holds only of a contact.
            if graph.authored(account) >= MAX_BLOCKS_PER_ACCOUNT {
                return Err(crate::limits::too_many(
                    "blocks one account makes in an organization",
                    MAX_BLOCKS_PER_ACCOUNT,
                ));
            }
            let before = graph.audience(block);
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            crate::access::append_owner(
                &transaction,
                identity.organization,
                block,
                identity.account,
                account,
                &record,
            )?;
            graph.set_author(block, account)?;
            transaction.execute(
                "UPDATE blocks SET author = ?3 WHERE organization_id = ?1 AND id = ?2",
                params![
                    identity.organization.to_string(),
                    block.to_string(),
                    account.to_string()
                ],
            )?;
            // The log grants the new owner Manage, as the row does: authorship alone gives it
            // only while they can view the parent, which a block given away from somewhere
            // they cannot see would not be.
            write_grant(
                graph,
                &transaction,
                identity.organization,
                block,
                account,
                Access::Manage,
            )?;
            write_grant(
                graph,
                &transaction,
                identity.organization,
                block,
                identity.account,
                Access::Edit,
            )?;
            transaction.commit()?;
            Ok(Some(before))
        })
        .await
    }

    /// The accounts that may view `block`, its author among them while it may, with the names
    /// they gave the server, for a caller that may view it (`query.names`).
    pub async fn list_names(
        &self,
        identity: Identity,
        block: Uuid,
    ) -> Result<Vec<AccountName>, ServerError> {
        self.with_graph(identity.organization, move |_, graph, database| {
            require(
                graph,
                &visibility(identity),
                block,
                Access::View,
                "viewable",
            )?;
            let node = graph.get(block).ok_or(ServerError::Corrupt)?;
            let listed: BTreeSet<Uuid> = graph
                .audience(block)
                .into_iter()
                .chain([node.author])
                .filter(|account| {
                    *account != ANYONE_INSIDE && graph.effective_access(block, *account).can_view()
                })
                // A guest learns no member of the organization (guides/blocks/server.md): it is
                // named only itself.
                .filter(|account| {
                    identity.role != OrganizationRole::Guest || *account == identity.account
                })
                .collect();
            let mut statement = database.prepare_cached(
                "SELECT accounts.display_name
                 FROM memberships
                 JOIN accounts ON accounts.id = memberships.account_id
                 WHERE memberships.organization_id = ?1 AND memberships.account_id = ?2",
            )?;
            let mut names = Vec::new();
            for account in listed {
                let name = statement
                    .query_row(
                        params![identity.organization.to_string(), account.to_string()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?;
                if let Some(display_name) = name {
                    names.push(AccountName {
                        account,
                        display_name,
                    });
                }
            }
            Ok(names)
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
            let linked: BTreeSet<Uuid> =
                crate::access::linked(database, identity.organization, block)?
                    .into_iter()
                    .map(|(account, _)| account)
                    .collect();
            let mut statement = database.prepare_cached(
                "SELECT COALESCE(accounts.email, ''), accounts.display_name, memberships.role
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
                        through_link: false,
                    });
                    continue;
                }
                let Some((email, display_name, role)) = row else {
                    continue;
                };
                let role = crate::store::decode_role(&role)?;
                if company && role == OrganizationRole::Administrator && account != identity.account
                {
                    continue;
                }
                let granted = node.grants.get(&account).copied();
                let (effective, source) = match role {
                    OrganizationRole::Administrator => (Access::Manage, None),
                    OrganizationRole::Editor | OrganizationRole::Guest => {
                        match graph.inherited_from(block, account) {
                            (level, source) if level.can_know_exists() => (level, source),
                            _ => (graph.effective_access(block, account), None),
                        }
                    }
                };
                let inherited = match (role, granted) {
                    (OrganizationRole::Administrator, _) | (_, None) => effective,
                    (OrganizationRole::Editor | OrganizationRole::Guest, Some(_)) => {
                        graph.inherited_above(block, account)
                    }
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
                    through_link: linked.contains(&account),
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
    // Who may view everything below it moved, and each summary says what its reader may do.
    let below = graph.subtree(block);
    crate::changes::touch(database, graph, organization, &below)?;
    // A revocation the server makes on its own (`remove-contact`, `delete-agent`) of a grant
    // the block's log names waits for a device to sign it (`KeyWork::unsigned_revocations`);
    // a signed one, appended after, clears it (`access::append_grant`).
    if access == Access::None && crate::access::names_grant(database, organization, block, account)?
    {
        database.execute(
            "INSERT OR IGNORE INTO unsigned_revocations (organization_id, block_id, account_id)
             VALUES (?1, ?2, ?3)",
            params![
                organization.to_string(),
                block.to_string(),
                account.to_string()
            ],
        )?;
    }
    let organization_id = organization;
    let organization = organization.to_string();
    database.execute(
        "UPDATE blocks SET version = version + 1 WHERE organization_id = ?1 AND id = ?2",
        params![organization, block.to_string()],
    )?;
    // (Who may view the block changed, which a reader of its tree is told of with the rest.)
    match encode_access(access) {
        None => {
            database.execute(
                "DELETE FROM block_access
                 WHERE organization_id = ?1 AND block_id = ?2 AND account_id = ?3",
                params![organization, block.to_string(), account.to_string()],
            )?;
            // A guest is there only through what links granted it: with its last grant gone,
            // whatever took it, it leaves.
            crate::share_links::drop_idle_guest(database, organization_id, account)?;
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
    let had: HashSet<Uuid> = before.iter().copied().collect();
    let has: HashSet<&Uuid> = references.iter().collect();
    let added = references
        .iter()
        .filter(|reference| !had.contains(reference))
        .copied()
        .collect::<Vec<_>>();
    Some(
        before
            .into_iter()
            .filter(|reference| !has.contains(reference))
            .chain(added)
            .collect(),
    )
}

/// Positions of a block's edges are spaced this far apart, so a reference put in between two
/// others takes a free position instead of moving the rest.
const EDGE_SPACING: i64 = 1 << 20;

fn store_edges(
    transaction: &rusqlite::Connection,
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
    transaction: &rusqlite::Connection,
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
    transaction: &rusqlite::Connection,
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
            (Some(_), Some(_)) => {
                store_edges(transaction, organization, block, after)?;
                return Ok(());
            }
        };
        for (offset, reference) in after[index..end].iter().enumerate() {
            let offset = i64::try_from(offset).map_err(|_| ServerError::Corrupt)?;
            placed.push((*reference, first + step * offset));
        }
        low = placed.last().map(|(_, position)| *position);
        index = end;
    }
    insert_edges(transaction, organization, block, placed)?;
    Ok(())
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

/// The blocks of `listing` the caller may know of, as a listing
/// holds them, in the order of their ids.
fn listing_ids(
    graph: &BlockGraph,
    seen: &Visibility,
    identity: Identity,
    listing: Listing,
) -> Vec<Uuid> {
    let knows =
        |block: &Uuid| graph.contains(*block) && seen.access(graph, *block).can_know_exists();
    let mut ids: Vec<Uuid> = match listing {
        Listing::Shared if seen.administrator => Vec::new(),
        Listing::Shared => graph
            .shared(identity.account)
            .into_iter()
            .filter(|block| graph.contains(*block) && seen.access(graph, *block).can_join())
            .collect(),
        Listing::Children(parent) => {
            let settings = crate::organization::settings_of(identity.organization);
            graph
                .children(parent)
                .into_iter()
                .filter(|block| *block != settings)
                .filter(knows)
                .collect()
        }
        Listing::Deleted => match seen.candidates(graph) {
            None => graph.deleted_roots(),
            Some(blocks) => blocks
                .into_iter()
                .filter(|block| graph.contains(*block) && !graph.holds(*block))
                .collect(),
        }
        .into_iter()
        .filter(knows)
        .collect(),
        Listing::Backrefs(block) => graph.backrefs(block).into_iter().filter(knows).collect(),
        Listing::References(block) if !seen.can_view(graph, block) => Vec::new(),
        Listing::References(block) => graph.references(block).into_iter().filter(knows).collect(),
        Listing::OpenSettings(block) => {
            let mut found: BTreeMap<Uuid, Uuid> = BTreeMap::new();
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
                    if seen.access(graph, kind).can_view() {
                        found.insert(node.content_type, kind);
                    }
                }
            }
            found.into_values().collect()
        }
    };
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// A block and its ancestors: the blocks whose sizes change with its own.
pub(crate) fn chain(graph: &BlockGraph, block: Uuid) -> Vec<Uuid> {
    std::iter::once(block)
        .chain(graph.ancestors(block))
        .collect()
}
