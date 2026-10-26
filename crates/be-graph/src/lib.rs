use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt,
};

use be_commit::CommitId;
use be_store::Hash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod access;

pub use access::Access;

pub const SETTINGS_TYPE: Uuid = Uuid::from_u128(0x7365_7474_696e_6773_2d62_6c6f_636b_3032);

pub const ANYONE_INSIDE: Uuid = Uuid::from_u128(0x616e_796f_6e65_2d69_6e73_6964_652d_3031);

pub const PREFIXES_TYPE: Uuid = Uuid::from_u128(0x7072_6566_6978_6573_2d6b_696e_642d_3031);

pub const TAGS_TYPE: Uuid = Uuid::from_u128(0x7461_6773_2d6b_696e_642d_6265_3364_3031);

pub const EDIT_KINDS: [Uuid; 2] = [PREFIXES_TYPE, TAGS_TYPE];

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum BlockParent {
    #[default]
    Detached,
    Root,
    Block(Uuid),
}

impl BlockParent {
    pub const fn block(self) -> Option<Uuid> {
        match self {
            Self::Block(id) => Some(id),
            Self::Detached | Self::Root => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlockNode {
    pub content_type: Uuid,
    pub author: Uuid,
    pub parent: BlockParent,
    pub head: Option<CommitId>,
    pub references: Vec<Uuid>,
    pub grants: BTreeMap<Uuid, Access>,
    pub metadata: Vec<u8>,
    pub version: u64,
    #[serde(default)]
    pub key: Option<Uuid>,
}

impl BlockNode {
    pub fn new(content_type: Uuid, author: Uuid, parent: BlockParent) -> Self {
        Self {
            content_type,
            author,
            parent,
            head: None,
            references: Vec::new(),
            grants: BTreeMap::new(),
            metadata: Vec::new(),
            version: 0,
            key: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveRefusal {
    MovesWorkspace,
    RaisesOwnAccess,
}

impl fmt::Display for MoveRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MovesWorkspace => {
                "moving a workspace, or a block to another workspace, needs Manage on the block"
            }
            Self::RaisesOwnAccess => {
                "this move would give you more access to the block than you have, which needs Manage on it"
            }
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum GraphError {
    NotFound(Uuid),
    AlreadyExists(Uuid),
    ParentCycle,
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(formatter, "block {id} is not in the graph"),
            Self::AlreadyExists(id) => write!(formatter, "block {id} already exists"),
            Self::ParentCycle => formatter.write_str("the requested parent would form a cycle"),
        }
    }
}

impl Error for GraphError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockGraph {
    blocks: BTreeMap<Uuid, BlockNode>,
    backrefs: BTreeMap<Uuid, BTreeSet<Uuid>>,
    children: HashMap<BlockParent, BTreeSet<Uuid>>,
    settings: HashMap<Uuid, BTreeSet<Uuid>>,
    entries: HashMap<Uuid, BTreeSet<Uuid>>,
    loose: BTreeSet<Uuid>,
    revision: u64,
}

impl BlockNode {
    fn holds_settings(&self) -> bool {
        holds_settings(self.content_type, self.parent)
    }

    pub fn is_block_settings(&self) -> bool {
        self.content_type == SETTINGS_TYPE && matches!(self.parent, BlockParent::Block(_))
    }

    pub fn is_edit_kind(&self) -> bool {
        EDIT_KINDS.contains(&self.content_type) && matches!(self.parent, BlockParent::Block(_))
    }

    pub fn is_open_inside(&self) -> bool {
        self.grants
            .get(&ANYONE_INSIDE)
            .is_some_and(|level| level.can_view())
    }

    pub fn sets_access(&self) -> bool {
        !self.is_block_settings() && self.grants.keys().any(|account| *account != ANYONE_INSIDE)
    }
}

pub fn holds_settings(content_type: Uuid, parent: BlockParent) -> bool {
    (content_type == SETTINGS_TYPE || EDIT_KINDS.contains(&content_type))
        && matches!(parent, BlockParent::Block(_))
}

pub fn created_access(
    content_type: Uuid,
    parent: BlockParent,
    parent_access: Access,
    below_settings: bool,
) -> Access {
    let author = Uuid::nil();
    let viewable = parent.block().is_none() || parent_access.can_view();
    level_below(
        &BlockNode::new(content_type, author, parent),
        author,
        parent_access,
        viewable,
        below_settings,
    )
}

fn level_below(
    node: &BlockNode,
    account: Uuid,
    parent: Access,
    parent_viewable: bool,
    in_settings: bool,
) -> Access {
    let granted = node.grants.get(&account).copied().unwrap_or_default();
    let level = match (
        in_settings,
        node.author == account && parent_viewable,
        parent.can_view(),
    ) {
        (true, _, true) => parent,
        (true, _, false) => granted.min(Access::KnowExists),
        (false, true, _) => Access::Manage,
        (false, false, true) => granted.max(parent),
        (false, false, false) => granted,
    };
    let ceiling = match (node.is_block_settings(), node.is_edit_kind()) {
        (true, _) if parent.can_manage() => Access::Manage,
        (true, _) => Access::View,
        (false, true) => parent.max(Access::View),
        (false, false) => return level,
    };
    match parent.can_view() {
        true => level.min(ceiling),
        false => level.min(Access::KnowExists),
    }
}

fn unindex<K: Eq + std::hash::Hash>(index: &mut HashMap<K, BTreeSet<Uuid>>, key: K, id: Uuid) {
    if let Some(set) = index.get_mut(&key) {
        set.remove(&id);
        if set.is_empty() {
            index.remove(&key);
        }
    }
}

impl BlockGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn advance_to(&mut self, revision: u64) {
        self.revision = self.revision.max(revision);
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn get(&self, id: Uuid) -> Option<&BlockNode> {
        self.blocks.get(&id)
    }

    pub fn contains(&self, id: Uuid) -> bool {
        self.blocks.contains_key(&id)
    }

    pub fn ids(&self) -> Vec<Uuid> {
        self.blocks.keys().copied().collect()
    }

    pub fn insert(&mut self, id: Uuid, node: BlockNode) -> Result<(), GraphError> {
        if self.blocks.contains_key(&id) {
            return Err(GraphError::AlreadyExists(id));
        }
        if let BlockParent::Block(parent) = node.parent
            && !self.blocks.contains_key(&parent)
        {
            return Err(GraphError::NotFound(parent));
        }
        for reference in &node.references {
            self.backrefs.entry(*reference).or_default().insert(id);
        }
        self.children.entry(node.parent).or_default().insert(id);
        if let (true, BlockParent::Block(owner)) = (node.content_type == SETTINGS_TYPE, node.parent)
        {
            self.settings.entry(owner).or_default().insert(id);
        }
        self.entries.entry(node.author).or_default().insert(id);
        for account in node.grants.keys() {
            self.entries.entry(*account).or_default().insert(id);
        }
        let references = node.references.clone();
        self.blocks.insert(id, node);
        self.refresh_loose(id);
        for reference in references {
            self.refresh_loose(reference);
        }
        Ok(())
    }

    fn refresh_loose(&mut self, id: Uuid) {
        match self.blocks.contains_key(&id) && !self.holds(id) {
            true => self.loose.insert(id),
            false => self.loose.remove(&id),
        };
    }

    pub fn holds(&self, id: Uuid) -> bool {
        match self.blocks.get(&id).map(|node| node.parent) {
            Some(BlockParent::Root) => true,
            Some(BlockParent::Block(parent)) => self
                .blocks
                .get(&parent)
                .is_some_and(|parent| parent.references.contains(&id)),
            Some(BlockParent::Detached) | None => false,
        }
    }

    pub fn touch(&mut self, id: Uuid) -> Result<u64, GraphError> {
        let node = self.node_mut(id)?;
        node.version += 1;
        Ok(node.version)
    }

    pub fn set_head(&mut self, id: Uuid, head: CommitId) -> Result<(), GraphError> {
        self.node_mut(id)?.head = Some(head);
        Ok(())
    }

    pub fn set_metadata(&mut self, id: Uuid, metadata: Vec<u8>) -> Result<(), GraphError> {
        self.node_mut(id)?.metadata = metadata;
        Ok(())
    }

    pub fn set_key(&mut self, id: Uuid, key: Uuid) -> Result<(), GraphError> {
        self.node_mut(id)?.key = Some(key);
        Ok(())
    }

    pub fn is_scope_root(&self, id: Uuid) -> bool {
        self.blocks.get(&id).is_some_and(|node| {
            !matches!(node.parent, BlockParent::Block(_))
                || node.grants.values().any(|level| level.can_view())
        })
    }

    pub fn scope(&self, id: Uuid) -> Uuid {
        std::iter::once(id)
            .chain(self.ancestors(id))
            .find(|block| self.is_scope_root(*block) || !self.blocks.contains_key(block))
            .unwrap_or(id)
    }

    pub fn head(&self, id: Uuid) -> Option<CommitId> {
        self.blocks.get(&id).and_then(|node| node.head)
    }

    pub fn set_parent(&mut self, id: Uuid, parent: BlockParent) -> Result<(), GraphError> {
        if !self.blocks.contains_key(&id) {
            return Err(GraphError::NotFound(id));
        }
        if let BlockParent::Block(target) = parent {
            if !self.blocks.contains_key(&target) {
                return Err(GraphError::NotFound(target));
            }
            if target == id || self.ancestors(target).any(|ancestor| ancestor == id) {
                return Err(GraphError::ParentCycle);
            }
        }
        let previous = std::mem::replace(&mut self.node_mut(id)?.parent, parent);
        unindex(&mut self.children, previous, id);
        self.children.entry(parent).or_default().insert(id);
        if self
            .blocks
            .get(&id)
            .is_some_and(|node| node.content_type == SETTINGS_TYPE)
        {
            if let BlockParent::Block(owner) = previous {
                unindex(&mut self.settings, owner, id);
            }
            if let BlockParent::Block(owner) = parent {
                self.settings.entry(owner).or_default().insert(id);
            }
        }
        self.refresh_loose(id);
        Ok(())
    }

    pub fn set_references(&mut self, id: Uuid, references: &[Uuid]) -> Result<(), GraphError> {
        let mut ordered = Vec::with_capacity(references.len());
        for reference in references {
            if !ordered.contains(reference) {
                ordered.push(*reference);
            }
        }
        let previous = std::mem::replace(&mut self.node_mut(id)?.references, ordered.clone());
        for reference in &previous {
            if !ordered.contains(reference)
                && let Some(backrefs) = self.backrefs.get_mut(reference)
            {
                backrefs.remove(&id);
                if backrefs.is_empty() {
                    self.backrefs.remove(reference);
                }
            }
        }
        for reference in &ordered {
            self.backrefs.entry(*reference).or_default().insert(id);
        }
        for reference in previous.into_iter().chain(ordered) {
            self.refresh_loose(reference);
        }
        Ok(())
    }

    pub fn references(&self, id: Uuid) -> Vec<Uuid> {
        self.blocks
            .get(&id)
            .map(|node| node.references.clone())
            .unwrap_or_default()
    }

    pub fn backrefs(&self, id: Uuid) -> Vec<Uuid> {
        self.backrefs
            .get(&id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn children(&self, parent: BlockParent) -> Vec<Uuid> {
        self.children
            .get(&parent)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    fn child_ids(&self, parent: Uuid) -> impl Iterator<Item = Uuid> + '_ {
        self.children
            .get(&BlockParent::Block(parent))
            .into_iter()
            .flatten()
            .copied()
    }

    pub fn ancestors(&self, id: Uuid) -> impl Iterator<Item = Uuid> + '_ {
        std::iter::successors(Some(id), |current| {
            self.blocks
                .get(current)
                .and_then(|node| node.parent.block())
        })
        .skip(1)
    }

    pub fn is_live(&self, id: Uuid) -> bool {
        self.blocks.contains_key(&id) && self.deleted_root(id).is_none()
    }

    pub fn deleted_root(&self, id: Uuid) -> Option<Uuid> {
        let mut current = id;
        loop {
            if !self.holds(current) {
                return Some(current);
            }
            match self.blocks.get(&current)?.parent {
                BlockParent::Block(parent) => current = parent,
                BlockParent::Root | BlockParent::Detached => return None,
            }
        }
    }

    pub fn deleted_roots(&self) -> Vec<Uuid> {
        self.loose.iter().copied().collect()
    }

    pub fn subtree(&self, root: Uuid) -> Vec<Uuid> {
        if !self.blocks.contains_key(&root) {
            return Vec::new();
        }
        let mut collected = vec![root];
        let mut next = 0;
        while let Some(id) = collected.get(next).copied() {
            collected.extend(self.child_ids(id));
            next += 1;
        }
        collected
    }

    pub fn detached(&self) -> Vec<Uuid> {
        let mut detached: Vec<Uuid> = self
            .loose
            .iter()
            .flat_map(|root| self.subtree(*root))
            .collect();
        detached.sort_unstable();
        detached.dedup();
        detached
    }

    pub fn remove(&mut self, id: Uuid) -> Option<BlockNode> {
        let node = self.blocks.remove(&id)?;
        for reference in &node.references {
            if let Some(backrefs) = self.backrefs.get_mut(reference) {
                backrefs.remove(&id);
                if backrefs.is_empty() {
                    self.backrefs.remove(reference);
                }
            }
        }
        for referrer in self.backrefs.remove(&id).unwrap_or_default() {
            if let Some(other) = self.blocks.get_mut(&referrer) {
                other.references.retain(|reference| *reference != id);
            }
        }
        unindex(&mut self.children, node.parent, id);
        if let BlockParent::Block(owner) = node.parent {
            unindex(&mut self.settings, owner, id);
        }
        self.settings.remove(&id);
        self.loose.remove(&id);
        for child in self
            .children
            .remove(&BlockParent::Block(id))
            .unwrap_or_default()
        {
            if let Some(other) = self.blocks.get_mut(&child) {
                other.parent = BlockParent::Detached;
            }
            self.children
                .entry(BlockParent::Detached)
                .or_default()
                .insert(child);
            self.loose.insert(child);
        }
        unindex(&mut self.entries, node.author, id);
        for account in node.grants.keys() {
            if *account != node.author {
                unindex(&mut self.entries, *account, id);
            }
        }
        Some(node)
    }

    pub fn grant(&mut self, id: Uuid, account: Uuid, access: Access) -> Result<(), GraphError> {
        let node = self.node_mut(id)?;
        let author = node.author;
        if access == Access::None {
            node.grants.remove(&account);
            if account != author {
                unindex(&mut self.entries, account, id);
            }
        } else {
            node.grants.insert(account, access);
            self.entries.entry(account).or_default().insert(id);
        }
        Ok(())
    }

    pub fn access(&self, id: Uuid, account: Uuid) -> Access {
        let mut current = Some(id);
        let mut best = Access::None;
        while let Some(block) = current {
            let Some(node) = self.blocks.get(&block) else {
                break;
            };
            best = best.max(node.grants.get(&account).copied().unwrap_or_default());
            if best == Access::Manage {
                return best;
            }
            current = node.parent.block();
        }
        best
    }

    pub fn entry_points(&self, account: Uuid) -> Vec<Uuid> {
        self.entries
            .get(&account)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn inherited(&self, id: Uuid, account: Uuid) -> Access {
        self.inherited_from(id, account).0
    }

    pub fn inherited_from(&self, id: Uuid, account: Uuid) -> (Access, Option<Uuid>) {
        let chain: Vec<(Uuid, &BlockNode)> = std::iter::once(id)
            .chain(self.ancestors(id))
            .map_while(|block| Some((block, self.blocks.get(&block)?)))
            .collect();
        let Some((_, top)) = chain.last() else {
            return (Access::None, None);
        };
        let mut above = Access::None;
        let mut source = None;
        let mut parent_viewable = matches!(top.parent, BlockParent::Root | BlockParent::Detached);
        let mut in_settings = top
            .parent
            .block()
            .is_some_and(|parent| self.in_settings(parent));
        for (block, node) in chain.into_iter().rev() {
            let mut level = level_below(node, account, above, parent_viewable, in_settings);
            if !level.can_view() && self.opens_to(block, account) {
                level = Access::View;
            }
            if level > above {
                source = Some(block);
            }
            above = level;
            parent_viewable = above.can_view();
            in_settings = in_settings || node.holds_settings();
        }
        (above, source.filter(|_| above > Access::None))
    }

    pub fn inherited_above(&self, id: Uuid, account: Uuid) -> Access {
        let Some(node) = self.blocks.get(&id) else {
            return Access::None;
        };
        let (above, parent_viewable) = match node.parent {
            BlockParent::Block(parent) if self.blocks.contains_key(&parent) => {
                let level = self.inherited(parent, account);
                (level, level.can_view())
            }
            BlockParent::Block(_) => (Access::None, false),
            BlockParent::Root | BlockParent::Detached => (Access::None, true),
        };
        let mut ungranted = node.clone();
        ungranted.grants.remove(&account);
        let in_settings = node
            .parent
            .block()
            .is_some_and(|parent| self.in_settings(parent));
        let level = level_below(&ungranted, account, above, parent_viewable, in_settings);
        match !level.can_view() && self.opens_to(id, account) {
            true => Access::View,
            false => level,
        }
    }

    pub fn set_inside(&self, id: Uuid) -> usize {
        self.subtree(id)
            .into_iter()
            .skip(1)
            .filter(|block| self.blocks.get(block).is_some_and(BlockNode::sets_access))
            .count()
    }

    pub fn workspace_of(&self, id: Uuid) -> Option<Uuid> {
        let top = std::iter::once(id).chain(self.ancestors(id)).last()?;
        match self.blocks.get(&top)?.parent {
            BlockParent::Root => Some(top),
            BlockParent::Detached | BlockParent::Block(_) => None,
        }
    }

    pub fn inherited_under(&self, id: Uuid, parent: BlockParent, account: Uuid) -> Access {
        let Some(node) = self.blocks.get(&id) else {
            return Access::None;
        };
        let (above, parent_viewable, in_settings) = match parent {
            BlockParent::Block(parent) => {
                let level = self.inherited(parent, account);
                (level, level.can_view(), self.in_settings(parent))
            }
            BlockParent::Root | BlockParent::Detached => (Access::None, true, false),
        };
        let mut moved = node.clone();
        moved.parent = parent;
        level_below(&moved, account, above, parent_viewable, in_settings)
    }

    pub fn move_refusal(
        &self,
        id: Uuid,
        parent: BlockParent,
        account: Uuid,
        access: Access,
    ) -> Option<MoveRefusal> {
        let node = self.blocks.get(&id)?;
        if access.can_manage() {
            return None;
        }
        if node.parent == BlockParent::Root && parent != BlockParent::Root {
            return Some(MoveRefusal::MovesWorkspace);
        }
        let to = match parent {
            BlockParent::Block(target) => self.workspace_of(target),
            BlockParent::Root => Some(id),
            BlockParent::Detached => None,
        };
        if let (Some(from), Some(to)) = (self.workspace_of(id), to)
            && from != to
        {
            return Some(MoveRefusal::MovesWorkspace);
        }
        match self.inherited_under(id, parent, account) > access {
            true => Some(MoveRefusal::RaisesOwnAccess),
            false => None,
        }
    }

    fn in_settings(&self, id: Uuid) -> bool {
        std::iter::once(id).chain(self.ancestors(id)).any(|block| {
            self.blocks
                .get(&block)
                .is_some_and(BlockNode::holds_settings)
        })
    }

    fn below_settings(&self, id: Uuid) -> bool {
        self.blocks
            .get(&id)
            .and_then(|node| node.parent.block())
            .is_some_and(|parent| self.in_settings(parent))
    }

    pub fn settings_of(&self, owner: Uuid) -> Vec<Uuid> {
        self.settings
            .get(&owner)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn kinds_of(&self, owner: Uuid) -> Vec<Uuid> {
        let mut kinds: Vec<Uuid> = self
            .settings_of(owner)
            .into_iter()
            .flat_map(|settings| self.child_ids(settings).collect::<Vec<_>>())
            .collect();
        kinds.extend(
            self.child_ids(owner)
                .filter(|child| self.blocks.get(child).is_some_and(BlockNode::is_edit_kind)),
        );
        kinds
    }

    pub fn settings_owner(&self, kind: Uuid) -> Option<Uuid> {
        let node = self.blocks.get(&kind)?;
        if node.is_edit_kind() {
            return node.parent.block();
        }
        let settings = node.parent.block()?;
        let node = self.blocks.get(&settings)?;
        match node.is_block_settings() {
            true => node.parent.block(),
            false => None,
        }
    }

    fn open_owner(&self, block: Uuid) -> Option<Uuid> {
        match self.blocks.get(&block)?.is_open_inside() {
            true => self.settings_owner(block),
            false => None,
        }
    }

    fn opens_to(&self, block: Uuid, account: Uuid) -> bool {
        self.open_owner(block)
            .is_some_and(|owner| self.views_inside(owner, account))
    }

    pub fn is_inside(&self, owner: Uuid, block: Uuid) -> bool {
        let mut current = block;
        for _ in 0..=self.blocks.len() {
            let Some(node) = self.blocks.get(&current) else {
                return false;
            };
            if node.is_block_settings() || node.is_edit_kind() {
                return false;
            }
            match node.parent.block() {
                Some(parent) if parent == owner => return true,
                Some(parent) => current = parent,
                None => return false,
            }
        }
        false
    }

    pub fn views_inside(&self, owner: Uuid, account: Uuid) -> bool {
        self.entry_points(account)
            .into_iter()
            .any(|entry| self.is_inside(owner, entry) && self.inherited(entry, account).can_view())
    }

    pub fn inside_viewers(&self, owner: Uuid) -> BTreeSet<Uuid> {
        let mut viewers = BTreeSet::new();
        let mut stack: Vec<Uuid> = self.child_ids(owner).collect();
        while let Some(block) = stack.pop() {
            let Some(node) = self.blocks.get(&block) else {
                continue;
            };
            if node.is_block_settings() || node.is_edit_kind() {
                continue;
            }
            viewers.insert(node.author);
            viewers.extend(
                node.grants
                    .iter()
                    .filter(|(_, level)| level.can_view())
                    .map(|(account, _)| *account),
            );
            stack.extend(self.child_ids(block));
        }
        viewers.remove(&ANYONE_INSIDE);
        viewers
    }

    pub fn effective_access(&self, id: Uuid, account: Uuid) -> Access {
        let level = self.inherited(id, account);
        if level.can_know_exists() {
            return level;
        }
        if self.referenced_by_viewer(id, account) {
            return Access::KnowExists;
        }
        Access::None
    }

    fn referenced_by_viewer(&self, id: Uuid, account: Uuid) -> bool {
        let Some(node) = self.blocks.get(&id) else {
            return false;
        };
        self.backrefs.get(&id).is_some_and(|referrers| {
            referrers.iter().any(|referrer| {
                node.parent != BlockParent::Block(*referrer)
                    && self.inherited(*referrer, account).can_view()
            })
        })
    }

    pub fn shared(&self, account: Uuid) -> Vec<Uuid> {
        self.entry_points(account)
            .into_iter()
            .filter(|entry| {
                let Some(node) = self.blocks.get(entry) else {
                    return false;
                };
                let BlockParent::Block(parent) = node.parent else {
                    return false;
                };
                !self.inherited(parent, account).can_view()
                    && level_below(node, account, Access::None, false, self.in_settings(parent))
                        .can_view()
            })
            .collect()
    }

    pub fn visible(&self, account: Uuid) -> BTreeMap<Uuid, Access> {
        let mut levels: BTreeMap<Uuid, Access> = BTreeMap::new();
        fn raise(levels: &mut BTreeMap<Uuid, Access>, id: Uuid, level: Access) {
            let current = levels.entry(id).or_default();
            *current = (*current).max(level);
        }
        let mut roots = Vec::new();
        for entry in self.entry_points(account) {
            let Some(node) = self.blocks.get(&entry) else {
                continue;
            };
            let parent = match node.parent {
                BlockParent::Block(parent) => self.inherited(parent, account),
                BlockParent::Root | BlockParent::Detached => Access::None,
            };
            if parent.can_view() {
                continue;
            }
            let open = matches!(node.parent, BlockParent::Root | BlockParent::Detached);
            let in_settings = node
                .parent
                .block()
                .is_some_and(|parent| self.in_settings(parent));
            let level = level_below(node, account, Access::None, open, in_settings);
            match level.can_view() {
                true => roots.push((entry, level, in_settings)),
                false if level.can_know_exists() => raise(&mut levels, entry, level),
                false => {}
            }
        }
        let mut owners = BTreeSet::new();
        for (root, _, _) in &roots {
            let mut current = *root;
            while let Some(node) = self.blocks.get(&current) {
                let Some(parent) = node.parent.block() else {
                    break;
                };
                if node.is_block_settings() || node.is_edit_kind() || !owners.insert(parent) {
                    break;
                }
                current = parent;
            }
        }
        let mut stack = roots;
        for owner in owners {
            for kind in self.kinds_of(owner) {
                if self
                    .blocks
                    .get(&kind)
                    .is_some_and(BlockNode::is_open_inside)
                {
                    stack.push((kind, Access::View, self.below_settings(kind)));
                }
            }
        }
        while let Some((id, level, in_settings)) = stack.pop() {
            raise(&mut levels, id, level);
            let in_settings =
                in_settings || self.blocks.get(&id).is_some_and(BlockNode::holds_settings);
            for child in self.child_ids(id) {
                if let Some(node) = self.blocks.get(&child) {
                    stack.push((
                        child,
                        level_below(node, account, level, true, in_settings),
                        in_settings,
                    ));
                }
            }
        }
        let referenced: Vec<Uuid> = levels
            .iter()
            .filter(|(_, level)| level.can_view())
            .flat_map(|(id, _)| {
                self.references(*id).into_iter().filter(move |reference| {
                    self.blocks
                        .get(reference)
                        .is_some_and(|target| target.parent != BlockParent::Block(*id))
                })
            })
            .collect();
        for id in referenced {
            raise(&mut levels, id, Access::KnowExists);
        }
        levels
    }

    pub fn viewers(&self, id: Uuid) -> BTreeSet<Uuid> {
        let mut viewers = BTreeSet::new();
        let chain: Vec<(Uuid, &BlockNode)> = std::iter::once(id)
            .chain(self.ancestors(id))
            .map_while(|block| Some((block, self.blocks.get(&block)?)))
            .collect();
        let mut in_settings = false;
        for (block, node) in chain.into_iter().rev() {
            if let Some(owner) = self.open_owner(block) {
                viewers.extend(self.inside_viewers(owner));
            }
            in_settings = in_settings || node.holds_settings();
            if in_settings {
                continue;
            }
            viewers.insert(node.author);
            viewers.extend(
                node.grants
                    .iter()
                    .filter(|(_, level)| level.can_view())
                    .map(|(account, _)| *account),
            );
        }
        viewers.remove(&ANYONE_INSIDE);
        viewers
    }

    pub fn audience(&self, id: Uuid) -> BTreeSet<Uuid> {
        let Some(node) = self.blocks.get(&id) else {
            return BTreeSet::new();
        };
        let mut audience = self.viewers(id);
        audience.extend(
            node.grants
                .iter()
                .filter(|(_, level)| level.can_know_exists())
                .map(|(account, _)| *account),
        );
        for referrer in self.backrefs.get(&id).into_iter().flatten() {
            if node.parent != BlockParent::Block(*referrer) {
                audience.extend(self.viewers(*referrer));
            }
        }
        audience
    }

    fn node_mut(&mut self, id: Uuid) -> Result<&mut BlockNode, GraphError> {
        self.blocks.get_mut(&id).ok_or(GraphError::NotFound(id))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObjectRefs {
    counts: BTreeMap<Hash, u32>,
}

impl ObjectRefs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn count(&self, hash: Hash) -> u32 {
        self.counts.get(&hash).copied().unwrap_or(0)
    }

    pub fn len(&self) -> usize {
        self.counts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    pub fn retain(&mut self, hashes: &[Hash]) -> Vec<Hash> {
        let mut fresh = Vec::new();
        for hash in hashes {
            let count = self.counts.entry(*hash).or_insert(0);
            if *count == 0 {
                fresh.push(*hash);
            }
            *count += 1;
        }
        fresh
    }

    pub fn release(&mut self, hashes: &[Hash]) -> Vec<Hash> {
        let mut freed = Vec::new();
        for hash in hashes {
            let Some(count) = self.counts.get_mut(hash) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.counts.remove(hash);
                freed.push(*hash);
            }
        }
        freed
    }
}

#[cfg(test)]
mod tests;
